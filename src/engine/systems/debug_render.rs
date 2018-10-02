use ordered_float::OrderedFloat;
use std::collections::HashMap;
use engine::ChunkPos;
use cgmath::Deg;
use specs::shred::PanicHandler;
use shrev::EventChannel;
use shrev::ReaderId;
use gl_api::buffer::{VertexBuffer, UsageType};
use gl_api::shader::{simple_pipeline, program::LinkedProgram};
use gl_api::vertex_array::VertexArray;
use cgmath::{Point3, Vector3, Vector4};
use collision::Aabb3;
use engine::components as comp;
use engine::resources as res;
use specs::prelude::*;
use engine::world::chunk::SIZE;

vertex! {
    vertex DebugVertex {
        pos: Point3<f64>,
        color: Vector4<f64>,
    }
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub enum Shape {
    GriddedChunk(f64, ChunkPos, Vector4<f64>),
    Chunk(f64, ChunkPos, Vector4<f64>),
    Box(f64, Aabb3<f64>, Vector4<f64>),
    Line(f64, Point3<f64>, Vector3<f64>, Vector4<f64>),
}

pub struct DebugRenderer {
    program: LinkedProgram,
    vao: VertexArray,
    vbo: VertexBuffer<DebugVertex>,
    geometry: HashMap<OrderedFloat<f64>, Vec<DebugVertex>>,
    request_rx: ReaderId<Shape>,
}

impl DebugRenderer {
    pub fn new(request_rx: ReaderId<Shape>) -> Self {
        let program = simple_pipeline("resources/debug.vs", "resources/debug.fs").unwrap();
        let mut vao = VertexArray::new();
        let vbo = VertexBuffer::new();
        vao.add_buffer(&vbo).unwrap();

        DebugRenderer {
            program, vao, vbo, request_rx, geometry: HashMap::new()
        }
    }

    fn add_line(&mut self, start: Point3<f64>, end: Point3<f64>, color: Vector4<f64>, weight: f64) {
        self.geometry.entry(OrderedFloat(weight)).or_default().push(DebugVertex { pos: start, color });
        self.geometry.entry(OrderedFloat(weight)).or_default().push(DebugVertex { pos: end, color });
    }

    fn add_box(&mut self, b: Aabb3<f64>, color: Vector4<f64>, weight: f64) {
        let len_x = b.max.x - b.min.x;
        let len_y = b.max.y - b.min.y;
        let len_z = b.max.z - b.min.z;

        let y_lnn = b.min;
        let y_lnp = b.min + Vector3::new(0.0, 0.0, len_z);
        let y_lpn = b.min + Vector3::new(len_x, 0.0, 0.0);
        let y_lpp = b.min + Vector3::new(len_x, 0.0, len_z);

        let y_hnn = b.min + Vector3::new(0.0, len_y, 0.0);
        let y_hnp = b.min + Vector3::new(0.0, len_y, len_z);
        let y_hpn = b.min + Vector3::new(len_x, len_y, 0.0);
        let y_hpp = b.min + Vector3::new(len_x, len_y, len_z);

        self.add_line(y_lnn, y_lnp, color, weight);
        self.add_line(y_lnp, y_lpp, color, weight);
        self.add_line(y_lpp, y_lpn, color, weight);
        self.add_line(y_lpn, y_lnn, color, weight);

        self.add_line(y_hnn, y_hnp, color, weight);
        self.add_line(y_hnp, y_hpp, color, weight);
        self.add_line(y_hpp, y_hpn, color, weight);
        self.add_line(y_hpn, y_hnn, color, weight);

        self.add_line(y_lnn, y_hnn, color, weight);
        self.add_line(y_lnp, y_hnp, color, weight);
        self.add_line(y_lpp, y_hpp, color, weight);
        self.add_line(y_lpn, y_hpn, color, weight);
    }
}

impl<'a> System<'a> for DebugRenderer {
    type SystemData = (
        Read<'a, EventChannel<Shape>>,
        ReadStorage<'a, comp::Transform>,
        ReadStorage<'a, comp::Player>,
        ReadStorage<'a, comp::ClientControlled>,
        Read<'a, res::ViewFrustum, PanicHandler>,
        Read<'a, res::FramebufferSize>,
    );

    fn run(&mut self, (channel, transforms, player_marker, client_controlled_marker, frustum, framebuffer_size): Self::SystemData) {
        self.geometry.clear();
        for shape in channel.read(&mut self.request_rx) {
            match shape {
                &Shape::Box(weight, b, color) => {
                    self.add_box(b, color, weight);
                }

                &Shape::Chunk(weight, pos, color) => {
                    let fsize = SIZE as f64;
                    let base = fsize * pos.cast().unwrap();
                    self.add_box(Aabb3::new(base, base + Vector3::new(fsize, fsize, fsize)), color, weight);
                }

                &Shape::GriddedChunk(weight, pos, color) => {
                    let fsize = SIZE as f64;
                    let base = fsize * pos.cast().unwrap();
                    self.add_box(Aabb3::new(base, base + Vector3::new(fsize, fsize, fsize)), color, weight);

                    for x in 0..SIZE {
                        let y_nn = base + Vector3::new(x as f64, 0.0, 0.0);
                        let y_np = base + Vector3::new(x as f64, 0.0, fsize);
                        let y_pn = base + Vector3::new(x as f64, fsize, 0.0);
                        let y_pp = base + Vector3::new(x as f64, fsize, fsize);

                        self.add_line(y_nn, y_np, Vector4::new(1.0, 0.5, 0.5, 1.0), weight / 2.0);
                        self.add_line(y_np, y_pp, Vector4::new(1.0, 0.5, 0.5, 1.0), weight / 2.0);
                        self.add_line(y_pp, y_pn, Vector4::new(1.0, 0.5, 0.5, 1.0), weight / 2.0);
                        self.add_line(y_pn, y_nn, Vector4::new(1.0, 0.5, 0.5, 1.0), weight / 2.0);
                    }
                    
                    for y in 0..SIZE {
                        let y_nn = base + Vector3::new(0.0, y as f64, 0.0);
                        let y_np = base + Vector3::new(0.0, y as f64, fsize);
                        let y_pn = base + Vector3::new(fsize, y as f64, 0.0);
                        let y_pp = base + Vector3::new(fsize, y as f64, fsize);

                        self.add_line(y_nn, y_np, Vector4::new(0.5, 1.0, 0.5, 1.0), weight / 2.0);
                        self.add_line(y_np, y_pp, Vector4::new(0.5, 1.0, 0.5, 1.0), weight / 2.0);
                        self.add_line(y_pp, y_pn, Vector4::new(0.5, 1.0, 0.5, 1.0), weight / 2.0);
                        self.add_line(y_pn, y_nn, Vector4::new(0.5, 1.0, 0.5, 1.0), weight / 2.0);
                    }

                    for z in 0..SIZE {
                        let y_nn = base + Vector3::new(0.0, 0.0, z as f64);
                        let y_np = base + Vector3::new(0.0, fsize, z as f64);
                        let y_pn = base + Vector3::new(fsize, 0.0, z as f64);
                        let y_pp = base + Vector3::new(fsize, fsize, z as f64);

                        self.add_line(y_nn, y_np, Vector4::new(0.5, 0.5, 1.0, 1.0), weight / 2.0);
                        self.add_line(y_np, y_pp, Vector4::new(0.5, 0.5, 1.0, 1.0), weight / 2.0);
                        self.add_line(y_pp, y_pn, Vector4::new(0.5, 0.5, 1.0, 1.0), weight / 2.0);
                        self.add_line(y_pn, y_nn, Vector4::new(0.5, 0.5, 1.0, 1.0), weight / 2.0);
                    }
                }

                &Shape::Line(weight, start, end, color) => self.add_line(start, start + end, color, weight),
            }
        }

        let player_transform = (&player_marker, &client_controlled_marker, &transforms).join().map(|(_, _, tfm)| tfm).next();

        if let Some(tfm) = player_transform {
            let aspect_ratio = framebuffer_size.x as f32 / framebuffer_size.y as f32;
            let view_matrix = tfm.as_matrix().cast::<f32>().unwrap();
            let projection = ::cgmath::perspective(Deg(frustum.fov.0 as f32), aspect_ratio, frustum.near_plane as f32, frustum.far_plane as f32);
            self.program.set_uniform("view", &view_matrix);
            self.program.set_uniform("projection", &projection);

            for (weight, geom) in &self.geometry {
                self.vbo.upload(geom, UsageType::DynamicDraw).unwrap();
                unsafe {
                    self.vao.bind();
                    self.vbo.bind();
                    self.program.bind();
                    gl_call!(LineWidth(weight.0 as f32)).unwrap();
                    gl_call!(DrawArrays(gl::LINES, 0, self.vbo.len() as i32)).unwrap();
                }
            }
        }

    }
}
