use crate::engine::{camera::Camera, prelude::*, world::chunk::SIZE};
use collision::{Aabb3, Ray3};
use glium::{
    index::{NoIndices, PrimitiveType},
    *,
};
use ordered_float::OrderedFloat;
use std::{
    collections::{HashMap, HashSet},
    sync::{Mutex, MutexGuard},
};

#[derive(Copy, Clone, Debug)]
pub struct DebugVertex {
    pos: [f32; 3],
    color: [f32; 4],
}

glium::implement_vertex!(DebugVertex, pos, color);

fn push_line(buf: &mut Vec<DebugVertex>, start: WorldPos, end: WorldPos, color: Color) {
    buf.push(DebugVertex {
        pos: start.0.into(),
        color: color.into(),
    });

    buf.push(DebugVertex {
        pos: end.0.into(),
        color: color.into(),
    });
}

fn push_box(buf: &mut Vec<DebugVertex>, aabb: Aabb3<f32>, color: Color) {
    let len_x = aabb.max.x - aabb.min.x;
    let len_y = aabb.max.y - aabb.min.y;
    let len_z = aabb.max.z - aabb.min.z;

    let bmin = WorldPos(aabb.min);

    let y_lnn = bmin;
    let y_lnp = bmin.offset((0.0, 0.0, len_z));
    let y_lpn = bmin.offset((len_x, 0.0, 0.0));
    let y_lpp = bmin.offset((len_x, 0.0, len_z));
    let y_hnn = bmin.offset((0.0, len_y, 0.0));
    let y_hnp = bmin.offset((0.0, len_y, len_z));
    let y_hpn = bmin.offset((len_x, len_y, 0.0));
    let y_hpp = bmin.offset((len_x, len_y, len_z));

    push_line(buf, y_lnn, y_lnp, color);
    push_line(buf, y_lnp, y_lpp, color);
    push_line(buf, y_lpp, y_lpn, color);
    push_line(buf, y_lpn, y_lnn, color);

    push_line(buf, y_hnn, y_hnp, color);
    push_line(buf, y_hnp, y_hpp, color);
    push_line(buf, y_hpp, y_hpn, color);
    push_line(buf, y_hpn, y_hnn, color);

    push_line(buf, y_lnn, y_hnn, color);
    push_line(buf, y_lnp, y_hnp, color);
    push_line(buf, y_lpp, y_hpp, color);
    push_line(buf, y_lpn, y_hpn, color);
}

fn push_chunk_grids(buf: &mut Vec<DebugVertex>, pos: ChunkPos) {
    let fsize = SIZE as f32;
    let pos = pos.base().base();

    for n in 0..SIZE {
        let n = n as f32;
        let x_nn = pos.offset((n, 0.0, 0.0));
        let x_np = pos.offset((n, 0.0, fsize));
        let x_pn = pos.offset((n, fsize, 0.0));
        let x_pp = pos.offset((n, fsize, fsize));
        let y_nn = pos.offset((0.0, n, 0.0));
        let y_np = pos.offset((0.0, n, fsize));
        let y_pn = pos.offset((fsize, n, 0.0));
        let y_pp = pos.offset((fsize, n, fsize));
        let z_nn = pos.offset((0.0, 0.0, n));
        let z_np = pos.offset((0.0, fsize, n));
        let z_pn = pos.offset((fsize, 0.0, n));
        let z_pp = pos.offset((fsize, fsize, n));

        push_line(buf, x_nn, x_np, Vector4::new(1.0, 0.5, 0.5, 1.0));
        push_line(buf, x_np, x_pp, Vector4::new(1.0, 0.5, 0.5, 1.0));
        push_line(buf, x_pp, x_pn, Vector4::new(1.0, 0.5, 0.5, 1.0));
        push_line(buf, x_pn, x_nn, Vector4::new(1.0, 0.5, 0.5, 1.0));

        push_line(buf, y_nn, y_np, Vector4::new(0.5, 1.0, 0.5, 1.0));
        push_line(buf, y_np, y_pp, Vector4::new(0.5, 1.0, 0.5, 1.0));
        push_line(buf, y_pp, y_pn, Vector4::new(0.5, 1.0, 0.5, 1.0));
        push_line(buf, y_pn, y_nn, Vector4::new(0.5, 1.0, 0.5, 1.0));

        push_line(buf, z_nn, z_np, Vector4::new(0.5, 0.5, 1.0, 1.0));
        push_line(buf, z_np, z_pp, Vector4::new(0.5, 0.5, 1.0, 1.0));
        push_line(buf, z_pp, z_pn, Vector4::new(0.5, 0.5, 1.0, 1.0));
        push_line(buf, z_pn, z_nn, Vector4::new(0.5, 0.5, 1.0, 1.0));
    }
}

type Color = Vector4<f32>;

pub enum DebugSection<'a> {
    Disabled,
    Enabled(MutexGuard<'a, HashMap<OrderedFloat<f32>, Vec<DebugVertex>>>),
}

fn get_buffer(
    map: &mut HashMap<OrderedFloat<f32>, Vec<DebugVertex>>,
    weight: f32,
) -> &mut Vec<DebugVertex> {
    map.entry(OrderedFloat(weight)).or_default()
}

impl<'a> DebugSection<'a> {
    pub fn line(&mut self, start: WorldPos, end: WorldPos, weight: f32, color: Color) {
        if let DebugSection::Enabled(buf) = self {
            push_line(get_buffer(&mut *buf, weight), start, end, color);
        }
    }

    pub fn aabb(&mut self, aabb: Aabb3<f32>, weight: f32, color: Color) {
        if let DebugSection::Enabled(buf) = self {
            push_box(get_buffer(&mut *buf, weight), aabb, color);
        }
    }

    pub fn block(&mut self, pos: BlockPos, weight: f32, color: Color) {
        if let DebugSection::Enabled(buf) = self {
            push_box(get_buffer(&mut *buf, weight), pos.into(), color);
        }
    }

    pub fn chunk(&mut self, pos: ChunkPos, weight: f32, color: Color) {
        if let DebugSection::Enabled(buf) = self {
            push_box(get_buffer(&mut *buf, weight), pos.into(), color);
        }
    }

    pub fn gridded_chunk(&mut self, pos: ChunkPos, weight: f32, color: Color) {
        if let DebugSection::Enabled(buf) = self {
            push_box(get_buffer(&mut *buf, weight), pos.into(), color);
            push_chunk_grids(get_buffer(&mut *buf, weight / 2.0), pos);
        }
    }

    pub fn ray(&mut self, ray: Ray3<f32>, weight: f32, color: Color) {
        if let DebugSection::Enabled(buf) = self {
            push_line(
                get_buffer(&mut *buf, weight),
                WorldPos(ray.origin),
                WorldPos(ray.origin + ray.direction),
                color,
            );
        }
    }
}

#[derive(Debug, Default)]
pub struct DebugAccumulator {
    shape_buffer: Mutex<HashMap<OrderedFloat<f32>, Vec<DebugVertex>>>,
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

    pub fn buffer_mut(&mut self, weight: f32) -> &mut Vec<DebugVertex> {
        self.shape_buffer
            .get_mut()
            .unwrap()
            .entry(OrderedFloat(weight))
            .or_default()
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
            &util::read_file("resources/shaders/debug.vert")?,
            &util::read_file("resources/shaders/debug.frag")?,
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

    pub fn draw<S: Surface>(
        &mut self,
        surface: &mut S,
        accumulator: &mut DebugAccumulator,
        camera: Camera,
    ) {
        for (weight, geometry) in accumulator.shape_buffer.get_mut().unwrap() {
            let buffer =
                VertexBuffer::new(&self.ctx, &geometry).expect("Could not create debug buffer");

            surface
                .draw(
                    &buffer,
                    &NoIndices(PrimitiveType::LinesList),
                    &self.program,
                    &glium::uniform! {},
                    &DrawParameters {
                        depth: Depth {
                            test: DepthTest::IfLess,
                            write: true,
                            ..Default::default()
                        },
                        line_width: Some(weight.0),
                        ..Default::default()
                    },
                )
                .unwrap();

            geometry.clear();
        }
    }
}
