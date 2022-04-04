use std::{
    borrow::Cow,
    cell::{Cell, RefCell},
    convert::TryInto,
    fs::{read, File},
    io::BufReader,
    path::PathBuf,
    rc::Rc,
    sync::{Arc, Condvar, Mutex},
    thread,
    time::Duration,
};

use calloop::EventLoop;
use plitki_core::{
    map::Map,
    scroll::ScrollSpeed,
    state::{GameState, LongNoteState, ObjectState, RegularObjectState},
    timing::{GameTimestamp, GameTimestampDifference, MapTimestampDifference},
};
use plitki_map_qua::from_reader;
use rodio::{Sink, Source};
use rust_hawktracer::*;
use slog::{o, Drain};
use slog_scope::{debug, info, trace, warn};
use smithay_client_toolkit::{
    default_environment, new_default_environment,
    reexports::client::{
        protocol::{
            wl_pointer::{self, ButtonState},
            wl_surface::WlSurface,
        },
        Attached, Display, Main,
    },
    seat::keyboard::{keysyms, map_keyboard_repeat, Event as KbEvent, KeyState, RepeatKind},
    window::{ConceptFrame, Event as WEvent},
    WaylandSource,
};
use structopt::StructOpt;
use triple_buffer::TripleBuffer;
use wayland_protocols::presentation_time::client::{
    wp_presentation::{self, WpPresentation},
    wp_presentation_feedback,
};

mod clock_gettime;
use clock_gettime::clock_gettime;

mod frame_scheduler;
use frame_scheduler::FrameScheduler;

mod backend;
use backend::create_context;

mod renderer;
use renderer::Renderer;

#[derive(Debug)]
enum RenderThreadEvent {
    Exit,
    Redraw { new_dimensions: Option<(u32, u32)> },
}

#[derive(StructOpt)]
struct Opt {
    /// Global (universal) offset in milliseconds.
    #[structopt(short, long, default_value = "-87")]
    global_offset: i16,

    /// Local (map) offset in milliseconds.
    #[structopt(short, long, default_value = "0")]
    local_offset: i16,

    /// Start from this timestamp in milliseconds.
    #[structopt(short, long, default_value = "0")]
    start: u32,

    /// Rounds map timestamps to 1 millisecond, which fixes broken osu! timing line animations (for
    /// example, the one in the end of Backbeat Maniac).
    #[structopt(long)]
    fix_osu_timing_line_animations: bool,

    /// Disables render target time adjustment based on estimated presentation latency.
    #[structopt(long)]
    disable_frame_scheduling: bool,

    /// Path to a supported map file.
    path: Option<PathBuf>,
}

default_environment!(Environment, desktop);

#[derive(Debug, Clone, PartialEq, Eq)]
struct State {
    pub game_state: GameState,
    /// If `true`, heavily limit the FPS for testing.
    pub cap_fps: bool,
    /// Note scrolling speed.
    pub scroll_speed: ScrollSpeed,
    /// If `true`, disable scroll speed changes.
    pub no_scroll_speed_changes: bool,
    /// If `true`, draws two playfields, one regular and another without scroll speed changes.
    pub two_playfields: bool,
}

impl State {
    fn new(map: Map) -> Self {
        Self {
            game_state: GameState::new(map).unwrap(),
            cap_fps: false,
            scroll_speed: ScrollSpeed(32),
            no_scroll_speed_changes: false,
            two_playfields: false,
        }
    }

    fn update_to_latest(&mut self, latest: &State) {
        self.game_state.update_to_latest(&latest.game_state);
        self.cap_fps = latest.cap_fps;
        self.scroll_speed = latest.scroll_speed;
        self.no_scroll_speed_changes = latest.no_scroll_speed_changes;
        self.two_playfields = latest.two_playfields;
    }
}

fn main() {
    better_panic::install();

    let mut opt = Opt::from_args();

    let instance = HawktracerInstance::new();
    let _listener = instance.create_listener(HawktracerListenerType::ToFile {
        file_path: "trace.bin".into(),
        buffer_size: 4096,
    });

    let decorator = slog_term::TermDecorator::new().build();
    let drain = slog_term::CompactFormat::new(decorator).build().fuse();
    let drain = slog_async::Async::new(drain).build().fuse();
    let log = slog::Logger::root(drain, o!("version" => env!("CARGO_PKG_VERSION")));
    let _guard = slog_scope::set_global_logger(log);

    let sink = Rc::new(RefCell::new(None));

    let (qua, mut play) = if let Some(mut path) = opt.path.take() {
        let sink = sink.clone();

        (
            Cow::from(read(&path).unwrap()),
            Some(move |filename| {
                path.pop();
                path.push(filename);

                if let Ok(audio_file) = File::open(&path) {
                    let source = rodio::Decoder::new(BufReader::new(audio_file))
                        .unwrap()
                        .amplify(0.6);

                    let (stream, stream_handle) = rodio::OutputStream::try_default().unwrap();
                    let sink_ = Sink::try_new(&stream_handle).unwrap();
                    sink_.append(source);
                    *sink.borrow_mut() = Some(sink_);

                    Box::leak(Box::new(stream));
                } else {
                    warn!("error opening audio file"; "path" => path.to_string_lossy().as_ref());
                }
            }),
        )
    } else {
        (
            Cow::from(&include_bytes!("../../plitki-map-qua/tests/data/actual_map.qua")[..]),
            None,
        )
    };
    let map: Map = from_reader(&*qua).unwrap().into();
    let mut audio_file = map.audio_file.as_ref().cloned();

    // The latest game state on the main thread. Main thread uses this for updates relying on
    // previous game state (for example, toggling a bool), and then refreshes the triple buffered
    // state accordingly.
    let mut latest_state = State::new(map);
    latest_state.game_state.timestamp_converter.global_offset =
        GameTimestampDifference::from_millis(i32::from(opt.global_offset));
    latest_state.game_state.timestamp_converter.local_offset =
        MapTimestampDifference::from_millis(i32::from(opt.local_offset));

    let state_buffer = TripleBuffer::new(latest_state.clone());
    let (buf_input, buf_output) = state_buffer.split();
    let state_pair = Rc::new(RefCell::new((latest_state, buf_input)));

    let (env, display, event_queue) = new_default_environment!(Environment, desktop)
        .expect("Failed to connect to the wayland server.");

    let mut event_loop = EventLoop::<Option<WEvent>>::new().unwrap();
    let surface = env.create_surface().detach();

    const INITIAL_DIMENSIONS: (u32, u32) = (640, 360);
    let mut window = env
        .create_window::<ConceptFrame, _>(
            surface,
            None,
            INITIAL_DIMENSIONS, // the initial internal dimensions of the window
            move |evt, mut dispatch_data| {
                let next_action = dispatch_data.get::<Option<WEvent>>().unwrap();
                // Check if we need to replace the old event by the new one.
                let replace = matches!(
                    (&evt, &*next_action),
                    // replace if there is no old event
                    (_, &None)
                    // or the old event is refresh
                    | (_, &Some(WEvent::Refresh))
                    // or we had a configure and received a new one
                    | (&WEvent::Configure { .. }, &Some(WEvent::Configure { .. }))
                    // or the new event is close
                    | (&WEvent::Close, _)
                );
                if replace {
                    *next_action = Some(evt);
                }
            },
            // creating the window may fail if the code drawing the frame
            // fails to initialize itself. For ConceptFrame this should not happen
            // unless the system is utterly broken, though.
        )
        .expect("Failed to create a window !");

    window.set_title("Plitki Wayland".to_string());

    // Get the presentation-time global.
    let presentation_clock_id = Arc::new(Mutex::new(0));
    let start = Arc::new(Mutex::new(None));
    let wp_presentation = {
        let presentation_clock_id = presentation_clock_id.clone();
        let start = start.clone();
        let start_from = opt.start;

        let wp_presentation: Main<WpPresentation> = env.manager.instantiate_exact(1).unwrap();
        wp_presentation.quick_assign(move |_, event, _| {
            if let wp_presentation::Event::ClockId { clk_id } = event {
                debug!("presentation ClockId"; "clk_id" => clk_id);
                *presentation_clock_id.lock().unwrap() = clk_id;

                let start_time = clock_gettime(clk_id);
                debug!("start"; "start" => ?start_time);

                *start.lock().unwrap() =
                    Some(start_time - Duration::from_millis(u64::from(start_from)));

                if start_from == 0 {
                    if let Some(mut play) = play.take() {
                        play(audio_file.take().unwrap());
                    }
                }
            }
        });
        wp_presentation
    };

    let seats = env.get_all_seats();
    let seat = seats.get(0).expect("No seats found");

    // Map the keyboard.
    {
        let presentation_clock_id = presentation_clock_id.clone();
        let start = start.clone();
        let state_pair = state_pair.clone();

        let _ = map_keyboard_repeat(
            event_loop.handle(),
            seat,
            None,
            RepeatKind::System,
            move |event: KbEvent, _, _| match event {
                KbEvent::Enter { keysyms, .. } => {
                    trace!("KbEvent::Enter"; "keysyms.len()" => keysyms.len());
                }
                KbEvent::Leave { .. } => {
                    trace!("KbEvent::Leave");
                }
                KbEvent::Key {
                    keysym,
                    state,
                    utf8,
                    time,
                    ..
                } => {
                    trace!(
                        "KbEvent::Key";
                        "state" => ?state, "time" => time, "keysym" => keysym, "utf8" => utf8
                    );

                    let elapsed = clock_gettime(*presentation_clock_id.lock().unwrap())
                        - start.lock().unwrap().unwrap();
                    let elapsed_timestamp = elapsed.try_into().unwrap();

                    let (latest_state, buf_input) = &mut *state_pair.borrow_mut();

                    match state {
                        KeyState::Pressed => match keysym {
                            keysyms::XKB_KEY_v => {
                                latest_state.cap_fps = !latest_state.cap_fps;
                                debug!("changed cap_fps"; "cap_fps" => latest_state.cap_fps);
                            }
                            keysyms::XKB_KEY_n => {
                                latest_state.no_scroll_speed_changes =
                                    !latest_state.no_scroll_speed_changes;
                                debug!(
                                    "changed no_scroll_speed_changes";
                                    "no_scroll_speed_changes"
                                        => latest_state.no_scroll_speed_changes
                                );
                            }
                            keysyms::XKB_KEY_m => {
                                latest_state.two_playfields = !latest_state.two_playfields;
                                debug!(
                                    "changed two_playfields";
                                    "two_playfields" => latest_state.two_playfields
                                );
                            }
                            keysyms::XKB_KEY_F3 => {
                                latest_state.scroll_speed.0 =
                                    (latest_state.scroll_speed.0 - 1).max(1);
                                debug!(
                                    "changed scroll_speed";
                                    "scroll_speed" => ?latest_state.scroll_speed
                                );
                            }
                            keysyms::XKB_KEY_F4 => {
                                latest_state.scroll_speed.0 += 1;
                                debug!(
                                    "changed scroll_speed";
                                    "scroll_speed" => ?latest_state.scroll_speed
                                );
                            }
                            keysyms::XKB_KEY_minus => {
                                latest_state.game_state.timestamp_converter.local_offset =
                                    latest_state.game_state.timestamp_converter.local_offset
                                        - MapTimestampDifference::from_millis(5);
                                debug!(
                                    "changed local_offset";
                                    "local_offset"
                                        => ?latest_state.game_state.timestamp_converter.local_offset
                                );
                            }
                            keysyms::XKB_KEY_plus | keysyms::XKB_KEY_equal => {
                                latest_state.game_state.timestamp_converter.local_offset =
                                    latest_state.game_state.timestamp_converter.local_offset
                                        + MapTimestampDifference::from_millis(5);
                                debug!(
                                    "changed local_offset";
                                    "local_offset"
                                        => ?latest_state.game_state.timestamp_converter.local_offset
                                );
                            }
                            keysyms::XKB_KEY_z | keysyms::XKB_KEY_a => {
                                latest_state
                                    .game_state
                                    .key_press(0, GameTimestamp(elapsed_timestamp));
                            }
                            keysyms::XKB_KEY_x | keysyms::XKB_KEY_s => {
                                latest_state
                                    .game_state
                                    .key_press(1, GameTimestamp(elapsed_timestamp));
                            }
                            keysyms::XKB_KEY_period | keysyms::XKB_KEY_d => {
                                latest_state
                                    .game_state
                                    .key_press(2, GameTimestamp(elapsed_timestamp));
                            }
                            keysyms::XKB_KEY_slash | keysyms::XKB_KEY_space => {
                                latest_state
                                    .game_state
                                    .key_press(3, GameTimestamp(elapsed_timestamp));
                            }
                            keysyms::XKB_KEY_l => {
                                latest_state
                                    .game_state
                                    .key_press(4, GameTimestamp(elapsed_timestamp));
                            }
                            keysyms::XKB_KEY_semicolon => {
                                latest_state
                                    .game_state
                                    .key_press(5, GameTimestamp(elapsed_timestamp));
                            }
                            keysyms::XKB_KEY_apostrophe => {
                                latest_state
                                    .game_state
                                    .key_press(6, GameTimestamp(elapsed_timestamp));
                            }
                            _ => (),
                        },

                        KeyState::Released => match keysym {
                            keysyms::XKB_KEY_z | keysyms::XKB_KEY_a => {
                                latest_state
                                    .game_state
                                    .key_release(0, GameTimestamp(elapsed_timestamp));
                            }
                            keysyms::XKB_KEY_x | keysyms::XKB_KEY_s => {
                                latest_state
                                    .game_state
                                    .key_release(1, GameTimestamp(elapsed_timestamp));
                            }
                            keysyms::XKB_KEY_period | keysyms::XKB_KEY_d => {
                                latest_state
                                    .game_state
                                    .key_release(2, GameTimestamp(elapsed_timestamp));
                            }
                            keysyms::XKB_KEY_slash | keysyms::XKB_KEY_space => {
                                latest_state
                                    .game_state
                                    .key_release(3, GameTimestamp(elapsed_timestamp));
                            }
                            keysyms::XKB_KEY_l => {
                                latest_state
                                    .game_state
                                    .key_release(4, GameTimestamp(elapsed_timestamp));
                            }
                            keysyms::XKB_KEY_semicolon => {
                                latest_state
                                    .game_state
                                    .key_release(5, GameTimestamp(elapsed_timestamp));
                            }
                            keysyms::XKB_KEY_apostrophe => {
                                latest_state
                                    .game_state
                                    .key_release(6, GameTimestamp(elapsed_timestamp));
                            }
                            _ => (),
                        },

                        _ => unreachable!(),
                    }

                    // if (something changed)?
                    buf_input.input_buffer().update_to_latest(latest_state);
                    buf_input.publish();
                }
                KbEvent::Repeat {
                    time, keysym, utf8, ..
                } => {
                    trace!(
                        "KbEvent::Repeat";
                        "time" => time, "keysym" => keysym, "utf8" => utf8
                    );
                }
                KbEvent::Modifiers { modifiers } => {
                    trace!("KbEvent::Modifiers"; "modifiers" => ?modifiers);
                }
            },
        )
        .expect("Failed to map keyboard");
    }

    let current_dimensions = Rc::new(Cell::new(INITIAL_DIMENSIONS));

    // Map the pointer.
    {
        let main_surface = window.surface().clone();
        let current_dimensions = current_dimensions.clone();
        let game_state_pair = state_pair.clone();
        let presentation_clock_id = presentation_clock_id.clone();
        let start = start.clone();
        let mut mouse_on_main_surface = false;
        let mut mouse_x = 0.;
        let mut _mouse_y = 0.;
        let mut holding_left_mouse_button = false;

        seat.get_pointer().quick_assign(move |_, evt, _| {
            let set_playback_position = |mouse_x| {
                let (state, _) = &*game_state_pair.borrow();

                let first_timestamp = state.game_state.first_timestamp();
                if first_timestamp.is_none() {
                    return;
                }
                let first_timestamp = first_timestamp.unwrap();
                let last_timestamp = state.game_state.last_timestamp().unwrap();

                let total_difference = last_timestamp - first_timestamp;
                let total_width = current_dimensions.get().0 as f32;

                let timestamp = first_timestamp
                    + MapTimestampDifference::from_milli_hundredths(
                        (mouse_x
                            / (f64::from(total_width)
                                / f64::from(total_difference.into_milli_hundredths())))
                            as i32,
                    );

                let elapsed = clock_gettime(*presentation_clock_id.lock().unwrap())
                    - start.lock().unwrap().unwrap();
                let elapsed_timestamp = GameTimestamp(elapsed.try_into().unwrap())
                    .to_map(&state.game_state.timestamp_converter);

                let difference = (timestamp - elapsed_timestamp)
                    .to_game(&state.game_state.timestamp_converter)
                    .as_millis();

                if difference >= 0 {
                    *start.lock().unwrap().as_mut().unwrap() -=
                        Duration::from_millis(difference as u64);
                } else {
                    *start.lock().unwrap().as_mut().unwrap() +=
                        Duration::from_millis(-difference as u64);
                }

                if let Some(sink) = &*sink.borrow() {
                    sink.stop();
                }
            };

            match evt {
                wl_pointer::Event::Enter {
                    surface,
                    surface_x,
                    surface_y,
                    ..
                } => {
                    if main_surface == surface {
                        trace!(
                            "pointer entered";
                            "surface_x" => surface_x,
                            "surface_y" => surface_y,
                        );
                        mouse_on_main_surface = true;
                        mouse_x = surface_x;
                        _mouse_y = surface_y;
                    }
                }
                wl_pointer::Event::Leave { surface, .. } => {
                    if main_surface == surface {
                        trace!("pointer left");
                        mouse_on_main_surface = false;
                    }
                }
                wl_pointer::Event::Motion {
                    surface_x,
                    surface_y,
                    ..
                } if mouse_on_main_surface => {
                    mouse_x = surface_x;
                    _mouse_y = surface_y;

                    if holding_left_mouse_button {
                        set_playback_position(mouse_x);
                    }
                }
                wl_pointer::Event::Button { button, state, .. } if mouse_on_main_surface => {
                    trace!(
                        "pointer button";
                        "button" => button,
                        "state" => ?state,
                    );

                    if button == 0x110 {
                        // Left mouse button.
                        if state == ButtonState::Pressed {
                            holding_left_mouse_button = true;
                            set_playback_position(mouse_x);
                        } else {
                            holding_left_mouse_button = false;
                        }
                    }
                }
                _ => {}
            }
        });
    }

    // Start receiving the frame callbacks.
    let need_redraw = Rc::new(Cell::new(false));
    {
        struct FrameHandler {
            need_redraw: Rc<Cell<bool>>,
            surface: Attached<WlSurface>,
        }

        impl FrameHandler {
            fn done(self) {
                trace!("frame done");

                // This will get picked up in the event handling loop.
                self.need_redraw.set(true);

                // Subscribe to the next frame callback.
                let surface = self.surface.clone();
                let mut frame_callback = Some(self);
                surface.frame().quick_assign(move |_, _, _| {
                    frame_callback.take().unwrap().done();
                });
            }
        }

        let surface = window.surface().as_ref().attach(event_queue.token());

        // Subscribe to the first frame callback.
        let mut frame_handler = Some(FrameHandler {
            need_redraw: need_redraw.clone(),
            surface: surface.clone(),
        });
        surface.frame().quick_assign(move |_, _, _| {
            frame_handler.take().unwrap().done();
        });
    }

    // Start up the rendering thread.
    let pair = Arc::new((Mutex::new(None), Condvar::new()));
    let rendering_thread = {
        let display = display.clone();
        let surface = window.surface().clone();
        let pair = pair.clone();
        let presentation_clock_id = presentation_clock_id.clone();
        let start = start.clone();
        let fix_osu_timing_line_animations = opt.fix_osu_timing_line_animations;
        let disable_frame_scheduling = opt.disable_frame_scheduling;
        let wp_presentation = (**wp_presentation).clone();

        thread::spawn(move || {
            render_thread(
                display,
                surface,
                INITIAL_DIMENSIONS,
                pair,
                buf_output,
                wp_presentation,
                presentation_clock_id,
                start,
                fix_osu_timing_line_animations,
                disable_frame_scheduling,
            )
        })
    };

    let &(ref lock, ref cvar) = &*pair;

    if !env.get_shell().unwrap().needs_configure() {
        // Not sure how exactly this is supposed to work.
        unimplemented!("wl_shell");

        // debug!("!needs_configure()");
        // initial draw to bootstrap on wl_shell
        // let cap_fps = *cap_fps.lock().unwrap();
        // window.refresh();
    }

    // We need to draw on the first configure. However, it will usually send None dimensions, and
    // redrawing is not forced when the dimensions haven't changed. Thus, this variable is needed
    // to force redrawing on the first configure.
    let mut received_configure = false;

    WaylandSource::new(event_queue)
        .quick_insert(event_loop.handle())
        .unwrap();

    let mut next_action = None;
    loop {
        let mut new_dimensions = None;
        let mut refresh_decorations = false;

        trace!("main thread iteration"; "next_action" => ?next_action);

        match next_action.take() {
            Some(WEvent::Close) => break,
            Some(WEvent::Refresh) => {
                refresh_decorations = true;
            }
            Some(WEvent::Configure { new_size, .. }) => {
                trace!("configure"; "new_size" => ?new_size);

                new_dimensions = new_size;
                refresh_decorations = true;

                if !received_configure {
                    received_configure = true;
                    need_redraw.set(true);
                }

                if let Some(new_dimensions) = new_dimensions {
                    if current_dimensions.get() != new_dimensions {
                        // TODO: if the rendering is slow, this gets triggered much more
                        // frequently, so the decorations resize faster than the surface itself.
                        //
                        // I'm not sure there's a good way of solving this without moving the
                        // window handling to the rendering thread.
                        window.resize(new_dimensions.0, new_dimensions.1);
                    }

                    current_dimensions.set(new_dimensions);
                }
            }
            None => {}
        }

        if refresh_decorations {
            window.refresh();
        }

        if need_redraw.get() || new_dimensions.is_some() {
            need_redraw.set(false);
            *lock.lock().unwrap() = Some(RenderThreadEvent::Redraw { new_dimensions });
            cvar.notify_one();

            // TODO: move this somewhere more appropriate.
            let elapsed = clock_gettime(*presentation_clock_id.lock().unwrap())
                - start.lock().unwrap().unwrap();
            let elapsed_timestamp = elapsed.try_into().unwrap();

            let (latest_state, buf_input) = &mut *state_pair.borrow_mut();
            for lane in 0..latest_state.game_state.lane_states.len() {
                latest_state
                    .game_state
                    .update(lane, GameTimestamp(elapsed_timestamp));
            }
            buf_input.input_buffer().update_to_latest(latest_state);
            buf_input.publish();
        }

        display.flush().unwrap();
        event_loop
            .dispatch(None, &mut next_action)
            .expect("Failed to dispatch all messages.");
    }

    *lock.lock().unwrap() = Some(RenderThreadEvent::Exit);
    cvar.notify_one();
    rendering_thread.join().unwrap();

    // Print hit statistics.
    let (latest_state, _) = &mut *state_pair.borrow_mut();
    let mut difference_sum = GameTimestampDifference::from_millis(0);
    let mut difference_count = 0u32;
    for object in latest_state
        .game_state
        .lane_states
        .iter()
        .flat_map(|x| x.object_states.iter())
    {
        difference_count += 1;
        match *object {
            ObjectState::Regular(RegularObjectState::Hit { difference }) => {
                difference_sum = difference_sum + difference;
            }
            ObjectState::LongNote(LongNoteState::Hit {
                press_difference,
                release_difference,
            }) => {
                difference_sum = difference_sum + press_difference + release_difference;
                difference_count += 1;
            }
            ObjectState::LongNote(LongNoteState::Held { press_difference }) => {
                difference_sum = difference_sum + press_difference;
            }
            ObjectState::LongNote(LongNoteState::Missed {
                press_difference: Some(press_difference),
                ..
            }) => {
                difference_sum = difference_sum + press_difference;
            }
            _ => {
                difference_count -= 1;
            }
        }
    }

    let average_hit_difference = if difference_count == 0 {
        0.
    } else {
        difference_sum.into_milli_hundredths() as f32 / difference_count as f32 / 100.
    };
    info!("hit statistics"; "average hit difference" => average_hit_difference);
}

#[allow(clippy::too_many_arguments)]
fn render_thread(
    display: Display,
    surface: WlSurface,
    mut dimensions: (u32, u32),
    pair: Arc<(Mutex<Option<RenderThreadEvent>>, Condvar)>,
    mut state_buffer: triple_buffer::Output<State>,
    wp_presentation: WpPresentation,
    presentation_clock_id: Arc<Mutex<u32>>,
    start_time: Arc<Mutex<Option<Duration>>>,
    fix_osu_timing_line_animations: bool,
    disable_frame_scheduling: bool,
) {
    let (backend, context) = create_context(&display, &surface, dimensions);
    let mut renderer = Renderer::new(context, dimensions);

    let mut start = None;
    let mut clk_id = None;

    let frame_scheduler = FrameScheduler::new();

    let mut event_queue = display.create_event_queue();
    let wp_presentation = wp_presentation.as_ref().attach(event_queue.token());

    let &(ref lock, ref cvar) = &*pair;
    loop {
        // Wait for an event.
        let event = {
            let mut event = lock.lock().unwrap();
            while event.is_none() {
                event = cvar.wait(event).unwrap();
            }
            event.take().unwrap()
        };

        trace!("render thread event"; "event" => ?event);

        event_queue
            .dispatch_pending(&mut (), |_, _, _| {})
            .expect("Failed to dispatch all messages.");

        if start.is_none() {
            clk_id = Some(*presentation_clock_id.lock().unwrap());
            start = Some(start_time.lock().unwrap().unwrap());
            debug!("start"; "start" => ?start.unwrap());
        } else if start_time.lock().unwrap().unwrap() != start.unwrap() {
            start = Some(start_time.lock().unwrap().unwrap());
            debug!("start"; "start" => ?start.unwrap());
        }

        match event {
            RenderThreadEvent::Exit => break,
            RenderThreadEvent::Redraw { new_dimensions } => {
                scoped_tracepoint!(_redraw_event);

                // Update the dimensions if needed.
                if let Some(new_dimensions) = new_dimensions {
                    if new_dimensions != dimensions {
                        dimensions = new_dimensions;
                        backend.borrow_mut().resize(dimensions);
                    }
                }

                state_buffer.update();
                let state = state_buffer.output_buffer();

                let current_time = clock_gettime(clk_id.unwrap());
                let elapsed = current_time - start.unwrap();
                let target_time = if disable_frame_scheduling {
                    elapsed
                } else {
                    frame_scheduler.get_target_time(current_time) - start.unwrap()
                };

                trace!(
                    "starting render";
                    "elapsed" => ?elapsed,
                    "target_time" => ?target_time
                );

                {
                    let frame_scheduler = frame_scheduler.clone();

                    wp_presentation.feedback(&surface).quick_assign(
                        move |_, event, _| match event {
                            wp_presentation_feedback::Event::Discarded => {
                                warn!(
                                    "frame discarded";
                                    "target_time" => ?target_time
                                );

                                frame_scheduler.discarded();
                            }
                            wp_presentation_feedback::Event::Presented {
                                tv_sec_hi,
                                tv_sec_lo,
                                tv_nsec,
                                refresh,
                                seq_hi,
                                seq_lo,
                                ..
                            } => {
                                let last_presentation = Duration::new(
                                    (u64::from(tv_sec_hi) << 32) | u64::from(tv_sec_lo),
                                    tv_nsec,
                                );
                                let refresh_time = Duration::new(0, refresh);

                                frame_scheduler.presented(last_presentation, refresh_time);

                                let presentation_time = last_presentation - start.unwrap();
                                let (presentation_latency, sign) = presentation_time
                                    .checked_sub(target_time)
                                    .map(|x| (x, ""))
                                    .unwrap_or_else(|| (target_time - presentation_time, "-"));

                                trace!(
                                    "frame presented";
                                    "elapsed" => ?elapsed,
                                    "target_time" => ?target_time,
                                    "presentation_time" => ?presentation_time,
                                    "presentation_latency"
                                        => &format!("{}{:?}", sign, presentation_latency),
                                    "refresh" => ?refresh_time,
                                    "sequence" => (u64::from(seq_hi) << 32) | u64::from(seq_lo),
                                );
                            }
                            _ => (),
                        },
                    );
                }

                frame_scheduler.commit();

                renderer.render(
                    dimensions,
                    target_time,
                    state,
                    fix_osu_timing_line_animations,
                );
            }
        }
    }
}
