use std::{
    convert::{identity, TryInto},
    rc::Rc,
    time::{Duration, Instant},
};

use cgmath::{Matrix4, Ortho, Point2, Vector2};
use glium::{
    backend::Context, implement_vertex, index::PrimitiveType, uniform, Blend, DrawParameters,
    Frame, IndexBuffer, Program, Surface, VertexBuffer,
};
use palette::{ComponentWise, Srgba};
use plitki_core::{
    object::Object,
    scroll::{Position, ScreenPositionDifference, ScrollSpeedMultiplier},
    state::{Hit, LongNoteCache, LongNoteState, ObjectCache, ObjectState},
    timing::{GameTimestamp, GameTimestampDifference, MapTimestamp},
};
use slog_scope::trace;

use crate::GameState;

#[derive(Copy, Clone)]
struct Vertex {
    position: [f32; 2],
}

implement_vertex!(Vertex, position);

#[derive(Copy, Clone)]
struct InstanceData {
    model: [[f32; 4]; 4],
    color: [f32; 4],
}

implement_vertex!(InstanceData, model, color);

pub struct Renderer {
    context: Rc<Context>,
    program: Program,
    vertex_buffer: VertexBuffer<Vertex>,
    index_buffer: IndexBuffer<u8>,
    dimensions: (u32, u32),
    ortho: Ortho<f32>,
    projection: Matrix4<f32>,
    sprites: Vec<Sprite>,
}

pub struct SingleFrameRenderer<'a> {
    renderer: &'a mut Renderer,
    state: &'a GameState,
    game_timestamp: GameTimestamp,
    map_timestamp: MapTimestamp,
    current_position: Position,
    lane_width: f32,
    border_offset: f32,
    border_width: f32,
    judgement_line_position: f32,
    note_height: f32,
    no_scroll_speed_changes: bool,
}

struct Sprite {
    pos: Point2<f32>,
    scale: Vector2<f32>,
    color: Srgba<f32>,
}

fn compute_ortho(dimensions: (u32, u32)) -> Ortho<f32> {
    let aspect_ratio = dimensions.0 as f32 / dimensions.1 as f32;
    let (x, y) = if aspect_ratio > 1. {
        (aspect_ratio, 1.)
    } else {
        (1., 1. / aspect_ratio)
    };

    Ortho {
        left: -x,
        right: x,
        bottom: -y,
        top: y,
        near: -1.,
        far: 1.,
    }
}

fn from_core_position_difference(x: ScreenPositionDifference) -> f32 {
    (x.0 as f64 / 1_000_000_000.) as f32
}

fn to_core_position_difference(x: f32) -> ScreenPositionDifference {
    ScreenPositionDifference((f64::from(x) * 1_000_000_000.) as i64)
}

impl Renderer {
    pub fn new(context: Rc<Context>, dimensions: (u32, u32)) -> Self {
        let shape = [
            Vertex { position: [0., 0.] },
            Vertex { position: [1., 0.] },
            Vertex { position: [1., 1.] },
            Vertex { position: [0., 1.] },
        ];
        let vertex_buffer = VertexBuffer::new(&context, &shape).unwrap();
        let indices = [0, 1, 2, 3];
        let index_buffer =
            IndexBuffer::new(&context, PrimitiveType::TriangleFan, &indices).unwrap();

        let vertex_shader_src = r#"
            #version 140

            // Vertex coordinates, before scaling.
            in vec2 position;
            // Model matrix.
            in mat4 model;
            // Color.
            in vec4 color;

            // Vertex color (output)
            out vec4 vertex_color;

            // Projection matrix, at 1:1 both coordinates should range from -1 to 1.
            uniform mat4 projection;

            void main() {
                vec4 position = vec4(position, 0.0, 1.0);
                gl_Position = projection * model * position;
                vertex_color = color;
            }
        "#;

        let fragment_shader_src = r#"
            #version 140

            // Color from the vertex shader
            in vec4 vertex_color;

            out vec4 color;

            void main() {
                color = vertex_color;
            }
        "#;

        let program =
            Program::from_source(&context, vertex_shader_src, fragment_shader_src, None).unwrap();

        let ortho = compute_ortho(dimensions);

        Self {
            context,
            program,
            vertex_buffer,
            index_buffer,
            dimensions,
            ortho,
            projection: ortho.into(),
            sprites: Vec::new(),
        }
    }

    fn build_instance_data(&self) -> VertexBuffer<InstanceData> {
        let mut buffer = VertexBuffer::empty(&self.context, self.sprites.len()).unwrap();
        {
            let mut map = buffer.as_mut_slice().map_write();
            for (i, sprite) in self.sprites.iter().enumerate() {
                const SPRITE_ORIGIN: Point2<f32> = Point2::new(0., 0.);

                let scale = Matrix4::from_nonuniform_scale(sprite.scale.x, sprite.scale.y, 1.);
                let offset = Matrix4::from_translation((sprite.pos - SPRITE_ORIGIN).extend(0.));
                let model = (offset * scale).into();

                let (r, g, b, a) = sprite.color.into_components();
                let color = [r, g, b, a];
                map.set(i, InstanceData { model, color });
            }
        }
        buffer
    }

    pub fn render(
        &mut self,
        dimensions: (u32, u32),
        elapsed: Duration,
        state: &GameState,
        fix_osu_timing_line_animations: bool,
    ) {
        let start = Instant::now();

        if dimensions != self.dimensions {
            self.dimensions = dimensions;
            self.ortho = compute_ortho(dimensions);
            self.projection = self.ortho.into();
        }

        self.sprites.clear();

        {
            let mut renderer = SingleFrameRenderer::new(
                self,
                elapsed,
                state,
                fix_osu_timing_line_animations,
                if state.two_playfields {
                    false
                } else {
                    state.no_scroll_speed_changes
                },
            );
            renderer.push_borders();
            renderer.push_timing_lines();
            renderer.push_objects();
            renderer.push_judgement_line();
            renderer.push_error_bar();
        }

        if state.two_playfields {
            let lane_count = state.immutable.map.lanes.len();
            let lane_width = if lane_count < 6 { 0.2 } else { 0.15 };
            let border_offset = lane_width * lane_count as f32 / 2.;

            for sprite in &mut self.sprites {
                sprite.pos.x -= border_offset + 0.1;
            }

            let left_len = self.sprites.len();

            {
                let mut renderer = SingleFrameRenderer::new(
                    self,
                    elapsed,
                    state,
                    fix_osu_timing_line_animations,
                    true,
                );
                renderer.push_borders();
                renderer.push_timing_lines();
                renderer.push_objects();
                renderer.push_judgement_line();
                renderer.push_error_bar();
            }

            for sprite in &mut self.sprites[left_len..] {
                sprite.pos.x += border_offset + 0.1;
            }
        }

        {
            let mut renderer = SingleFrameRenderer::new(
                self,
                elapsed,
                state,
                fix_osu_timing_line_animations,
                true,
            );
            renderer.push_timeline();
        }

        let instance_data = self.build_instance_data();

        let mut frame = Frame::new(self.context.clone(), dimensions);

        if state.cap_fps {
            frame.clear_color(0.2, 0., 0., 1.);
        } else {
            frame.clear_color(0., 0., 0., 1.);
        }

        let projection: [[f32; 4]; 4] = self.projection.into();
        frame
            .draw(
                (&self.vertex_buffer, instance_data.per_instance().unwrap()),
                &self.index_buffer,
                &self.program,
                &uniform! {
                    projection: projection,
                },
                &DrawParameters {
                    blend: Blend::alpha_blending(),
                    ..Default::default()
                },
            )
            .unwrap();

        if state.cap_fps {
            std::thread::sleep(Duration::from_millis(1000 / 15));
        }

        frame.finish().unwrap();

        trace!("finished redraw"; "time_taken" => ?(Instant::now() - start));
    }
}

impl<'a> SingleFrameRenderer<'a> {
    fn new(
        renderer: &'a mut Renderer,
        elapsed: Duration,
        state: &'a GameState,
        fix_osu_timing_line_animations: bool,
        no_scroll_speed_changes: bool,
    ) -> Self {
        let elapsed_timestamp = GameTimestamp(elapsed.try_into().unwrap());

        let map_timestamp = elapsed_timestamp.to_map(&state.timestamp_converter);
        let map_timestamp = if fix_osu_timing_line_animations {
            MapTimestamp::from_millis(map_timestamp.as_millis())
        } else {
            map_timestamp
        };
        let game_timestamp = map_timestamp.to_game(&state.timestamp_converter);

        let lane_count = state.immutable.map.lanes.len();
        let lane_width = if lane_count < 6 { 0.2 } else { 0.15 };
        let border_offset = lane_width * lane_count as f32 / 2.;
        let border_width = 0.01;
        let judgement_line_position = renderer.ortho.bottom + 0.29;
        let note_height = lane_width / 2.;

        let current_position = if no_scroll_speed_changes {
            map_timestamp.no_scroll_speed_change_position()
        } else {
            state.position_at_time(map_timestamp)
        };

        Self {
            renderer,
            state,
            game_timestamp,
            map_timestamp,
            current_position,
            lane_width,
            border_offset,
            border_width,
            judgement_line_position,
            note_height,
            no_scroll_speed_changes,
        }
    }

    #[inline]
    fn core_to_screen(&self, core_position: Position) -> f32 {
        let screen_position_difference =
            (core_position - self.current_position) * self.state.scroll_speed;
        self.judgement_line_position + from_core_position_difference(screen_position_difference)
    }

    #[inline]
    fn screen_to_core(&self, screen_position: f32) -> Position {
        let screen_position_difference =
            to_core_position_difference(screen_position - self.judgement_line_position);
        screen_position_difference / self.state.scroll_speed + self.current_position
    }

    fn push_borders(&mut self) {
        // Left lane border.
        self.renderer.sprites.push(Sprite {
            pos: Point2::new(
                -self.border_offset - self.border_width,
                self.renderer.ortho.bottom,
            ),
            scale: Vector2::new(
                self.border_width,
                self.renderer.ortho.top - self.renderer.ortho.bottom,
            ),
            color: Srgba::new(1., 1., 1., 1.),
        });
        // Right lane border.
        self.renderer.sprites.push(Sprite {
            pos: Point2::new(self.border_offset, self.renderer.ortho.bottom),
            scale: Vector2::new(
                self.border_width,
                self.renderer.ortho.top - self.renderer.ortho.bottom,
            ),
            color: Srgba::new(1., 1., 1., 1.),
        });
    }

    fn push_timing_lines(&mut self) {
        let first_visible_position =
            self.screen_to_core(self.renderer.ortho.bottom - self.border_width);
        let one_past_last_visible_position = self.screen_to_core(self.renderer.ortho.top);

        let range = if self.no_scroll_speed_changes {
            let into_timestamp = |position| {
                MapTimestamp::from_millis(0)
                    + (position - Position::zero()) / ScrollSpeedMultiplier::default()
            };
            let first_timestamp = into_timestamp(first_visible_position);
            let last_timestamp = into_timestamp(one_past_last_visible_position);

            let first_index = self
                .state
                .immutable
                .timing_lines
                .binary_search_by_key(&first_timestamp, |timing_line| timing_line.timestamp)
                .unwrap_or_else(identity);
            let last_index = self
                .state
                .immutable
                .timing_lines
                .binary_search_by_key(&last_timestamp, |timing_line| timing_line.timestamp)
                .unwrap_or_else(identity);
            first_index..last_index
        } else {
            let first_index = self
                .state
                .immutable
                .timing_lines
                .binary_search_by_key(&first_visible_position, |timing_line| timing_line.position)
                .unwrap_or_else(identity);
            let last_index = self
                .state
                .immutable
                .timing_lines
                .binary_search_by_key(&one_past_last_visible_position, |timing_line| {
                    timing_line.position
                })
                .unwrap_or_else(identity);
            first_index..last_index
        };

        for timing_line in self.state.immutable.timing_lines[range].iter().rev() {
            let position = if self.no_scroll_speed_changes {
                timing_line.timestamp.no_scroll_speed_change_position()
            } else {
                timing_line.position
            };

            let pos = Point2::new(-self.border_offset, self.core_to_screen(position));

            self.renderer.sprites.push(Sprite {
                pos,
                // TODO: 1 pixel.
                scale: Vector2::new(self.border_offset * 2., self.border_width / 2.),
                color: Srgba::new(0.5, 0.5, 1., 1.),
            });
        }
    }

    fn push_objects(&mut self) {
        // Yay, partial borrowing to win vs. the borrow checker...
        let state = self.state;

        let first_visible_position =
            self.screen_to_core(self.renderer.ortho.bottom - self.note_height);
        let one_past_last_visible_position = self.screen_to_core(self.renderer.ortho.top);

        if self.no_scroll_speed_changes {
            let into_timestamp = |position| {
                MapTimestamp::from_millis(0)
                    + (position - Position::zero()) / ScrollSpeedMultiplier::default()
            };
            let first_visible_timestamp = into_timestamp(first_visible_position);
            let one_past_last_visible_timestamp = into_timestamp(one_past_last_visible_position);

            for (lane, objects, object_states, object_caches) in
                (0..self.state.immutable.map.lanes.len()).map(|lane| {
                    (
                        lane,
                        &state.immutable.map.lanes[lane].objects[..],
                        &state.lane_states[lane].object_states[..],
                        &state.immutable.lane_caches[lane].object_caches[..],
                    )
                })
            {
                let first_visible_index = objects
                    .binary_search_by_key(&first_visible_timestamp, |object| object.end_timestamp())
                    .unwrap_or_else(identity);
                let one_past_last_visible_index = objects
                    .binary_search_by_key(&one_past_last_visible_timestamp, |object| {
                        object.start_timestamp()
                    })
                    .unwrap_or_else(identity);

                let range = first_visible_index..one_past_last_visible_index;
                for ((object, object_state), object_cache) in objects[range.clone()]
                    .iter()
                    .zip(object_states[range.clone()].iter())
                    .zip(object_caches[range].iter())
                    .rev()
                    .filter(|((_, s), _)| !s.is_hit())
                {
                    self.renderer.sprites.push(self.object_sprite(
                        lane,
                        object,
                        object_state,
                        object_cache,
                    ));
                }
            }
        } else {
            for (lane, objects, object_states, object_caches) in
                (0..self.state.immutable.map.lanes.len()).map(|lane| {
                    (
                        lane,
                        &state.immutable.map.lanes[lane].objects[..],
                        &state.lane_states[lane].object_states[..],
                        &state.immutable.lane_caches[lane].object_caches[..],
                    )
                })
            {
                let first_visible_index = object_caches
                    .binary_search_by_key(&first_visible_position, |cache| cache.end_position())
                    .unwrap_or_else(identity);
                let one_past_last_visible_index = object_caches
                    .binary_search_by_key(&one_past_last_visible_position, |cache| {
                        cache.start_position()
                    })
                    .unwrap_or_else(identity);

                let range = first_visible_index..one_past_last_visible_index;
                for ((object, object_state), object_cache) in objects[range.clone()]
                    .iter()
                    .zip(object_states[range.clone()].iter())
                    .zip(object_caches[range].iter())
                    .rev()
                    .filter(|((_, s), _)| !s.is_hit())
                {
                    self.renderer.sprites.push(self.object_sprite(
                        lane,
                        object,
                        object_state,
                        object_cache,
                    ));
                }
            }
        }
    }

    fn object_sprite(
        &self,
        lane: usize,
        object: &Object,
        object_state: &ObjectState,
        object_cache: &ObjectCache,
    ) -> Sprite {
        let (start, ln_positions) = if self.no_scroll_speed_changes {
            let start = match *object {
                Object::Regular { .. } => object.start_timestamp(),
                Object::LongNote { start, end } => match *object_state {
                    ObjectState::LongNote(LongNoteState::Held { .. }) => {
                        self.map_timestamp.min(end).max(start)
                    }

                    ObjectState::LongNote(LongNoteState::Missed {
                        held_until: Some(held_until),
                        ..
                    }) => held_until.max(start),

                    _ => object.start_timestamp(),
                },
            };

            let start = start.no_scroll_speed_change_position();

            let ln_positions = match *object {
                Object::Regular { .. } => None,
                Object::LongNote {
                    start: start_timestamp,
                    end,
                } => Some((
                    start_timestamp.no_scroll_speed_change_position(),
                    end.no_scroll_speed_change_position(),
                )),
            };

            (start, ln_positions)
        } else {
            let start = match *object {
                Object::Regular { .. } => object_cache.start_position(),
                Object::LongNote { start, end } => match *object_state {
                    ObjectState::LongNote(LongNoteState::Held { .. }) => self
                        .state
                        .position_at_time(self.map_timestamp.min(end).max(start)),

                    ObjectState::LongNote(LongNoteState::Missed {
                        held_until: Some(held_until),
                        ..
                    }) => self.state.position_at_time(held_until.max(start)),

                    _ => object_cache.start_position(),
                },
            };

            let ln_positions = match *object_cache {
                ObjectCache::Regular(_) => None,
                ObjectCache::LongNote(LongNoteCache {
                    end_position,
                    start_position,
                }) => Some((start_position, end_position)),
            };

            (start, ln_positions)
        };

        let pos = Point2::new(
            -self.border_offset + self.lane_width * lane as f32,
            self.core_to_screen(start),
        );

        let height = if let Some((ln_start, ln_end)) = ln_positions {
            let max_height =
                to_core_position_difference(self.note_height / 2.) / self.state.scroll_speed;
            let capped_end = ln_start + (ln_end - ln_start).max(max_height);
            from_core_position_difference((capped_end - start) * self.state.scroll_speed)
        } else {
            self.note_height
        };

        let mut color = if self.state.lane_states.len() == 4 {
            if lane == 0 || lane == 3 {
                Srgba::new(1., 1., 1., 1.)
            } else {
                Srgba::new(0., 0.5, 1., 1.)
            }
        } else {
            if self.state.lane_states.len() % 2 == 1 && lane == self.state.lane_states.len() / 2 {
                Srgba::new(1., 1., 0., 1.)
            } else if lane % 2 == 0 {
                Srgba::new(1., 1., 1., 1.)
            } else {
                Srgba::new(0., 0.5, 1., 1.)
            }
        };

        if let ObjectState::LongNote(LongNoteState::Missed { .. }) = *object_state {
            color.color = color.color.component_wise_self(|x| x * 0.5);
        }

        Sprite {
            pos,
            scale: Vector2::new(self.lane_width, height),
            color,
        }
    }

    fn push_judgement_line(&mut self) {
        self.renderer.sprites.push(Sprite {
            pos: Point2::new(
                -self.border_offset,
                self.judgement_line_position - self.border_width,
            ),
            scale: Vector2::new(self.border_offset * 2., self.border_width),
            color: Srgba::new(1., 1., 1., 1.),
        });
    }

    fn push_error_bar(&mut self) {
        let error_bar_width = 0.5;
        let error_bar_offset = error_bar_width / 2.;
        let error_bar_position = self.renderer.ortho.bottom + 1.;
        let error_bar_height = 0.003;
        let error_bar_hit_height = 0.05;
        let error_bar_hit_width = 0.01;
        let error_bar_perfect_hit_width = 0.003;

        let zero_error_bar_hit_position = -error_bar_hit_width / 2.;
        let rightmost_error_bar_hit_position = error_bar_offset - error_bar_hit_width;
        let highest_hit_difference = GameTimestampDifference::from_millis(76);
        let highest_hit_difference = highest_hit_difference.into_milli_hundredths() as f32;
        let hit_difference_offset_factor = (rightmost_error_bar_hit_position
            - zero_error_bar_hit_position)
            / highest_hit_difference;

        self.renderer.sprites.push(Sprite {
            pos: Point2::new(
                -error_bar_offset,
                error_bar_position - error_bar_height / 2.,
            ),
            scale: Vector2::new(error_bar_width, error_bar_height),
            color: Srgba::new(1., 1., 1., 0.1),
        });

        for Hit {
            timestamp,
            difference,
        } in self.state.last_hits.iter()
        {
            let offset = difference.into_milli_hundredths() as f32 * hit_difference_offset_factor
                + zero_error_bar_hit_position;
            let alpha = (0.3
                - (self.game_timestamp - *timestamp).into_milli_hundredths() as f32 / 500_000.)
                .max(0.);

            self.renderer.sprites.push(Sprite {
                pos: Point2::new(offset, error_bar_position - error_bar_hit_height / 2.),
                scale: Vector2::new(error_bar_hit_width, error_bar_hit_height),
                color: Srgba::new(1., 1., 1., alpha),
            });
        }

        self.renderer.sprites.push(Sprite {
            pos: Point2::new(
                -error_bar_perfect_hit_width / 2.,
                error_bar_position - error_bar_hit_height / 2.,
            ),
            scale: Vector2::new(error_bar_perfect_hit_width, error_bar_hit_height),
            color: Srgba::new(1., 0., 0., 1.),
        });
    }

    fn push_timeline(&mut self) {
        let first_timestamp = self.state.first_timestamp();
        if first_timestamp.is_none() {
            return;
        }
        let first_timestamp = first_timestamp.unwrap();
        let last_timestamp = self.state.last_timestamp().unwrap();

        let total_difference = last_timestamp - first_timestamp;
        let current_difference =
            self.map_timestamp.max(first_timestamp).min(last_timestamp) - first_timestamp;

        let total_width = self.renderer.ortho.right - self.renderer.ortho.left;
        let width = f64::from(current_difference.into_milli_hundredths())
            * (f64::from(total_width) / f64::from(total_difference.into_milli_hundredths()));

        self.renderer.sprites.push(Sprite {
            pos: Point2::new(self.renderer.ortho.left, self.renderer.ortho.bottom),
            scale: Vector2::new(width as f32, self.border_width * 3.),
            color: Srgba::new(1., 1., 1., 1.),
        });
    }
}
