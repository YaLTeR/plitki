use std::{
    rc::Rc,
    time::{Duration, Instant},
};

use cgmath::{ortho, Matrix4};
use glium::{
    backend::Context, implement_vertex, index::PrimitiveType, uniform, Frame, IndexBuffer, Program,
    Surface, VertexBuffer,
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
    offset: [f32; 2],
    color: [f32; 4],
}

implement_vertex!(InstanceData, offset, color);

pub struct Renderer {
    context: Rc<Context>,
    program: Program,
    vertex_buffer: VertexBuffer<Vertex>,
    index_buffer: IndexBuffer<u8>,
    dimensions: (u32, u32),
    projection: Matrix4<f32>,
    offset_vertex_buffer: Option<VertexBuffer<InstanceData>>,
}

impl Renderer {
    pub fn new(context: Rc<Context>, dimensions: (u32, u32)) -> Self {
        let shape = [
            Vertex {
                position: [-1., 0.],
            },
            Vertex { position: [1., 0.] },
            Vertex { position: [1., 1.] },
            Vertex {
                position: [-1., 1.],
            },
        ];
        let vertex_buffer = VertexBuffer::new(&context, &shape).unwrap();
        let indices = [0, 1, 2, 3];
        let index_buffer =
            IndexBuffer::new(&context, PrimitiveType::TriangleFan, &indices).unwrap();

        let vertex_shader_src = r#"
            #version 140

            // Vertex coordinates, before scaling.
            in vec2 position;
            // Offset.
            // X: after scaling, used for splitting objects in lanes.
            // Y: in seconds, so it must be multiplied by note_speed.
            // TODO: precision and stuff? This should ideally be computed only for objects visible
            // on screen and with maximal precision, otherwise stuff like ultra slow SVs long into
            // the map will break (and those SVs are the exact reason timestamps are integers with
            // high precision).
            in vec2 offset;
            // Vertex color
            in vec4 color;

            // Vertex color (output)
            out vec4 vertex_color;

            // Time in seconds, must be multiplied by note_speed.
            uniform float time;
            // Note speed in Y coordinates per second (*half* of a 1:1 screen).
            uniform float note_speed;
            // In coordinates after scaling.
            uniform float bottom;
            // Scale for vertex coordinates.
            uniform float scale;
            // Projection matrix, at 1:1 both coordinates should range from -1 to 1.
            uniform mat4 projection;

            void main() {
                mat4 model = mat4(
                    scale,    0.0,                                     0.0, 0.0,
                    0.0,      scale,                                   0.0, 0.0,
                    0.0,      0.0,                                     0.0, 0.0,
                    offset.x, (offset.y - time) * note_speed + bottom, 0.0, 1.0
                );
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

        let (w, h) = (dimensions.0 as f32, dimensions.1 as f32);
        let projection = ortho(-w / 2., w / 2., -h / 2., h / 2., -1., 1.);

        Self {
            context,
            program,
            vertex_buffer,
            index_buffer,
            dimensions,
            projection,
            offset_vertex_buffer: None,
        }
    }

    pub fn render(&mut self, dimensions: (u32, u32), elapsed: Duration, state: &GameState) {
        let start = Instant::now();

        if dimensions != self.dimensions {
            self.dimensions = dimensions;
            let (w, h) = (dimensions.0 as f32, dimensions.1 as f32);
            let aspect_ratio = w / h;
            let (x, y) = if aspect_ratio > 1. {
                (aspect_ratio, 1.)
            } else {
                (1., 1. / aspect_ratio)
            };
            self.projection = ortho(-x, x, -y, y, -1., 1.);
        }
        let aspect_ratio = self.dimensions.0 as f32 / self.dimensions.1 as f32;
        let bottom = if aspect_ratio > 1. {
            -1.
        } else {
            -1. / aspect_ratio
        };

        let scale = 0.1f32;

        if self.offset_vertex_buffer.is_none() {
            let object_count = state.map.lanes.iter().map(|x| x.objects.len()).sum();

            let mut buffer = VertexBuffer::empty(&self.context, object_count).unwrap();
            {
                let mut map = buffer.as_mut_slice().map_write();
                for (i, (lane, object)) in state
                    .map
                    .lanes
                    .iter()
                    .enumerate()
                    .flat_map(|(lane, x)| x.objects.iter().map(move |x| (lane, x)))
                    .enumerate()
                {
                    let offset = [
                        (lane as f32 - 1.5) * scale * 2.,
                        (object.timestamp().0).0 as f32 / 100_000., // In seconds.
                    ];
                    let color = if lane == 0 || lane == 3 {
                        [0.1, 0.1, 0.1, 0.1]
                    } else {
                        [0.00, 0.05, 0.1, 0.1]
                    };
                    map.set(i, InstanceData { offset, color });
                }
            }

            self.offset_vertex_buffer = Some(buffer);
        }

        let mut frame = Frame::new(self.context.clone(), dimensions);

        if state.cap_fps {
            frame.clear_color(0.2, 0., 0., 1.);
        } else {
            frame.clear_color(0., 0., 0., 1.);
        }

        let elapsed_seconds = elapsed.as_secs() as f32 + elapsed.subsec_nanos() as f32 * 1e-9;

        let projection: [[f32; 4]; 4] = self.projection.into();
        frame
            .draw(
                (
                    &self.vertex_buffer,
                    self.offset_vertex_buffer
                        .as_ref()
                        .unwrap()
                        .per_instance()
                        .unwrap(),
                ),
                &self.index_buffer,
                &self.program,
                &uniform! {
                    time: elapsed_seconds,
                    scale: scale,
                    note_speed: f32::from(state.scroll_speed) / 5.,
                    bottom: bottom,
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
