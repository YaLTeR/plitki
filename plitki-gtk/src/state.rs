use std::cell::{Ref, RefMut};

use glib::subclass::prelude::*;
use gtk::glib;
use plitki_core::state::GameState;

#[derive(Debug, Clone, glib::Boxed)]
#[boxed_type(nullable, name = "BoxedGameState")]
struct BoxedGameState(GameState);

mod imp {
    use std::cell::RefCell;

    use glib::prelude::*;
    use once_cell::sync::Lazy;
    use once_cell::unsync::OnceCell;

    use super::*;

    #[derive(Debug, Default)]
    pub struct State {
        game_state: OnceCell<RefCell<GameState>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for State {
        const NAME: &'static str = "PlitkiState";
        type Type = super::State;
        type ParentType = glib::Object;
    }

    impl ObjectImpl for State {
        fn properties() -> &'static [glib::ParamSpec] {
            static PROPERTIES: Lazy<Vec<glib::ParamSpec>> = Lazy::new(|| {
                vec![
                    glib::ParamSpecBoxed::builder::<BoxedGameState>("game-state")
                        .write_only()
                        .construct_only()
                        .build(),
                    glib::ParamSpecUInt64::builder("lane-count")
                        .read_only()
                        .build(),
                ]
            });
            PROPERTIES.as_ref()
        }

        fn property(&self, _id: usize, pspec: &glib::ParamSpec) -> glib::Value {
            match pspec.name() {
                "lane-count" => {
                    let count: u64 = self.lane_count().try_into().unwrap();
                    count.to_value()
                }
                _ => unreachable!(),
            }
        }

        fn set_property(&self, _id: usize, value: &glib::Value, pspec: &glib::ParamSpec) {
            match pspec.name() {
                "game-state" => {
                    let boxed: BoxedGameState = value.get().unwrap();
                    self.game_state.set(RefCell::new(boxed.0)).unwrap();
                }
                _ => unreachable!(),
            }
        }
    }

    impl State {
        pub fn game_state(&self) -> Ref<GameState> {
            self.game_state.get().unwrap().borrow()
        }

        pub fn game_state_mut(&self) -> RefMut<GameState> {
            self.game_state.get().unwrap().borrow_mut()
        }

        pub fn lane_count(&self) -> usize {
            self.game_state().lane_count()
        }
    }
}

glib::wrapper! {
    pub struct State(ObjectSubclass<imp::State>);
}

impl State {
    pub fn new(game_state: GameState) -> Self {
        glib::Object::builder()
            .property("game-state", BoxedGameState(game_state))
            .build()
    }

    pub fn game_state(&self) -> Ref<GameState> {
        self.imp().game_state()
    }

    pub fn game_state_mut(&self) -> RefMut<GameState> {
        self.imp().game_state_mut()
    }

    pub fn lane_count(&self) -> usize {
        self.imp().lane_count()
    }
}
