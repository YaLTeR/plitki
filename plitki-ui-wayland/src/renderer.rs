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
use palette::{ComponentWise, Srgba};
use plitki_core::{
    object::Object,
    state::{LongNoteState, ObjectState},
    timing::{GameTimestamp, MapTimestamp},
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

pub struct SingleFrameRenderer<'a> {
    renderer: &'a mut Renderer,
    state: &'a GameState,
    elapsed_timestamp: GameTimestamp,
    lane_width: f32,
    border_offset: f32,
    border_width: f32,
    judgement_line_position: f32,
    note_height: f32,
    first_visible_timestamp: MapTimestamp,
    one_past_last_visible_timestamp: MapTimestamp,
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

fn to_scroll_speed_coord(x: f32) -> f32 {
    x * 5.
}

fn from_scroll_speed_coord(x: f32) -> f32 {
    x / 5.
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

        {
            let mut renderer = SingleFrameRenderer::new(self, elapsed, state);
            renderer.push_borders();
            renderer.push_objects();
            renderer.push_judgement_line();
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

impl<'a> SingleFrameRenderer<'a> {
    fn new(renderer: &'a mut Renderer, elapsed: Duration, state: &'a GameState) -> Self {
        let elapsed_timestamp = GameTimestamp(elapsed.try_into().unwrap());

        let lane_width = 0.2;
        let lane_count = state.map.lanes.len();
        let border_offset = lane_width * lane_count as f32 / 2.;
        let border_width = 0.01;
        let judgement_line_position = renderer.ortho.bottom + 0.4;
        let note_height = 0.1;

        let first_visible_timestamp = (elapsed_timestamp
            - to_scroll_speed_coord(judgement_line_position - renderer.ortho.bottom + note_height)
                / state.scroll_speed)
            .to_map(state);
        let one_past_last_visible_timestamp = (elapsed_timestamp
            + to_scroll_speed_coord(renderer.ortho.top - judgement_line_position)
                / state.scroll_speed)
            .to_map(state);

        renderer.sprites.clear();

        Self {
            renderer,
            state,
            elapsed_timestamp,
            lane_width,
            border_offset,
            border_width,
            judgement_line_position,
            note_height,
            first_visible_timestamp,
            one_past_last_visible_timestamp,
        }
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

    fn push_objects(&mut self) {
        // Yay, partial borrowing to win vs. the borrow checker...
        let state = self.state;

        for (lane, objects, object_states) in (0..self.state.map.lanes.len()).map(|lane| {
            (
                lane,
                &state.map.lanes[lane].objects[..],
                &state.lane_states[lane].object_states[..],
            )
        }) {
            let first_visible_index = objects
                .binary_search_by_key(&self.first_visible_timestamp, Object::end_timestamp)
                .unwrap_or_else(identity);
            let one_past_last_visible_index = objects
                .binary_search_by_key(
                    &self.one_past_last_visible_timestamp,
                    Object::start_timestamp,
                )
                .unwrap_or_else(identity);

            let range = first_visible_index..one_past_last_visible_index;
            for (object, object_state) in objects[range.clone()]
                .iter()
                .zip(object_states[range].iter())
                .rev()
                .filter(|(_, s)| !s.is_hit())
            {
                self.renderer
                    .sprites
                    .push(self.object_sprite(lane, object, object_state));
            }
        }
    }

    fn object_sprite(&self, lane: usize, object: &Object, object_state: &ObjectState) -> Sprite {
        let start = match *object {
            Object::Regular { .. } => object.start_timestamp(),
            Object::LongNote { start, end } => match *object_state {
                ObjectState::LongNote(LongNoteState::Held) => self
                    .elapsed_timestamp
                    .to_map(self.state)
                    .min(end)
                    .max(start),

                ObjectState::LongNote(LongNoteState::Missed {
                    held_until: Some(held_until),
                }) => held_until.max(start),

                _ => start,
            },
        };

        let pos = Point2::new(
            -self.border_offset + self.lane_width * lane as f32,
            self.judgement_line_position
                + from_scroll_speed_coord(
                    (self.state.map_to_game(start) - self.elapsed_timestamp)
                        * self.state.scroll_speed,
                ),
        );

        let height = match *object {
            Object::Regular { .. } => self.note_height,
            Object::LongNote { end, .. } => {
                from_scroll_speed_coord((end - start).to_game(self.state) * self.state.scroll_speed)
            }
        };

        let mut color = if lane == 0 || lane == 3 {
            Srgba::new(0.5, 0.5, 0.5, 0.5)
        } else {
            Srgba::new(0.00, 0.25, 0.5, 0.5)
        };

        if let ObjectState::LongNote(LongNoteState::Missed { .. }) = *object_state {
            color = color.component_wise_self(|x| x * 0.1);
        }

        Sprite {
            pos,
            scale: Vector2::new(self.lane_width, height),
            color,
        }
    }

    fn push_judgement_line(&mut self) {
        self.renderer.sprites.push(Sprite {
            pos: Point2::new(-self.border_offset, self.judgement_line_position),
            scale: Vector2::new(self.border_offset * 2., self.border_width),
            color: Srgba::new(1., 1., 1., 1.),
        });
    }
}
