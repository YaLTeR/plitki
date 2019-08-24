use std::{
    borrow::Cow,
    cell::Cell,
    convert::TryInto,
    env::args,
    fs::read,
    rc::Rc,
    sync::{Arc, Condvar, Mutex},
    thread,
    time::Duration,
};

use plitki_core::{map::Map, state::GameState, timing::GameTimestamp};
use plitki_map_qua::from_reader;
use slog::{o, Drain};
use slog_scope::{debug, trace};
use smithay_client_toolkit::{
    keyboard::{
        keysyms, map_keyboard_auto_with_repeat, Event as KbEvent, KeyRepeatEvent, KeyRepeatKind,
        KeyState,
    },
    reexports::client::{
        protocol::{
            wl_callback::{self, WlCallback},
            wl_surface::WlSurface,
        },
        Display, NewProxy,
    },
    window::{ConceptFrame, Event as WEvent, Window},
    Environment,
};
use triple_buffer::TripleBuffer;
use wayland_protocols::presentation_time::client::{
    wp_presentation::{self, WpPresentation},
    wp_presentation_feedback,
};

mod clock_gettime;
use clock_gettime::clock_gettime;

mod backend;
use backend::create_context;

mod renderer;
use renderer::Renderer;

#[derive(Debug)]
enum RenderThreadEvent {
    Exit,
    RefreshDecorations,
    Redraw {
        new_dimensions: Option<(u32, u32)>,
        refresh_decorations: bool,
    },
}

fn main() {
    better_panic::install();
    let decorator = slog_term::TermDecorator::new().build();
    let drain = slog_term::CompactFormat::new(decorator).build().fuse();
    let drain = slog_async::Async::new(drain).build().fuse();
    let log = slog::Logger::root(drain, o!("version" => env!("CARGO_PKG_VERSION")));
    let _guard = slog_scope::set_global_logger(log);

    let qua = if let Some(path) = args().nth(1) {
        Cow::from(read(path).unwrap())
    } else {
        Cow::from(
            &include_bytes!(
                "/home/yalter/Source/rust/plitki/plitki-map-qua/tests/data/actual_map.qua"
            )[..],
        )
    };
    let map: Map = from_reader(&*qua).unwrap().into();

    // The latest game state on the main thread. Main thread uses this for updates relying on
    // previous game state (for example, toggling a bool), and then refreshes the triple buffered
    // state accordingly.
    let mut latest_game_state = GameState::new(map);
    let state_buffer = TripleBuffer::new(latest_game_state.clone());
    let (mut buf_input, buf_output) = state_buffer.split();

    let (display, mut event_queue) =
        Display::connect_to_env().expect("Failed to connect to the wayland server.");
    let env = Environment::from_display(&*display, &mut event_queue).unwrap();

    let dpi = Arc::new(Mutex::new(1));
    let surface = {
        let dpi = dpi.clone();
        env.create_surface(move |new_dpi, _surface| {
            debug!("DPI changed"; "dpi" => new_dpi);
            *dpi.lock().unwrap() = new_dpi;
        })
    };

    let next_action = Arc::new(Mutex::new(None::<WEvent>));
    let waction = next_action.clone();
    const INITIAL_DIMENSIONS: (u32, u32) = (640, 360);
    let mut window = Window::<ConceptFrame>::init_from_env(
        &env,
        surface,
        INITIAL_DIMENSIONS, // the initial internal dimensions of the window
        move |evt| {
            let mut next_action = waction.lock().unwrap();
            // Check if we need to replace the old event by the new one.
            let replace = match (&evt, &*next_action) {
                // replace if there is no old event
                (_, &None)
                // or the old event is refresh
                | (_, &Some(WEvent::Refresh))
                // or we had a configure and received a new one
                | (&WEvent::Configure { .. }, &Some(WEvent::Configure { .. }))
                // or the new event is close
                | (&WEvent::Close, _) => true,
                // keep the old event otherwise
                _ => false,
            };
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
    let wp_presentation: WpPresentation = {
        let presentation_clock_id = presentation_clock_id.clone();
        let start = start.clone();

        env.manager
            .instantiate_exact(1, move |proxy| {
                proxy.implement_closure(
                    move |event, _| {
                        if let wp_presentation::Event::ClockId { clk_id } = event {
                            debug!("presentation ClockId"; "clk_id" => clk_id);
                            *presentation_clock_id.lock().unwrap() = clk_id;

                            let start_time = clock_gettime(clk_id);
                            debug!("start"; "start" => ?start_time);

                            *start.lock().unwrap() = Some(start_time);
                        }
                    },
                    (),
                )
            })
            .unwrap()
    };

    let seat = env
        .manager
        .instantiate_range(1, 6, NewProxy::implement_dummy)
        .unwrap();
    window.new_seat(&seat);

    let seat = seat
        .as_ref()
        .make_wrapper(&event_queue.get_token())
        .expect("Failed to bind seat to event queue");

    // Map the keyboard.
    {
        let presentation_clock_id = presentation_clock_id.clone();
        let start = start.clone();

        map_keyboard_auto_with_repeat(
            &seat,
            KeyRepeatKind::System,
            move |event: KbEvent, _| match event {
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

                    if state != KeyState::Pressed {
                        return;
                    }

                    let elapsed = clock_gettime(*presentation_clock_id.lock().unwrap())
                        - start.lock().unwrap().unwrap();
                    let elapsed_timestamp = elapsed.try_into().unwrap();

                    match keysym {
                        keysyms::XKB_KEY_v => {
                            latest_game_state.cap_fps = !latest_game_state.cap_fps;
                            debug!("changed cap_fps"; "cap_fps" => latest_game_state.cap_fps);
                        }
                        keysyms::XKB_KEY_F3 => {
                            latest_game_state.scroll_speed.0 =
                                (latest_game_state.scroll_speed.0 - 1).max(1);
                            debug!(
                                "changed scroll_speed";
                                "scroll_speed" => ?latest_game_state.scroll_speed
                            );
                        }
                        keysyms::XKB_KEY_F4 => {
                            latest_game_state.scroll_speed.0 += 1;
                            debug!(
                                "changed scroll_speed";
                                "scroll_speed" => ?latest_game_state.scroll_speed
                            );
                        }
                        keysyms::XKB_KEY_z => {
                            latest_game_state.key_press(0, GameTimestamp(elapsed_timestamp));
                        }
                        keysyms::XKB_KEY_x => {
                            latest_game_state.key_press(1, GameTimestamp(elapsed_timestamp));
                        }
                        keysyms::XKB_KEY_period => {
                            latest_game_state.key_press(2, GameTimestamp(elapsed_timestamp));
                        }
                        keysyms::XKB_KEY_slash => {
                            latest_game_state.key_press(3, GameTimestamp(elapsed_timestamp));
                        }
                        _ => (),
                    }

                    // if (something changed)?
                    buf_input
                        .raw_input_buffer()
                        .update_to_latest(&latest_game_state);
                    buf_input.raw_publish();
                }
                KbEvent::RepeatInfo { rate, delay } => {
                    trace!("KbEvent::RepeatInfo"; "rate" => rate, "delay" => delay);
                }
                KbEvent::Modifiers { modifiers } => {
                    trace!("KbEvent::Modifiers"; "modifiers" => ?modifiers);
                }
            },
            move |repeat_event: KeyRepeatEvent, _| {
                trace!(
                    "KeyRepeatEvent";
                    "keysym" => repeat_event.keysym, "utf8" => repeat_event.utf8
                );
            },
        )
        .expect("Failed to map keyboard");
    }

    // Start receiving the frame callbacks.
    let need_redraw = Rc::new(Cell::new(false));
    {
        struct FrameHandler {
            need_redraw: Rc<Cell<bool>>,
            surface: WlSurface,
        }

        impl wl_callback::EventHandler for FrameHandler {
            fn done(&mut self, _object: WlCallback, _data: u32) {
                trace!("frame done");

                // This will get picked up in the event handling loop.
                self.need_redraw.set(true);

                // Subscribe to the next frame callback.
                let need_redraw = self.need_redraw.clone();
                let surface = self.surface.clone();
                self.surface
                    .frame(move |callback| {
                        callback.implement(
                            FrameHandler {
                                need_redraw,
                                surface,
                            },
                            (),
                        )
                    })
                    .unwrap();
            }
        }

        // Subscribe to the first frame callback.
        let need_redraw = need_redraw.clone();
        let surface = window.surface().clone();
        window
            .surface()
            .frame(move |callback| {
                callback.implement(
                    FrameHandler {
                        need_redraw,
                        surface,
                    },
                    (),
                )
            })
            .unwrap();
    }

    // Start up the rendering thread.
    let window = Arc::new(Mutex::new(window));

    let pair = Arc::new((Mutex::new(None), Condvar::new()));
    let rendering_thread = {
        let display = display.clone();
        let window = window.clone();
        let pair = pair.clone();
        let presentation_clock_id = presentation_clock_id.clone();
        let start = start.clone();

        thread::spawn(move || {
            render_thread(
                display,
                window,
                INITIAL_DIMENSIONS,
                pair,
                buf_output,
                wp_presentation,
                presentation_clock_id,
                start,
            )
        })
    };

    let &(ref lock, ref cvar) = &*pair;

    if !env.shell.needs_configure() {
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

    loop {
        let mut new_dimensions = None;
        let mut refresh_decorations = false;

        trace!("main thread iteration"; "next_action" => ?next_action.lock().unwrap());

        match next_action.lock().unwrap().take() {
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
            }
            None => {}
        }

        if need_redraw.get() || new_dimensions.is_some() {
            need_redraw.set(false);
            *lock.lock().unwrap() = Some(RenderThreadEvent::Redraw {
                new_dimensions,
                refresh_decorations,
            });
            cvar.notify_one();
        } else if refresh_decorations {
            *lock.lock().unwrap() = Some(RenderThreadEvent::RefreshDecorations);
            cvar.notify_one();
        }

        event_queue
            .dispatch()
            .expect("Failed to dispatch all messages.");
    }

    *lock.lock().unwrap() = Some(RenderThreadEvent::Exit);
    cvar.notify_one();
    rendering_thread.join().unwrap();
}

fn render_thread(
    display: Display,
    window: Arc<Mutex<Window<ConceptFrame>>>,
    mut dimensions: (u32, u32),
    pair: Arc<(Mutex<Option<RenderThreadEvent>>, Condvar)>,
    mut state_buffer: triple_buffer::Output<GameState>,
    wp_presentation: WpPresentation,
    presentation_clock_id: Arc<Mutex<u32>>,
    start_time: Arc<Mutex<Option<Duration>>>,
) {
    let surface = window.lock().unwrap().surface().clone();
    let (backend, context) = create_context(&display, &surface, dimensions);
    let mut renderer = Renderer::new(context, dimensions);

    let mut start = None;
    let mut clk_id = None;
    let next_frame_timestamp = Arc::new(Mutex::new(None));

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

        if start.is_none() {
            clk_id = Some(*presentation_clock_id.lock().unwrap());
            start = Some(start_time.lock().unwrap().unwrap());
            debug!("start"; "start" => ?start.unwrap());
        }

        let mut window = window.lock().unwrap();

        match event {
            RenderThreadEvent::Exit => break,
            RenderThreadEvent::RefreshDecorations => {
                window.refresh();
                display.flush().unwrap();
            }
            RenderThreadEvent::Redraw {
                new_dimensions,
                refresh_decorations,
            } => {
                // Update the dimensions if needed.
                if let Some(new_dimensions) = new_dimensions {
                    if new_dimensions != dimensions {
                        dimensions = new_dimensions;
                        window.resize(dimensions.0, dimensions.1);
                        backend.borrow_mut().resize(dimensions);

                        assert!(refresh_decorations);
                    }
                }

                if refresh_decorations {
                    window.refresh();
                }

                state_buffer.raw_update();
                let state = state_buffer.raw_output_buffer();

                let elapsed = clock_gettime(clk_id.unwrap()) - start.unwrap();
                // TODO: handle low fps and subsequent too high fps (also applies to refresh rate
                // changes)
                let target_time = if let Some(next_frame_timestamp) =
                    next_frame_timestamp.lock().unwrap().as_ref().cloned()
                {
                    next_frame_timestamp
                } else {
                    elapsed
                };

                debug!(
                    "starting render";
                    "elapsed" => ?elapsed,
                    "target_time" => ?target_time
                );

                {
                    let next_frame_timestamp = next_frame_timestamp.clone();

                    wp_presentation
                        .feedback(window.surface(), move |proxy| {
                            proxy.implement_closure_threadsafe(
                                move |event, _| match event {
                                    wp_presentation_feedback::Event::Discarded => {
                                        debug!(
                                            "frame discarded";
                                            "target_time" => ?target_time
                                        );
                                    }
                                    wp_presentation_feedback::Event::Presented {
                                        tv_sec_hi,
                                        tv_sec_lo,
                                        tv_nsec,
                                        refresh,
                                        ..
                                    } => {
                                        let presentation_time = Duration::new(
                                            (u64::from(tv_sec_hi) << 32) | u64::from(tv_sec_lo),
                                            tv_nsec,
                                        ) - start.unwrap();
                                        let (presentation_latency, sign) = presentation_time
                                            .checked_sub(target_time)
                                            .map(|x| (x, ""))
                                            .unwrap_or_else(|| {
                                                (target_time - presentation_time, "-")
                                            });

                                        let refresh = Duration::new(0, refresh);

                                        *next_frame_timestamp.lock().unwrap() =
                                            Some(presentation_time + refresh);

                                        debug!(
                                            "frame presented";
                                            "target_time" => ?target_time,
                                            "presentation_time" => ?presentation_time,
                                            "presentation_latency"
                                                => &format!("{}{:?}", sign, presentation_latency),
                                            "refresh" => ?refresh
                                        );
                                    }
                                    _ => (),
                                },
                                (),
                            )
                        })
                        .unwrap();
                }

                renderer.render(dimensions, target_time, state);
            }
        }
    }
}
