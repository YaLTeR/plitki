use std::{
    cell::Cell,
    rc::Rc,
    sync::{Arc, Condvar, Mutex},
    thread,
    time::Instant,
};

use plitki_core::map::Map;
use plitki_map_qua::from_reader;
use slog::{debug, o, trace, Drain, Logger};
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

mod backend;
use backend::create_context;

mod renderer;
use renderer::Renderer;

#[derive(Debug)]
enum RenderThreadEvent {
    RefreshDecorations,
    Redraw {
        new_dimensions: Option<(u32, u32)>,
        refresh_decorations: bool,
    },
}

#[derive(Clone)]
pub struct GameState {
    map: Arc<Map>,
    cap_fps: bool,
    /// The scroll speed, in vertical square screens per second, multiplied by 10. That is, on a
    /// square 1:1 screen, 10 means a note travels from the very top to the very bottom of the
    /// screen in one second; 5 means in two seconds and 20 means in half a second.
    scroll_speed: u8,
}

impl GameState {
    fn update_to_latest(&mut self, latest: &GameState) {
        self.cap_fps = latest.cap_fps;
        self.scroll_speed = latest.scroll_speed;
    }
}

fn main() {
    better_panic::install();
    let decorator = slog_term::TermDecorator::new().build();
    let drain = slog_term::CompactFormat::new(decorator).build().fuse();
    let drain = slog_async::Async::new(drain).build().fuse();
    let log = slog::Logger::root(drain, o!("version" => env!("CARGO_PKG_VERSION")));

    let map: Map = from_reader(
        &include_bytes!("/home/yalter/Source/rust/plitki/plitki-map-qua/tests/data/actual_map.qua")
            [..],
    )
    .unwrap()
    .into();

    // The latest game state on the main thread. Main thread uses this for updates relying on
    // previous game state (for example, toggling a bool), and then refreshes the triple buffered
    // state accordingly.
    let mut latest_game_state = GameState {
        map: Arc::new(map),
        cap_fps: false,
        scroll_speed: 10,
    };
    let state_buffer = TripleBuffer::new(latest_game_state.clone());
    let (mut buf_input, buf_output) = state_buffer.split();

    let (display, mut event_queue) =
        Display::connect_to_env().expect("Failed to connect to the wayland server.");
    let env = Environment::from_display(&*display, &mut event_queue).unwrap();

    let dpi = Arc::new(Mutex::new(1));
    let surface = {
        let dpi = dpi.clone();
        let log = log.clone();
        env.create_surface(move |new_dpi, _surface| {
            debug!(log, "DPI changed"; "dpi" => new_dpi);
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
    let log_kb = log.clone();
    let log_repeat = log.clone();
    map_keyboard_auto_with_repeat(
        &seat,
        KeyRepeatKind::System,
        move |event: KbEvent, _| match event {
            KbEvent::Enter { keysyms, .. } => {
                debug!(log_kb, "KbEvent::Enter"; "keysyms.len()" => keysyms.len());
            }
            KbEvent::Leave { .. } => {
                debug!(log_kb, "KbEvent::Leave");
            }
            KbEvent::Key {
                keysym,
                state,
                utf8,
                time,
                ..
            } => {
                debug!(
                    log_kb, "KbEvent::Key";
                    "state" => ?state, "time" => time, "keysym" => keysym, "utf8" => utf8
                );

                if state != KeyState::Pressed {
                    return;
                }

                match keysym {
                    keysyms::XKB_KEY_v => {
                        latest_game_state.cap_fps = !latest_game_state.cap_fps;
                        debug!(log_kb, "changed cap_fps"; "cap_fps" => latest_game_state.cap_fps);
                    }
                    keysyms::XKB_KEY_F3 => {
                        latest_game_state.scroll_speed -= 1;
                        debug!(
                            log_kb, "changed scroll_speed";
                            "scroll_speed" => latest_game_state.scroll_speed
                        );
                    }
                    keysyms::XKB_KEY_F4 => {
                        latest_game_state.scroll_speed += 1;
                        debug!(
                            log_kb, "changed scroll_speed";
                            "scroll_speed" => latest_game_state.scroll_speed
                        );
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
                debug!(log_kb, "KbEvent::RepeatInfo"; "rate" => rate, "delay" => delay);
            }
            KbEvent::Modifiers { modifiers } => {
                debug!(log_kb, "KbEvent::Modifiers"; "modifiers" => ?modifiers);
            }
        },
        move |repeat_event: KeyRepeatEvent, _| {
            debug!(
                log_repeat, "KeyRepeatEvent";
                "keysym" => repeat_event.keysym, "utf8" => repeat_event.utf8
            );
        },
    )
    .expect("Failed to map keyboard");

    // Start receiving the frame callbacks.
    let need_redraw = Rc::new(Cell::new(false));
    {
        struct FrameHandler {
            log: Logger,
            need_redraw: Rc<Cell<bool>>,
            surface: WlSurface,
        }

        impl wl_callback::EventHandler for FrameHandler {
            fn done(&mut self, _object: WlCallback, _data: u32) {
                trace!(self.log, "frame done");

                // This will get picked up in the event handling loop.
                self.need_redraw.set(true);

                // Subscribe to the next frame callback.
                let log = self.log.clone();
                let need_redraw = self.need_redraw.clone();
                let surface = self.surface.clone();
                self.surface
                    .frame(move |callback| {
                        callback.implement(
                            FrameHandler {
                                log,
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
        let log = log.clone();
        let need_redraw = need_redraw.clone();
        let surface = window.surface().clone();
        window
            .surface()
            .frame(move |callback| {
                callback.implement(
                    FrameHandler {
                        log,
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
    {
        let log = log.clone();
        let display = display.clone();
        let window = window.clone();
        let pair = pair.clone();

        thread::spawn(move || {
            render_thread(log, display, window, INITIAL_DIMENSIONS, pair, buf_output)
        });
    }

    let &(ref lock, ref cvar) = &*pair;

    if !env.shell.needs_configure() {
        // Not sure how exactly this is supposed to work.
        unimplemented!("wl_shell");

        // debug!(log, "!needs_configure()");
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

        trace!(log, "main thread iteration"; "next_action" => ?next_action.lock().unwrap());

        match next_action.lock().unwrap().take() {
            Some(WEvent::Close) => break,
            Some(WEvent::Refresh) => {
                refresh_decorations = true;
            }
            Some(WEvent::Configure { new_size, .. }) => {
                trace!(log, "configure"; "new_size" => ?new_size);

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
}

fn render_thread(
    log: Logger,
    display: Display,
    window: Arc<Mutex<Window<ConceptFrame>>>,
    mut dimensions: (u32, u32),
    pair: Arc<(Mutex<Option<RenderThreadEvent>>, Condvar)>,
    mut state_buffer: triple_buffer::Output<GameState>,
) {
    let surface = window.lock().unwrap().surface().clone();
    let (backend, context) = create_context(log.clone(), &display, &surface, dimensions);
    let mut renderer = Renderer::new(log.clone(), context, dimensions);

    let start = Instant::now();

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

        trace!(log, "render thread event"; "event" => ?event);

        let mut window = window.lock().unwrap();

        match event {
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

                let elapsed = Instant::now() - start;
                renderer.render(dimensions, elapsed, state);
            }
        }
    }
}
