//! this module houses the machinery for creating mesh data from world data.
//!
//! [`MeshBuilder`] holds the current mesh, and `mesh_*` functions like
//! [`mesh_cross`] and [`mesh_full_cube_side`] add to this structure. the
//! [`MeshBuilder`] is driven by the [`MeshCreationContext`], which holds all
//! the state necessary to mesh a single chunk.

use std::sync::Arc;

use crossbeam_channel::Sender;
use nalgebra::{Point3, Vector3};
use rand::{prelude::SliceRandom, rngs::SmallRng, FromEntropy};

use notcraft_common::{
    prelude::*,
    world::{
        chunk::{ChunkData, ChunkSectionPos, ChunkSectionSnapshot, CHUNK_LENGTH},
        lighting::LightValue,
        registry::{BlockId, BlockMeshType, BlockRegistry, TextureId},
        VoxelWorld,
    },
    Side,
};

use super::{TerrainMesh, TerrainVertex};

pub struct ChunkNeighbors {
    chunks: Vec<ChunkSectionSnapshot>,
}

impl ChunkNeighbors {
    pub fn lock(world: &Arc<VoxelWorld>, pos: ChunkSectionPos) -> Option<Self> {
        let mut chunks = Vec::with_capacity(27);

        for dx in -1..=1 {
            for dy in -1..=1 {
                for dz in -1..=1 {
                    chunks.push(world.section(pos.offset([dx, dy, dz]))?.snapshot());
                }
            }
        }

        Some(Self { chunks })
    }

    fn id<I: Into<[ChunkAxisOffset; 3]>>(&self, pos: I) -> BlockId {
        let [x, y, z] = pos.into();
        let (cx, mx) = chunks_index_and_offset(x);
        let (cy, my) = chunks_index_and_offset(y);
        let (cz, mz) = chunks_index_and_offset(z);

        match self.chunks[9 * cx + 3 * cy + cz].blocks() {
            ChunkData::Homogeneous(id) => *id,
            ChunkData::Array(arr) => arr[[mx, my, mz]],
        }
    }

    fn light<I: Into<[ChunkAxisOffset; 3]>>(&self, pos: I) -> LightValue {
        let [x, y, z] = pos.into();
        let (cx, mx) = chunks_index_and_offset(x);
        let (cy, my) = chunks_index_and_offset(y);
        let (cz, mz) = chunks_index_and_offset(z);

        match self.chunks[9 * cx + 3 * cy + cz].light() {
            ChunkData::Homogeneous(id) => *id,
            ChunkData::Array(arr) => arr[[mx, my, mz]],
        }
    }
}

fn chunks_index_and_offset(n: ChunkAxisOffset) -> (usize, usize) {
    const LEN: ChunkAxisOffset = CHUNK_LENGTH as ChunkAxisOffset;
    match n {
        _ if n < 0 => (0, (n + LEN) as usize),
        _ if n >= LEN => (2, (n - LEN) as usize),
        _ => (1, n as usize),
    }
}

type ChunkAxis = u16;
type ChunkAxisOffset = i16;

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash, Default)]
pub struct VoxelQuad {
    ao: FaceAo,
    light: FaceLight,
    id: BlockId,
    width: ChunkAxis,
    height: ChunkAxis,
}

impl From<VoxelFace> for VoxelQuad {
    fn from(face: VoxelFace) -> Self {
        VoxelQuad {
            ao: face.ao,
            id: face.id,
            light: face.light,
            width: 1,
            height: 1,
        }
    }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash, Default)]
struct VoxelFace {
    ao: FaceAo,
    light: FaceLight,
    id: BlockId,
    visited: bool,
}

impl VoxelFace {
    fn new(ao: FaceAo, light: FaceLight, id: BlockId) -> Self {
        Self {
            ao,
            light,
            id,
            visited: false,
        }
    }

    fn visited() -> Self {
        Self {
            visited: true,
            ..Default::default()
        }
    }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub enum LightingType {
    Smooth,
    Simple,
}

pub struct MeshCreationContext {
    registry: Arc<BlockRegistry>,
    chunks: ChunkNeighbors,
    mesh_constructor: MeshBuilder,
    pos: ChunkSectionPos,
    slice: Vec<VoxelFace>,
    lighting_type: LightingType,
}

// index into the flat voxel face slice using a 2D coordinate
const fn idx(u: ChunkAxis, v: ChunkAxis) -> usize {
    CHUNK_LENGTH * u as usize + v as usize
}

pub fn should_add_face(registry: &BlockRegistry, current: BlockId, neighbor: BlockId) -> bool {
    let cur_solid = matches!(registry.get(current).mesh_type(), BlockMeshType::FullCube);
    let other_solid = matches!(registry.get(neighbor).mesh_type(), BlockMeshType::FullCube);

    let cur_liquid = registry.get(current).liquid();
    let other_liquid = registry.get(neighbor).liquid();

    // note that cross-type blocks are not handled here; they're added in a
    // completely separate pass that doesn't depend on this function at all.
    if cur_liquid {
        // liquids only need a face when that face touches a non-full-cube type block.
        !other_solid && !other_liquid
    } else if cur_solid {
        // solids need a face when touching a non-full-cube type block *or* if they
        // touch a liquid.
        !other_solid || other_liquid
    } else {
        false
    }
}

impl MeshCreationContext {
    pub fn new(
        pos: ChunkSectionPos,
        neighbors: ChunkNeighbors,
        registry: &Arc<BlockRegistry>,
    ) -> Self {
        let mesh_constructor = MeshBuilder {
            registry: Arc::clone(registry),
            terrain_mesh: Default::default(),
            // transparency_mesh: Default::default(),
            rng: SmallRng::from_entropy(),
        };

        MeshCreationContext {
            registry: Arc::clone(registry),
            chunks: neighbors,
            pos,
            slice: vec![VoxelFace::default(); notcraft_common::world::chunk::CHUNK_LENGTH_2],
            mesh_constructor,
            lighting_type: LightingType::Simple,
        }
    }

    fn face_ao(&self, pos: Point3<ChunkAxis>, side: Side) -> FaceAo {
        let pos = pos.cast::<ChunkAxisOffset>();
        let contributes_ao = |pos| {
            let id = self.chunks.id(pos);
            matches!(self.registry.get(id).mesh_type(), BlockMeshType::FullCube)
                && !self.registry.get(id).liquid()
        };

        let neg_neg = contributes_ao(pos + side.uvl_to_xyz(-1, -1, 1));
        let neg_cen = contributes_ao(pos + side.uvl_to_xyz(-1, 0, 1));
        let neg_pos = contributes_ao(pos + side.uvl_to_xyz(-1, 1, 1));
        let pos_neg = contributes_ao(pos + side.uvl_to_xyz(1, -1, 1));
        let pos_cen = contributes_ao(pos + side.uvl_to_xyz(1, 0, 1));
        let pos_pos = contributes_ao(pos + side.uvl_to_xyz(1, 1, 1));
        let cen_neg = contributes_ao(pos + side.uvl_to_xyz(0, -1, 1));
        let cen_pos = contributes_ao(pos + side.uvl_to_xyz(0, 1, 1));

        let face_pos_pos = ao_value(cen_pos, pos_pos, pos_cen); // c+ ++ +c
        let face_pos_neg = ao_value(pos_cen, pos_neg, cen_neg); // +c +- c-
        let face_neg_neg = ao_value(cen_neg, neg_neg, neg_cen); // c- -- -c
        let face_neg_pos = ao_value(neg_cen, neg_pos, cen_pos); // -c -+ c+

        FaceAo(
            face_pos_pos << FaceAo::AO_POS_POS
                | face_pos_neg << FaceAo::AO_POS_NEG
                | face_neg_neg << FaceAo::AO_NEG_NEG
                | face_neg_pos << FaceAo::AO_NEG_POS,
        )
    }

    fn face_light(&self, pos: Point3<ChunkAxis>, side: Side) -> FaceLight {
        match self.lighting_type {
            LightingType::Smooth => {
                let pos = pos.cast::<ChunkAxisOffset>();
                let light = |pos| self.chunks.light(pos);

                let nn = light(pos + side.uvl_to_xyz(-1, -1, 1));
                let nc = light(pos + side.uvl_to_xyz(-1, 0, 1));
                let np = light(pos + side.uvl_to_xyz(-1, 1, 1));
                let cn = light(pos + side.uvl_to_xyz(0, -1, 1));
                let cc = light(pos + side.uvl_to_xyz(0, 0, 1));
                let cp = light(pos + side.uvl_to_xyz(0, 1, 1));
                let pn = light(pos + side.uvl_to_xyz(1, -1, 1));
                let pc = light(pos + side.uvl_to_xyz(1, 0, 1));
                let pp = light(pos + side.uvl_to_xyz(1, 1, 1));

                let neg_neg = LightValue::combine_max(
                    LightValue::combine_max(nn, nc),
                    LightValue::combine_max(cn, cc),
                );
                let neg_pos = LightValue::combine_max(
                    LightValue::combine_max(np, nc),
                    LightValue::combine_max(cp, cc),
                );
                let pos_neg = LightValue::combine_max(
                    LightValue::combine_max(pn, pc),
                    LightValue::combine_max(cn, cc),
                );
                let pos_pos = LightValue::combine_max(
                    LightValue::combine_max(pp, pc),
                    LightValue::combine_max(cp, cc),
                );

                FaceLight {
                    neg_neg,
                    neg_pos,
                    pos_neg,
                    pos_pos,
                }
            }

            LightingType::Simple => {
                let light = self
                    .chunks
                    .light(pos.cast::<ChunkAxisOffset>() + side.normal());
                FaceLight {
                    neg_neg: light,
                    neg_pos: light,
                    pos_neg: light,
                    pos_pos: light,
                }
            }
        }
    }

    /*
    for each x:
        for each y:
            if the face has been expanded onto already, skip this.

            # note that width and height start off as 1, and mark the "next" block
            while (x + width) is still in chunk bounds and the face at (x + width, y) is the same as the current face:
                increment width

            while (y + height) is still in chunk bounds:
                # every block under the current quad
                if every block in x=[x, x + width] y=y+1 is the same as the current:
                    increment height
                else:
                    stop the loop

            mark every block under expanded quad as visited
    */
    // TODO: explain how greedy meshing works

    fn submit_quads(
        &mut self,
        side: Side,
        point_constructor: impl Fn(ChunkAxis, ChunkAxis) -> Point3<ChunkAxis>,
    ) {
        for u in 0..(CHUNK_LENGTH as ChunkAxis) {
            for v in 0..(CHUNK_LENGTH as ChunkAxis) {
                let cur = self.slice[idx(u, v)];

                let is_liquid = self.registry.get(cur.id).liquid();

                // if the face has been expanded onto already, skip it.
                if cur.visited
                    || !(matches!(
                        self.registry.get(cur.id).mesh_type(),
                        BlockMeshType::FullCube
                    ) || is_liquid)
                {
                    continue;
                }
                let mut quad = VoxelQuad::from(cur);

                // while the next position is in chunk bounds and is the same block face as the
                // current
                while u + quad.width < (CHUNK_LENGTH as ChunkAxis)
                    && self.slice[idx(u + quad.width, v)] == cur
                {
                    quad.width += 1;
                }

                while v + quad.height < (CHUNK_LENGTH as ChunkAxis) {
                    if (u..u + quad.width)
                        .map(|u| self.slice[idx(u, v + quad.height)])
                        .all(|face| face == cur)
                    {
                        quad.height += 1;
                    } else {
                        break;
                    }
                }

                for w in 0..quad.width {
                    for h in 0..quad.height {
                        self.slice[idx(u + w, v + h)].visited = true;
                    }
                }

                // if is_liquid {
                //     self.mesh_constructor
                //         .add_liquid(quad, side, point_constructor(u, v));
                // } else {
                // }
                mesh_full_cube_side(
                    &mut self.mesh_constructor,
                    quad,
                    side,
                    point_constructor(u, v),
                );
            }
        }
    }

    fn mesh_slice(
        &mut self,
        side: Side,
        make_coordinate: impl Fn(ChunkAxis, ChunkAxis, ChunkAxis) -> Point3<ChunkAxis>,
    ) {
        let normal = side.normal::<ChunkAxisOffset>();
        for layer in 0..(CHUNK_LENGTH as ChunkAxis) {
            for u in 0..(CHUNK_LENGTH as ChunkAxis) {
                for v in 0..(CHUNK_LENGTH as ChunkAxis) {
                    let pos = make_coordinate(layer, u, v);
                    let cur_id = self.chunks.id(pos.cast());
                    let neighbor_id = self.chunks.id(pos.cast() + normal);

                    let face = should_add_face(&self.registry, cur_id, neighbor_id)
                        .then(|| {
                            VoxelFace::new(
                                self.face_ao(pos, side),
                                self.face_light(pos, side),
                                cur_id,
                            )
                        })
                        .unwrap_or(VoxelFace::visited());
                    self.slice[idx(u, v)] = face;
                }
            }

            self.submit_quads(side, |u, v| make_coordinate(layer, u, v));
        }
    }

    pub fn mesh_simple(mut self, sender: Sender<CompletedMesh>) {
        for x in 0..(CHUNK_LENGTH as ChunkAxis) {
            for z in 0..(CHUNK_LENGTH as ChunkAxis) {
                for y in 0..(CHUNK_LENGTH as ChunkAxis) {
                    let pos = point![x, y, z];
                    let cur_id = self.chunks.id(pos.cast());
                    let cur_light = self.chunks.light(pos.cast());
                    match self.registry.get(cur_id).mesh_type() {
                        BlockMeshType::None => {}
                        BlockMeshType::Cross => {
                            mesh_cross(&mut self.mesh_constructor, cur_id, pos, cur_light)
                        }
                        BlockMeshType::FullCube => Side::enumerate(|side| {
                            let normal = side.normal::<ChunkAxisOffset>();
                            let neighbor_id = self.chunks.id(pos.cast() + normal);
                            if should_add_face(&self.registry, cur_id, neighbor_id) {
                                let ao = self.face_ao(pos, side);
                                let light = self.face_light(pos, side);
                                mesh_full_cube_side(
                                    &mut self.mesh_constructor,
                                    VoxelQuad {
                                        ao,
                                        id: cur_id,
                                        light,
                                        width: 1,
                                        height: 1,
                                    },
                                    side,
                                    pos,
                                );
                            }
                        }),
                    }
                }
            }
        }

        sender
            .send(CompletedMesh::Completed {
                pos: self.pos,
                terrain: self.mesh_constructor.terrain_mesh,
            })
            .unwrap();
    }

    pub fn mesh_greedy(mut self, sender: Sender<CompletedMesh>) {
        for x in 0..(CHUNK_LENGTH as ChunkAxis) {
            for z in 0..(CHUNK_LENGTH as ChunkAxis) {
                for y in 0..(CHUNK_LENGTH as ChunkAxis) {
                    let pos = point![x, y, z];
                    let id = self.chunks.id(pos.cast());
                    let light = self.chunks.light(pos.cast());
                    if matches!(self.registry.get(id).mesh_type(), BlockMeshType::Cross) {
                        // TODO: light
                        mesh_cross(&mut self.mesh_constructor, id, pos, light)
                    }
                }
            }
        }

        self.mesh_slice(Side::Right, |layer, u, v| point!(layer, u, v));
        self.mesh_slice(Side::Left, |layer, u, v| point!(layer, u, v));

        self.mesh_slice(Side::Top, |layer, u, v| point!(u, layer, v));
        self.mesh_slice(Side::Bottom, |layer, u, v| point!(u, layer, v));

        self.mesh_slice(Side::Front, |layer, u, v| point!(u, v, layer));
        self.mesh_slice(Side::Back, |layer, u, v| point!(u, v, layer));

        sender
            .send(CompletedMesh::Completed {
                pos: self.pos,
                terrain: self.mesh_constructor.terrain_mesh,
            })
            .unwrap();
    }
}

#[derive(Debug)]
pub enum CompletedMesh {
    Completed {
        pos: ChunkSectionPos,
        terrain: TerrainMesh,
    },
    Failed {
        pos: ChunkSectionPos,
    },
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash, Default)]
struct FaceAo(u8);

impl FaceAo {
    const AO_NEG_NEG: u8 = 2;
    const AO_NEG_POS: u8 = 0;
    const AO_POS_NEG: u8 = 4;
    const AO_POS_POS: u8 = 6;

    fn corner_ao(&self, bits: u8) -> u8 {
        (self.0 & (3 << bits)) >> bits
    }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash, Default)]
struct FaceLight {
    neg_neg: LightValue,
    neg_pos: LightValue,
    pos_neg: LightValue,
    pos_pos: LightValue,
}

const FLIPPED_QUAD_CW: &'static [u32] = &[0, 1, 2, 3, 2, 1];
const FLIPPED_QUAD_CCW: &'static [u32] = &[2, 1, 0, 1, 2, 3];
const NORMAL_QUAD_CW: &'static [u32] = &[3, 2, 0, 0, 1, 3];
const NORMAL_QUAD_CCW: &'static [u32] = &[0, 2, 3, 3, 1, 0];

#[derive(Debug)]
pub struct MeshBuilder {
    // liquid_mesh: LiquidMesh,
    terrain_mesh: TerrainMesh,
    // transparency_mesh: TerrainTransparencyMesh,
    registry: Arc<BlockRegistry>,
    rng: SmallRng,
}

pub fn mesh_cross(ctx: &mut MeshBuilder, id: BlockId, pos: Point3<ChunkAxis>, light: LightValue) {
    let tex_id = choose_face_texture(ctx, id, Side::Right).0 as u16;
    let wind_sway = ctx.registry.get(id).wind_sway();

    {
        #[rustfmt::skip]
        const CROSS_INDICES: &'static [u32] = &[
            0,1,2, 0,2,3, 0,2,1, 0,3,2,
            4,5,6, 4,6,7, 4,6,5, 4,7,6,
        ];

        let idx_start = ctx.terrain_mesh.vertices.len() as u32;
        ctx.terrain_mesh
            .indices
            .extend(CROSS_INDICES.iter().copied().map(|idx| idx_start + idx));
    }

    let mut vert = |sway, offset: Vector3<_>| {
        let pos = (16 * pos) + offset;
        ctx.terrain_mesh.vertices.push(TerrainVertex::pack(
            pos.into(),
            sway,
            Side::Right,
            light,
            tex_id,
            3,
        ));
    };

    // we dont just use 1 here because of some weird wrapping behavior in the
    // terrain shader. we end up getting artifacts at the top of crosses if we do.
    let l = 1;
    let h = 15;

    vert(false, vector![l, 0, l]);
    vert(wind_sway, vector![l, h, l]);
    vert(wind_sway, vector![h, h, h]);
    vert(false, vector![h, 0, h]);

    vert(false, vector![l, 0, h]);
    vert(wind_sway, vector![l, h, h]);
    vert(wind_sway, vector![h, h, l]);
    vert(false, vector![h, 0, l]);
}

pub fn mesh_full_cube_side(
    ctx: &mut MeshBuilder,
    quad: VoxelQuad,
    side: Side,
    pos: Point3<ChunkAxis>,
) {
    let ao_pp = quad.ao.corner_ao(FaceAo::AO_POS_POS);
    let ao_pn = quad.ao.corner_ao(FaceAo::AO_POS_NEG);
    let ao_nn = quad.ao.corner_ao(FaceAo::AO_NEG_NEG);
    let ao_np = quad.ao.corner_ao(FaceAo::AO_NEG_POS);
    let flipped = ao_pp + ao_nn < ao_pn + ao_np;

    let light_pp = quad.light.pos_pos;
    let light_pn = quad.light.pos_neg;
    let light_nn = quad.light.neg_neg;
    let light_np = quad.light.neg_pos;
    let flipped = flipped
        || light_pp.intensity() + light_nn.intensity()
            <= light_pn.intensity() + light_np.intensity();

    let clockwise = match side {
        Side::Top => false,
        Side::Bottom => true,
        Side::Front => true,
        Side::Back => false,
        Side::Right => false,
        Side::Left => true,
    };

    let indices = match (flipped, clockwise) {
        (true, true) => FLIPPED_QUAD_CW,
        (true, false) => FLIPPED_QUAD_CCW,
        (false, true) => NORMAL_QUAD_CW,
        (false, false) => NORMAL_QUAD_CCW,
    };

    let idx_start = ctx.terrain_mesh.vertices.len() as u32;
    ctx.terrain_mesh
        .indices
        .extend(indices.iter().copied().map(|idx| idx_start + idx));

    let tex_id = choose_face_texture(ctx, quad.id, side).0 as u16;
    let wind_sway = ctx.registry.get(quad.id).wind_sway();

    let mut vert = |offset: Vector3<_>, ao, light| {
        let pos: Point3<u16> = (16 * pos) + (16 * offset);
        ctx.terrain_mesh.vertices.push(TerrainVertex::pack(
            pos.into(),
            wind_sway,
            side,
            light,
            tex_id,
            ao,
        ));
    };

    let h = if side.facing_positive() { 1 } else { 0 };
    let qw = quad.width;
    let qh = quad.height;

    match side {
        Side::Left | Side::Right => {
            vert(vector!(h, qw, 0), ao_pn, light_pn);
            vert(vector!(h, qw, qh), ao_pp, light_pp);
            vert(vector!(h, 0, 0), ao_nn, light_nn);
            vert(vector!(h, 0, qh), ao_np, light_np);
        }

        Side::Top | Side::Bottom => {
            vert(vector!(0, h, qh), ao_pn, light_pn);
            vert(vector!(qw, h, qh), ao_pp, light_pp);
            vert(vector!(0, h, 0), ao_nn, light_nn);
            vert(vector!(qw, h, 0), ao_np, light_np);
        }

        Side::Front | Side::Back => {
            vert(vector!(0, qh, h), ao_np, light_np);
            vert(vector!(qw, qh, h), ao_pp, light_pp);
            vert(vector!(0, 0, h), ao_nn, light_nn);
            vert(vector!(qw, 0, h), ao_pn, light_pn);
        }
    }
}

fn ao_value(side1: bool, corner: bool, side2: bool) -> u8 {
    if side1 && side2 {
        0
    } else {
        3 - (side1 as u8 + side2 as u8 + corner as u8)
    }
}

fn choose_face_texture(ctx: &mut MeshBuilder, id: BlockId, side: Side) -> TextureId {
    let pool_ids = ctx.registry.get(id).block_textures().unwrap();
    let pool_ids = pool_ids.choose(&mut ctx.rng).unwrap();
    let pool_id = pool_ids[side];

    let tex_ids = ctx.registry.pool_textures(pool_id);
    *tex_ids.choose(&mut ctx.rng).unwrap()
}
