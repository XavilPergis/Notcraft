use crate::engine::{camera::Camera, prelude::*, world::chunk::SIZE};
use cgmath::Deg;
use collision::{Aabb3, Ray3};
use glium::{
    index::{NoIndices, PrimitiveType},
    *,
};
use ordered_float::OrderedFloat;
use specs::shred::PanicHandler;
use std::{
    collections::{HashMap, HashSet},
    rc::Rc,
    sync::{Mutex, MutexGuard},
};

#[derive(Copy, Clone, Debug)]
struct DebugVertex {
    pos: [f32; 3],
    color: [f32; 4],
}

glium::implement_vertex!(DebugVertex, pos, color);

#[derive(Copy, Clone, Debug, PartialEq)]
pub enum Shape {
    GriddedChunk(f32, ChunkPos, Vector4<f32>),
    Chunk(f32, ChunkPos, Vector4<f32>),
    Box(f32, Aabb3<f32>, Vector4<f32>),
    Block(f32, BlockPos, Vector4<f32>),
    Ray(f32, Ray3<f32>, Vector4<f32>),
    Line(f32, WorldPos, Vector3<f32>, Vector4<f32>),
}

pub enum DebugSection<'a> {
    Disabled,
    Enabled(MutexGuard<'a, Vec<Shape>>),
}

impl<'a> DebugSection<'a> {
    pub fn draw(&mut self, shape: Shape) {
        match self {
            DebugSection::Enabled(buf) => buf.push(shape),
            _ => {}
        }
    }
}

#[derive(Debug, Default)]
pub struct DebugAccumulator {
    shape_buffer: Mutex<Vec<Shape>>,
    enabled: HashSet<String>,
}

impl DebugAccumulator {
    pub fn section(&self, name: &str) -> DebugSection {
        if self.enabled.contains(name) {
            self.shape_buffer
                .try_lock()
                .map(|guard| DebugSection::Enabled(guard))
                .unwrap_or(DebugSection::Disabled)
        } else {
            DebugSection::Disabled
        }
    }

    pub fn shapes_mut(&mut self) -> &mut Vec<Shape> {
        self.shape_buffer.get_mut().unwrap()
    }

    pub fn enable(&mut self, name: String) {
        self.enabled.insert(name);
    }

    pub fn toggle(&mut self, name: String) {
        if self.enabled.contains(&name) {
            self.enabled.remove(&name);
        } else {
            self.enabled.insert(name);
        }
    }
}

pub struct DebugRenderer {
    ctx: Display,
    program: Program,
    buffer: VertexBuffer<DebugVertex>,
    geometry: HashMap<OrderedFloat<f32>, Vec<DebugVertex>>,
}

impl DebugRenderer {
    pub fn new(ctx: &Display) -> std::io::Result<Self> {
        let program = Program::from_source(
            ctx,
            &util::read_file("resources/shaders/debug.vs")?,
            &util::read_file("resources/shaders/debug.fs")?,
            None,
        )
        .unwrap();

        Ok(DebugRenderer {
            ctx: ctx.clone(),
            geometry: HashMap::new(),
            program,
            buffer: VertexBuffer::empty_dynamic(ctx, 0).unwrap(),
        })
    }

    fn add_line(&mut self, start: WorldPos, end: WorldPos, color: Vector4<f32>, weight: f32) {
        self.geometry
            .entry(OrderedFloat(weight))
            .or_default()
            .push(DebugVertex {
                pos: start.0.into(),
                color: color.into(),
            });
        self.geometry
            .entry(OrderedFloat(weight))
            .or_default()
            .push(DebugVertex {
                pos: end.0.into(),
                color: color.into(),
            });
    }

    fn add_box(&mut self, b: Aabb3<f32>, color: Vector4<f32>, weight: f32) {
        let len_x = b.max.x - b.min.x;
        let len_y = b.max.y - b.min.y;
        let len_z = b.max.z - b.min.z;

        let bmin = WorldPos(b.min);

        let y_lnn = bmin;
        let y_lnp = bmin.offset((0.0, 0.0, len_z));
        let y_lpn = bmin.offset((len_x, 0.0, 0.0));
        let y_lpp = bmin.offset((len_x, 0.0, len_z));
        let y_hnn = bmin.offset((0.0, len_y, 0.0));
        let y_hnp = bmin.offset((0.0, len_y, len_z));
        let y_hpn = bmin.offset((len_x, len_y, 0.0));
        let y_hpp = bmin.offset((len_x, len_y, len_z));

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

    pub fn draw<S: Surface>(
        &mut self,
        surface: &mut S,
        accumulator: &mut DebugAccumulator,
        camera: Camera,
    ) {
        self.geometry.clear();
        for shape in accumulator.shapes_mut().drain(..) {
            match shape {
                Shape::Box(weight, b, color) => {
                    self.add_box(b, color, weight);
                }

                Shape::Block(weight, pos, color) => {
                    self.add_box(pos.aabb(), color, weight);
                }

                Shape::Ray(weight, ray, color) => {
                    self.add_line(
                        WorldPos(ray.origin),
                        WorldPos(ray.origin).offset(ray.direction),
                        color,
                        weight,
                    );
                }

                Shape::Chunk(weight, pos, color) => {
                    let fsize = SIZE as f32;
                    let base = pos.base().base().0;
                    self.add_box(
                        Aabb3::new(base, base + Vector3::new(fsize, fsize, fsize)),
                        color,
                        weight,
                    );
                }

                Shape::GriddedChunk(weight, pos, color) => {
                    let fsize = SIZE as f32;
                    let base = pos.base().base();
                    self.add_box(
                        Aabb3::new(base.0, base.0 + Vector3::new(fsize, fsize, fsize)),
                        color,
                        weight,
                    );

                    for n in 0..SIZE {
                        let n = n as f32;
                        let x_nn = base.offset((n, 0.0, 0.0));
                        let x_np = base.offset((n, 0.0, fsize));
                        let x_pn = base.offset((n, fsize, 0.0));
                        let x_pp = base.offset((n, fsize, fsize));
                        let y_nn = base.offset((0.0, n, 0.0));
                        let y_np = base.offset((0.0, n, fsize));
                        let y_pn = base.offset((fsize, n, 0.0));
                        let y_pp = base.offset((fsize, n, fsize));
                        let z_nn = base.offset((0.0, 0.0, n));
                        let z_np = base.offset((0.0, fsize, n));
                        let z_pn = base.offset((fsize, 0.0, n));
                        let z_pp = base.offset((fsize, fsize, n));

                        self.add_line(x_nn, x_np, Vector4::new(1.0, 0.5, 0.5, 1.0), weight / 2.0);
                        self.add_line(x_np, x_pp, Vector4::new(1.0, 0.5, 0.5, 1.0), weight / 2.0);
                        self.add_line(x_pp, x_pn, Vector4::new(1.0, 0.5, 0.5, 1.0), weight / 2.0);
                        self.add_line(x_pn, x_nn, Vector4::new(1.0, 0.5, 0.5, 1.0), weight / 2.0);

                        self.add_line(y_nn, y_np, Vector4::new(0.5, 1.0, 0.5, 1.0), weight / 2.0);
                        self.add_line(y_np, y_pp, Vector4::new(0.5, 1.0, 0.5, 1.0), weight / 2.0);
                        self.add_line(y_pp, y_pn, Vector4::new(0.5, 1.0, 0.5, 1.0), weight / 2.0);
                        self.add_line(y_pn, y_nn, Vector4::new(0.5, 1.0, 0.5, 1.0), weight / 2.0);

                        self.add_line(z_nn, z_np, Vector4::new(0.5, 0.5, 1.0, 1.0), weight / 2.0);
                        self.add_line(z_np, z_pp, Vector4::new(0.5, 0.5, 1.0, 1.0), weight / 2.0);
                        self.add_line(z_pp, z_pn, Vector4::new(0.5, 0.5, 1.0, 1.0), weight / 2.0);
                        self.add_line(z_pn, z_nn, Vector4::new(0.5, 0.5, 1.0, 1.0), weight / 2.0);
                    }
                }

                Shape::Line(weight, start, end, color) => {
                    self.add_line(start, start.offset(end), color, weight)
                }
            }
        }

        surface
            .draw(
                &self.buffer,
                &NoIndices(PrimitiveType::LinesList),
                &self.program,
                &glium::uniform! {},
                &DrawParameters {
                    depth: Depth {
                        test: DepthTest::IfLess,
                        write: true,
                        ..Default::default()
                    },
                    ..Default::default()
                },
            )
            .unwrap();

        // let player_transform = (&player_marker, &client_controlled_marker,
        // &transforms)     .join()
        //     .map(|(_, _, tfm)| tfm)
        //     .next();

        // if let Some(tfm) = player_transform {
        //     let view_matrix = camera.view_matrix().cast::<f32>().unwrap();
        //     let projection: Matrix4<f32> =
        // camera.projection_matrix().cast().unwrap();     self.program
        //         .set_uniform(&mut self.ctx, "view", &view_matrix);
        //     self.program
        //         .set_uniform(&mut self.ctx, "projection", &projection);

        //     for (weight, geom) in &self.geometry {
        //         self.buffer.write(geom);
        //     }
        // }
    }
}
