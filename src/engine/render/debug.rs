use cgmath::Deg;
use collision::{Aabb3, Ray3};
use engine::{prelude::*, world::chunk::SIZE};
use gl_api::{
    buffer::{Buffer, UsageType},
    context::Context,
    shader::{program::LinkedProgram, simple_pipeline},
    PrimitiveType,
};
use ordered_float::OrderedFloat;
use specs::shred::PanicHandler;
use std::{
    collections::{HashMap, HashSet},
    sync::{Mutex, MutexGuard},
};

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
    Block(f64, BlockPos, Vector4<f64>),
    Ray(f64, Ray3<f64>, Vector4<f64>),
    Line(f64, WorldPos, Vector3<f64>, Vector4<f64>),
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
    ctx: Context,
    program: LinkedProgram,
    vbo: Buffer<DebugVertex>,
    geometry: HashMap<OrderedFloat<f64>, Vec<DebugVertex>>,
}

impl DebugRenderer {
    pub fn new(ctx: &Context) -> (Self, DebugAccumulator) {
        let program = simple_pipeline("resources/debug.vs", "resources/debug.fs").unwrap();
        let vbo = Buffer::new(ctx);

        (
            DebugRenderer {
                ctx: ctx.clone(),
                geometry: HashMap::new(),
                program,
                vbo,
            },
            DebugAccumulator {
                shape_buffer: Mutex::new(vec![]),
                enabled: HashSet::new(),
            },
        )
    }

    fn add_line(&mut self, start: WorldPos, end: WorldPos, color: Vector4<f64>, weight: f64) {
        self.geometry
            .entry(OrderedFloat(weight))
            .or_default()
            .push(DebugVertex {
                pos: start.0,
                color,
            });
        self.geometry
            .entry(OrderedFloat(weight))
            .or_default()
            .push(DebugVertex { pos: end.0, color });
    }

    fn add_box(&mut self, b: Aabb3<f64>, color: Vector4<f64>, weight: f64) {
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
}

impl<'a> System<'a> for DebugRenderer {
    type SystemData = (
        ReadStorage<'a, comp::Transform>,
        ReadStorage<'a, comp::Player>,
        ReadStorage<'a, comp::ClientControlled>,
        Read<'a, res::ViewFrustum, PanicHandler>,
        ReadExpect<'a, ::glutin::GlWindow>,
        WriteExpect<'a, DebugAccumulator>,
    );

    fn run(
        &mut self,
        (transforms, player_marker, client_controlled_marker, frustum, window, mut accumulator): Self::SystemData,
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
                    let fsize = SIZE as f64;
                    let base = pos.base().base().0;
                    self.add_box(
                        Aabb3::new(base, base + Vector3::new(fsize, fsize, fsize)),
                        color,
                        weight,
                    );
                }

                Shape::GriddedChunk(weight, pos, color) => {
                    let fsize = SIZE as f64;
                    let base = pos.base().base();
                    self.add_box(
                        Aabb3::new(base.0, base.0 + Vector3::new(fsize, fsize, fsize)),
                        color,
                        weight,
                    );

                    for n in 0..SIZE {
                        let n = n as f64;
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

        let player_transform = (&player_marker, &client_controlled_marker, &transforms)
            .join()
            .map(|(_, _, tfm)| tfm)
            .next();

        if let Some(tfm) = player_transform {
            let aspect_ratio = ::util::aspect_ratio(&window).unwrap() as f32;
            let view_matrix = tfm.as_matrix().cast::<f32>().unwrap();
            let projection = ::cgmath::perspective(
                Deg(frustum.fov.0 as f32),
                aspect_ratio,
                frustum.near_plane as f32,
                frustum.far_plane as f32,
            );
            self.program
                .set_uniform(&mut self.ctx, "view", &view_matrix);
            self.program
                .set_uniform(&mut self.ctx, "projection", &projection);

            for (weight, geom) in &self.geometry {
                self.vbo
                    .upload(&self.ctx, geom, UsageType::DynamicDraw)
                    .unwrap();

                gl_call!(LineWidth(weight.0 as f32)).unwrap();
                self.ctx
                    .draw_arrays(PrimitiveType::Lines, &self.program, &self.vbo);
            }
        }
    }
}
