use std::{
    convert::{identity, TryInto},
    rc::Rc,
    time::{Duration, Instant},
};

use cgmath::{Matrix4, Ortho, Point2, Vector2};
use glium::{
    backend::Context, implement_vertex, index::PrimitiveType, uniform, Frame, IndexBuffer, Program,
    Surface, VertexBuffer,
};
use palette::Srgba;
use plitki_core::{
    object::Object,
    state::{LongNoteState, ObjectState},
    timing::GameTimestamp,
};
use slog_scope::{debug, trace};

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

    pub fn render(&mut self, dimensions: (u32, u32), elapsed: Duration, state: &GameState) {
        let start = Instant::now();

        if dimensions != self.dimensions {
            self.dimensions = dimensions;
            self.ortho = compute_ortho(dimensions);
            self.projection = self.ortho.into();
        }

        self.sprites.clear();

        let lane_width = 0.2;
        let lane_count = state.map.lanes.len();
        let border_offset = lane_width * lane_count as f32 / 2.;
        let border_width = 0.01;
        let judgement_line_position = self.ortho.bottom + 0.4;

        // Left lane border.
        self.sprites.push(Sprite {
            pos: Point2::new(-border_offset - border_width, self.ortho.bottom),
            scale: Vector2::new(border_width, self.ortho.top - self.ortho.bottom),
            color: Srgba::new(1., 1., 1., 1.),
        });
        // Right lane border.
        self.sprites.push(Sprite {
            pos: Point2::new(border_offset, self.ortho.bottom),
            scale: Vector2::new(border_width, self.ortho.top - self.ortho.bottom),
            color: Srgba::new(1., 1., 1., 1.),
        });

        let elapsed_timestamp = GameTimestamp(elapsed.try_into().unwrap());

        #[allow(clippy::inconsistent_digit_grouping)]
        let to_scroll_speed_coord = |x| x * 5.;
        #[allow(clippy::inconsistent_digit_grouping)]
        let from_scroll_speed_coord = |x| x / 5.;

        let note_height = 0.1;
        let first_visible_timestamp = state.game_to_map(
            elapsed_timestamp
                - to_scroll_speed_coord(judgement_line_position - self.ortho.bottom + note_height)
                    / state.scroll_speed,
        );
        let one_past_last_visible_timestamp = state.game_to_map(
            elapsed_timestamp
                + to_scroll_speed_coord(self.ortho.top - judgement_line_position)
                    / state.scroll_speed,
        );

        for (lane, objects, object_states) in (0..state.map.lanes.len()).map(|lane| {
            (
                lane,
                &state.map.lanes[lane].objects[..],
                &state.lane_states[lane].object_states[..],
            )
        }) {
            let first_visible_index = objects
                .binary_search_by_key(&first_visible_timestamp, Object::end_timestamp)
                .unwrap_or_else(identity);
            let one_past_last_visible_index = objects
                .binary_search_by_key(&one_past_last_visible_timestamp, Object::start_timestamp)
                .unwrap_or_else(identity);

            let range = first_visible_index..one_past_last_visible_index;
            for (object, object_state) in objects[range.clone()]
                .iter()
                .zip(object_states[range].iter())
                .rev()
                .filter(|(_, s)| !s.is_hit())
            {
                let pos = Point2::new(
                    -border_offset + lane_width * lane as f32,
                    judgement_line_position
                        + from_scroll_speed_coord(
                            (state.map_to_game(object.start_timestamp()) - elapsed_timestamp)
                                * state.scroll_speed,
                        ),
                );

                let height = match *object {
                    Object::Regular { .. } => note_height,
                    Object::LongNote { start, end } => {
                        from_scroll_speed_coord(state.map_to_game(end - start) * state.scroll_speed)
                    }
                };

                let color = match object_state {
                    ObjectState::LongNote {
                        state: LongNoteState::Held,
                    } => Srgba::new(0.1, 0.1, 0.0, 0.1),
                    ObjectState::LongNote {
                        state: LongNoteState::Missed,
                    } => Srgba::new(0.1, 0.0, 0.0, 0.1),
                    _ => Srgba::new(0.1, 0.1, 0.1, 0.1),
                };
                // let color = if lane == 0 || lane == 3 {
                //     Srgba::new(0.1, 0.1, 0.1, 0.1)
                // } else {
                //     Srgba::new(0.00, 0.05, 0.1, 0.1)
                // };

                self.sprites.push(Sprite {
                    pos,
                    scale: Vector2::new(lane_width, height),
                    color,
                });
            }
        }

        // Judgement line.
        self.sprites.push(Sprite {
            pos: Point2::new(-border_offset, judgement_line_position),
            scale: Vector2::new(border_offset * 2., border_width),
            color: Srgba::new(1., 1., 1., 1.),
        });

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
                &Default::default(),
            )
            .unwrap();

        if state.cap_fps {
            std::thread::sleep(Duration::from_millis(1000));
        }

        frame.finish().unwrap();

        trace!("finished redraw"; "time_taken" => ?(Instant::now() - start));
    }
}
