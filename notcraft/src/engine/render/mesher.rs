use crate::engine::{
    components::Transform,
    prelude::*,
    render::{Ao, Norm, Pos, Tang, Tex, TexId},
    world::{
        block::{BlockId, BlockRegistry},
        chunk::{ChunkType, PaddedChunk, SIZE},
        ChunkPos, VoxelWorld,
    },
    Side,
};
use crossbeam_channel::Receiver;
use legion::{systems::CommandBuffer, world::SubWorld, Entity, Query};
use na::point;
// use specs::{prelude::*, world::EntitiesRes};
use std::collections::{HashMap, HashSet};

/*

_   _   _   _   _   _   _

_   _   _   _   _   _   _
      \ | /
_   _ - 8 - _   _   _   _
      / | \
_   _   _   _   _   _   _

_   _   _   _   _   _   _

_   _   _   _   _   _   _

_   _   _   _   _   _   _


new chunk:
    for anything waiting on chunk:
        decrease waiting count
        if waiting count is 0:
            add to mesh set

*/

#[derive(Debug)]
pub struct MesherContext {
    terrain_entities: HashMap<ChunkPos, Entity>,

    have_data: HashSet<ChunkPos>,
    waiters: HashMap<ChunkPos, HashSet<ChunkPos>>,
    waiting_counts: HashMap<ChunkPos, usize>,
    needs_mesh: HashSet<ChunkPos>,
    completed_meshes: HashSet<ChunkPos>,

    new_chunk_rx: Receiver<ChunkPos>,
    modified_chunk_rx: Receiver<ChunkPos>,
}

impl MesherContext {
    pub fn new(voxel_world: &VoxelWorld) -> Self {
        Self {
            terrain_entities: Default::default(),
            have_data: Default::default(),
            waiters: Default::default(),
            waiting_counts: Default::default(),
            needs_mesh: Default::default(),
            completed_meshes: Default::default(),
            new_chunk_rx: voxel_world.new_chunks_notifier.clone(),
            modified_chunk_rx: voxel_world.modified_chunks_notifier.clone(),
        }
    }
}

fn neighborhood<F>(pos: ChunkPos, mut func: F)
where
    F: FnMut(ChunkPos),
{
    for x in pos.0.x - 1..=pos.0.x + 1 {
        for y in pos.0.y - 1..=pos.0.y + 1 {
            for z in pos.0.z - 1..=pos.0.z + 1 {
                func(ChunkPos(point!(x, y, z)));
            }
        }
    }
}

#[legion::system]
pub fn chunk_mesher(
    cmd: &mut CommandBuffer,
    #[state] ctx: &mut MesherContext,
    #[resource] voxel_world: &mut VoxelWorld,
) {
    for chunk in ctx.new_chunk_rx.try_iter() {
        ctx.have_data.insert(chunk);

        // setup waiting count and register this chunk to the surrounding waiters
        let mut unknown_neighbor_count = 0;
        neighborhood(chunk, |pos| {
            if pos != chunk && !ctx.have_data.contains(&pos) {
                unknown_neighbor_count += 1;
                ctx.waiters.entry(pos).or_default().insert(chunk);
            }
        });
        ctx.waiting_counts.insert(chunk, unknown_neighbor_count);

        // update waiters and add finished waiters to the mesh queue
        if let Some(waiters) = ctx.waiters.get(&chunk) {
            for &pos in waiters {
                match ctx.waiting_counts.get_mut(&pos).unwrap() {
                    1 => drop(ctx.needs_mesh.insert(pos)),
                    count => *count -= 1,
                }
            }
        }
        ctx.waiters.remove(&chunk);
    }

    for chunk in ctx.modified_chunk_rx.try_iter() {
        if ctx.completed_meshes.remove(&chunk) {
            ctx.needs_mesh.insert(chunk);
        }
    }

    if let Some(&pos) = ctx.needs_mesh.iter().next() {
        match voxel_world.chunk(pos) {
            Some(ChunkType::Homogeneous(_id)) => {
                // FIXME: causes holes when a solid homogeneous chunk touches air
                ctx.needs_mesh.remove(&pos);
            }

            Some(ChunkType::Array(_)) => {
                let mesh = mesh_chunk(pos, &voxel_world);
                let transform = Transform::from(pos.base().base().0);

                let entity = ctx
                    .terrain_entities
                    .get(&pos)
                    .copied()
                    .unwrap_or_else(|| cmd.push(()));

                cmd.add_component(entity, mesh);
                cmd.add_component(entity, transform);

                ctx.needs_mesh.remove(&pos);
            }

            None => {
                ctx.needs_mesh.remove(&pos);
            }
        }
    }
}

#[derive(Debug, Default)]
pub struct ChunkMesher {}

pub fn mesh_chunk(pos: ChunkPos, world: &VoxelWorld) -> TerrainMesh {
    let mut mesher = Mesher::new(pos, world);
    mesher.mesh();

    let mut terrain = mesher.mesh_constructor.terrain_mesh;
    // terrain.recalculate_norm_tang();

    terrain
}

type ChunkAxis = u16;
type ChunkOffset = i16;

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash, Default)]
struct VoxelQuad {
    ao: FaceAo,
    id: BlockId,
    width: ChunkAxis,
    height: ChunkAxis,
}

impl From<VoxelFace> for VoxelQuad {
    fn from(face: VoxelFace) -> Self {
        VoxelQuad {
            ao: face.ao,
            id: face.id,
            width: 1,
            height: 1,
        }
    }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash, Default)]
struct VoxelFace {
    ao: FaceAo,
    id: BlockId,
    visited: bool,
}

pub struct Mesher<'w> {
    registry: &'w BlockRegistry,
    center: PaddedChunk,
    mesh_constructor: MeshConstructor<'w>,
    slice: Vec<VoxelFace>,
}

// index into the flat voxel face slice using a 2D coordinate
const fn idx(u: ChunkAxis, v: ChunkAxis) -> usize {
    SIZE * u as usize + v as usize
}

impl<'w> Mesher<'w> {
    pub fn new(pos: ChunkPos, world: &'w VoxelWorld) -> Self {
        Mesher {
            registry: &world.registry,
            center: crate::engine::world::chunk::make_padded(world, pos).unwrap(),
            slice: vec![VoxelFace::default(); crate::engine::world::chunk::AREA],
            mesh_constructor: MeshConstructor {
                terrain_mesh: Default::default(),
                registry: &world.registry,
            },
        }
    }

    fn face_ao(&self, pos: Point3<ChunkAxis>, side: Side) -> FaceAo {
        if self.registry.liquid(self.center[pos.cast::<usize>()]) {
            return FaceAo::default();
        }

        let pos = na::point!(pos.x as i32, pos.y as i32, pos.z as i32);
        let is_opaque = |pos| self.registry.opaque(self.center[pos]);

        let neg_neg = is_opaque(pos + side.uvl_to_xyz(-1, -1, 1));
        let neg_cen = is_opaque(pos + side.uvl_to_xyz(-1, 0, 1));
        let neg_pos = is_opaque(pos + side.uvl_to_xyz(-1, 1, 1));
        let pos_neg = is_opaque(pos + side.uvl_to_xyz(1, -1, 1));
        let pos_cen = is_opaque(pos + side.uvl_to_xyz(1, 0, 1));
        let pos_pos = is_opaque(pos + side.uvl_to_xyz(1, 1, 1));
        let cen_neg = is_opaque(pos + side.uvl_to_xyz(0, -1, 1));
        let cen_pos = is_opaque(pos + side.uvl_to_xyz(0, 1, 1));

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

    fn is_not_occluded(&self, pos: Point3<ChunkAxis>, offset: Vector3<ChunkOffset>) -> bool {
        let pos = pos.cast::<usize>();
        let offset = pos.cast::<isize>() + offset.cast();

        let cur_solid = self.registry.opaque(self.center[pos]);
        let other_solid = self.registry.opaque(self.center[offset]);

        let cur_liquid = self.registry.liquid(self.center[pos]);
        let other_liquid = self.registry.liquid(self.center[offset]);

        if self.registry.liquid(self.center[pos]) {
            // if the current block is liquid, we need a face when the other block is
            // non-solid or non-liquid
            cur_liquid && !other_solid && !other_liquid
        } else {
            // if the current block is not liquid...
            // if the current block is not opaque, then it would never need any faces
            // if the current block is solid, and the other is either non-opaque or a
            // liquid, then we need a face
            cur_solid && (!other_solid || self.registry.liquid(self.center[offset]))
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

    pub fn submit_quads(
        &mut self,
        side: Side,
        point_constructor: impl Fn(ChunkAxis, ChunkAxis) -> Point3<ChunkAxis>,
    ) {
        for u in 0..(SIZE as ChunkAxis) {
            for v in 0..(SIZE as ChunkAxis) {
                let cur = self.slice[idx(u, v)];

                let is_liquid = self.registry.liquid(cur.id);

                // if the face has been expanded onto already, skip it.
                if cur.visited || !(self.registry.opaque(cur.id) || is_liquid) {
                    continue;
                }
                let mut quad = VoxelQuad::from(cur);

                // while the next position is in chunk bounds and is the same block face as the
                // current
                while u + quad.width < (SIZE as ChunkAxis)
                    && self.slice[idx(u + quad.width, v)] == cur
                {
                    quad.width += 1;
                }

                while v + quad.height < (SIZE as ChunkAxis) {
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

                if is_liquid {
                    self.mesh_constructor
                        .add_liquid(quad, side, point_constructor(u, v));
                } else {
                    self.mesh_constructor
                        .add_terrain(quad, side, point_constructor(u, v));
                }
            }
        }
    }

    fn mesh_slice(
        &mut self,
        side: Side,
        make_coordinate: impl Fn(ChunkAxis, ChunkAxis, ChunkAxis) -> Point3<ChunkAxis>,
    ) {
        for layer in 0..(SIZE as ChunkAxis) {
            for u in 0..(SIZE as ChunkAxis) {
                for v in 0..(SIZE as ChunkAxis) {
                    // chunk coords -> padded chunk coords
                    let padded = make_coordinate(layer, u, v) + na::vector!(1, 1, 1);
                    self.slice[idx(u, v)] = if self.is_not_occluded(padded, side.normal()) {
                        VoxelFace {
                            ao: self.face_ao(padded, side),
                            id: self.center[padded.cast::<usize>()],
                            visited: false,
                        }
                    } else {
                        VoxelFace {
                            ao: FaceAo::default(),
                            id: BlockId::default(),
                            visited: true,
                        }
                    };
                }
            }

            self.submit_quads(side, |u, v| make_coordinate(layer, u, v));
        }
    }

    pub fn mesh(&mut self) {
        self.mesh_slice(Side::Right, |layer, u, v| na::point!(layer, u, v));
        self.mesh_slice(Side::Left, |layer, u, v| na::point!(layer, u, v));

        self.mesh_slice(Side::Top, |layer, u, v| na::point!(u, layer, v));
        self.mesh_slice(Side::Bottom, |layer, u, v| na::point!(u, layer, v));

        self.mesh_slice(Side::Front, |layer, u, v| na::point!(u, v, layer));
        self.mesh_slice(Side::Back, |layer, u, v| na::point!(u, v, layer));
    }
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

const FLIPPED_QUAD_CW: &'static [u32] = &[0, 1, 2, 3, 2, 1];
const FLIPPED_QUAD_CCW: &'static [u32] = &[2, 1, 0, 1, 2, 3];
const NORMAL_QUAD_CW: &'static [u32] = &[3, 2, 0, 0, 1, 3];
const NORMAL_QUAD_CCW: &'static [u32] = &[0, 2, 3, 3, 1, 0];

#[derive(Copy, Clone, Debug, PartialEq, Default)]
#[repr(C)]
pub struct TerrainVertex {
    // - 10 bits for each position
    // 5 bits of precisions gets 1-block resolution, an additonal 5 bits gets 32 subdivisions of a
    // block.
    // - 2 bits for AO
    // AO only has 3 possible values, [0,3]
    pub pos_ao: u32,

    // - 10 bits for tex coords
    // each axis is 5 bits because chunks are 32 blocks or 2^5 bits across, so each face can be 32
    // blocks long at most. actual UVs will be calculated in the shader by casting each axis to a
    // float and getting the fractional part.
    // (6 bit residual)
    // - 16 bits for block id
    // this seems substantial enough to never ever be a problem
    pub uv_id: u32,
}

glium::implement_vertex!(TerrainVertex, pos_ao, uv_id);

impl TerrainVertex {
    pub fn pack(pos: [u16; 3], uv: [u8; 2], id: u16, ao: u8) -> Self {
        let [x, y, z] = pos;
        let mut pos_ao = 0u32;
        // while 10 bits are reserved for each axis, we only use 5 of them currently.
        pos_ao |= x as u32 & 0x7ff;
        pos_ao <<= 10;
        pos_ao |= y as u32 & 0x7ff;
        pos_ao <<= 10;
        pos_ao |= z as u32 & 0x7ff;
        pos_ao <<= 2;
        pos_ao |= ao as u32;

        let [u, v] = uv;
        let mut uv_id = 0u32;
        uv_id |= u as u32 & 0x3f;
        uv_id <<= 5;
        uv_id |= v as u32 & 0x3f;
        uv_id <<= 6;
        // reserved
        uv_id <<= 16;
        uv_id |= id as u32;

        Self { pos_ao, uv_id }
    }
}

#[derive(Clone, Debug, PartialEq, Default)]
pub struct TerrainMesh {
    pub vertices: Vec<TerrainVertex>,
    // TODO: use u16s when possible
    pub indices: Vec<u32>,
}

impl TerrainMesh {
    // pub fn recalculate_norm_tang(&mut self) {
    //     // Zero vertex normal and tangent vectors
    //     self.norm.clear();
    //     self.tang.clear();
    //     self.norm.resize(self.pos.len(), Default::default());
    //     self.tang.resize(self.pos.len(), Default::default());

    //     // sub-assign
    //     fn arr3_sub(lhs: &mut [f32; 3], rhs: &[f32; 3]) {
    //         lhs[0] -= rhs[0];
    //         lhs[1] -= rhs[1];
    //         lhs[2] -= rhs[2];
    //     }

    //     // normalize
    //     fn arr3_norm(arr: &mut [f32; 3]) {
    //         let (a, b, c) = (arr[0], arr[1], arr[2]);
    //         let mag = f32::sqrt(a * a + b * b + c * c);
    //         arr[0] /= mag;
    //         arr[1] /= mag;
    //         arr[2] /= mag;
    //     }

    //     // Sum all the unit normals and tangents for each vertex. for non-"flat"
    // meshes     // where different indices point to the same vertex, the
    // vector will not be a     // normal vector after the summation. We
    // compensate for this, though, in the     // next step by normalizing all
    // the basis vectors     for (a, b, c) in self
    //         .index
    //         .chunks(3)
    //         .map(|i| (i[0] as usize, i[1] as usize, i[2] as usize))
    //     {
    //         let triangle_norm = triangle_normal(self, a, b, c).into();
    //         let triangle_tang = triangle_tangent(self, a, b, c).into();

    //         arr3_sub(&mut self.norm[a].normal, &triangle_norm);
    //         arr3_sub(&mut self.norm[b].normal, &triangle_norm);
    //         arr3_sub(&mut self.norm[c].normal, &triangle_norm);

    //         arr3_sub(&mut self.tang[a].tangent, &triangle_tang);
    //         arr3_sub(&mut self.tang[b].tangent, &triangle_tang);
    //         arr3_sub(&mut self.tang[c].tangent, &triangle_tang);
    //     }

    //     // Normalize the normals and tangents
    //     self.norm
    //         .iter_mut()
    //         .for_each(|norm| arr3_norm(&mut norm.normal));
    //     self.tang
    //         .iter_mut()
    //         .for_each(|tang| arr3_norm(&mut tang.tangent));
    // }
}

// fn triangle_normal(mesh: &TerrainMesh, a: usize, b: usize, c: usize) ->
// Vector3<f32> {     let pa = Vector3::from(mesh.pos[a].pos);
//     let pb = Vector3::from(mesh.pos[b].pos);
//     let pc = Vector3::from(mesh.pos[c].pos);

//     let edge_b = pb - pa;
//     let edge_c = pc - pa;

//     edge_b.cross(&edge_c)
// }

// fn triangle_tangent(mesh: &TerrainMesh, a: usize, b: usize, c: usize) ->
// Vector3<f32> {     let p1 = Vector3::from(mesh.pos[a].pos);
//     let p2 = Vector3::from(mesh.pos[b].pos);
//     let p3 = Vector3::from(mesh.pos[c].pos);

//     let t1 = Vector2::from(mesh.tex[a].uv);
//     let t2 = Vector2::from(mesh.tex[b].uv);
//     let t3 = Vector2::from(mesh.tex[c].uv);

//     let delta_pos_2 = p2 - p1;
//     let delta_pos_3 = p3 - p1;

//     let delta_uv_2 = t2 - t1;
//     let delta_uv_3 = t3 - t1;

//     let r = 1.0 / (delta_uv_2.x * delta_uv_3.y - delta_uv_2.y *
// delta_uv_3.x);     r * (delta_pos_2 * delta_uv_3.y - delta_pos_3 *
// delta_uv_2.y) }

#[derive(Debug)]
struct MeshConstructor<'w> {
    // liquid_mesh: LiquidMesh,
    terrain_mesh: TerrainMesh,
    registry: &'w BlockRegistry,
}

impl<'w> MeshConstructor<'w> {
    fn add_liquid(&mut self, _quad: VoxelQuad, _side: Side, _pos: Point3<ChunkAxis>) {
        // let pos: Point3<f32> = pos.cast().unwrap();

        // let clockwise = match side {
        //     Side::Top => false,
        //     Side::Bottom => true,
        //     Side::Front => true,
        //     Side::Back => false,
        //     Side::Right => false,
        //     Side::Left => true,
        // };

        // let indices = if clockwise {
        //     NORMAL_QUAD_CW
        // } else {
        //     NORMAL_QUAD_CCW
        // };

        // let normal = side.normal();

        // let face = self.registry.block_texture(quad.id, side).unwrap();
        // let _tex_id = *face.texture.select() as i32;

        // let h = if side.facing_positive() { 1.0 } else { 0.0 };
        // let qw = quad.width as f32;
        // let qh = quad.height as f32;

        // let vert = |offset, uv| {
        //     LiquidVertex::default()
        //         .with_pos(pos + offset)
        //         .with_uv(uv)
        //         .with_normal(normal)
        // };

        // let uvs = UV_VARIANT_1;

        // if !self.liquid_mesh.add(
        //     match side {
        //         Side::Top | Side::Bottom => [
        //             vert(
        //                 na::vector!(0.0, h, qh),
        //                 na::vector!(uvs[0].x * qw, uvs[0].y * qh),
        //             ),
        //             vert(
        //                 na::vector!(qw, h, qh),
        //                 na::vector!(uvs[1].x * qw, uvs[1].y * qh),
        //             ),
        //             vert(
        //                 na::vector!(0.0, h, 0.0),
        //                 na::vector!(uvs[2].x * qw, uvs[2].y * qh),
        //             ),
        //             vert(
        //                 na::vector!(qw, h, 0.0),
        //                 na::vector!(uvs[3].x * qw, uvs[3].y * qh),
        //             ),
        //         ],

        //         Side::Left | Side::Right => [
        //             vert(
        //                 na::vector!(h, qw, 0.0),
        //                 na::vector!(uvs[0].x * qh, uvs[0].y * qw),
        //             ),
        //             vert(
        //                 na::vector!(h, qw, qh),
        //                 na::vector!(uvs[1].x * qh, uvs[1].y * qw),
        //             ),
        //             vert(
        //                 na::vector!(h, 0.0, 0.0),
        //                 na::vector!(uvs[2].x * qh, uvs[2].y * qw),
        //             ),
        //             vert(
        //                 na::vector!(h, 0.0, qh),
        //                 na::vector!(uvs[3].x * qh, uvs[3].y * qw),
        //             ),
        //         ],

        //         Side::Front | Side::Back => [
        //             vert(
        //                 na::vector!(0.0, qh, h),
        //                 na::vector!(uvs[0].x * qw, uvs[0].y * qh),
        //             ),
        //             vert(
        //                 na::vector!(qw, qh, h),
        //                 na::vector!(uvs[1].x * qw, uvs[1].y * qh),
        //             ),
        //             vert(
        //                 na::vector!(0.0, 0.0, h),
        //                 na::vector!(uvs[2].x * qw, uvs[2].y * qh),
        //             ),
        //             vert(
        //                 na::vector!(qw, 0.0, h),
        //                 na::vector!(uvs[3].x * qw, uvs[3].y * qh),
        //             ),
        //         ],
        //     }
        //     .iter()
        //     .cloned(),
        //     indices.iter().cloned(),
        // ) {
        //     panic!("Mesh could not be created");
        // }
    }

    fn add_terrain(&mut self, quad: VoxelQuad, side: Side, pos: Point3<ChunkAxis>) {
        let ao_pp = quad.ao.corner_ao(FaceAo::AO_POS_POS);
        let ao_pn = quad.ao.corner_ao(FaceAo::AO_POS_NEG);
        let ao_nn = quad.ao.corner_ao(FaceAo::AO_NEG_NEG);
        let ao_np = quad.ao.corner_ao(FaceAo::AO_NEG_POS);
        let flipped = ao_pp + ao_nn > ao_pn + ao_np;

        let clockwise = match side {
            Side::Top => false,
            Side::Bottom => true,
            Side::Front => true,
            Side::Back => false,
            Side::Right => false,
            Side::Left => true,
        };

        let indices = if flipped {
            if clockwise {
                FLIPPED_QUAD_CW
            } else {
                FLIPPED_QUAD_CCW
            }
        } else {
            if clockwise {
                NORMAL_QUAD_CW
            } else {
                NORMAL_QUAD_CCW
            }
        };

        let idx_start = self.terrain_mesh.vertices.len() as u32;
        self.terrain_mesh
            .indices
            .extend(indices.iter().copied().map(|idx| idx_start + idx));

        // let normal = side.normal::<f32>();

        let face = self.registry.block_texture(quad.id, side).unwrap();
        let tex_id = *face.texture.select() as u16;

        let mut vert = |offset: Vector3<_>, uv: Vector2<_>, ao| {
            let pos = pos + offset;
            self.terrain_mesh
                .vertices
                .push(TerrainVertex::pack(pos.into(), uv.into(), tex_id, ao));
        };

        let uvs = &[
            na::vector!(1, 1),
            na::vector!(0, 1),
            na::vector!(1, 0),
            na::vector!(0, 0),
        ];

        let h = if side.facing_positive() { 1 } else { 0 };
        let qw = quad.width;
        let qh = quad.height;

        // REALLY don't know why I have to reverse the quad width and height along the X
        // axis... I bet someone more qualified could tell me :>
        match side {
            Side::Left | Side::Right => {
                vert(
                    na::vector!(h, qw, 0),
                    na::vector!(uvs[0].x * qh as u8, uvs[0].y * qw as u8),
                    ao_pn,
                );
                vert(
                    na::vector!(h, qw, qh),
                    na::vector!(uvs[1].x * qh as u8, uvs[1].y * qw as u8),
                    ao_pp,
                );
                vert(
                    na::vector!(h, 0, 0),
                    na::vector!(uvs[2].x * qh as u8, uvs[2].y * qw as u8),
                    ao_nn,
                );
                vert(
                    na::vector!(h, 0, qh),
                    na::vector!(uvs[3].x * qh as u8, uvs[3].y * qw as u8),
                    ao_np,
                );
            }

            Side::Top | Side::Bottom => {
                vert(
                    na::vector!(0, h, qh),
                    na::vector!(uvs[0].x * qw as u8, uvs[0].y * qh as u8),
                    ao_pn,
                );
                vert(
                    na::vector!(qw, h, qh),
                    na::vector!(uvs[1].x * qw as u8, uvs[1].y * qh as u8),
                    ao_pp,
                );
                vert(
                    na::vector!(0, h, 0),
                    na::vector!(uvs[2].x * qw as u8, uvs[2].y * qh as u8),
                    ao_nn,
                );
                vert(
                    na::vector!(qw, h, 0),
                    na::vector!(uvs[3].x * qw as u8, uvs[3].y * qh as u8),
                    ao_np,
                );
            }

            Side::Front | Side::Back => {
                vert(
                    na::vector!(0, qh, h),
                    na::vector!(uvs[0].x * qw as u8, uvs[0].y * qh as u8),
                    ao_np,
                );
                vert(
                    na::vector!(qw, qh, h),
                    na::vector!(uvs[1].x * qw as u8, uvs[1].y * qh as u8),
                    ao_pp,
                );
                vert(
                    na::vector!(0, 0, h),
                    na::vector!(uvs[2].x * qw as u8, uvs[2].y * qh as u8),
                    ao_nn,
                );
                vert(
                    na::vector!(qw, 0, h),
                    na::vector!(uvs[3].x * qw as u8, uvs[3].y * qh as u8),
                    ao_pn,
                );
            }
        };
    }
}

fn ao_value(side1: bool, corner: bool, side2: bool) -> u8 {
    if side1 && side2 {
        0
    } else {
        3 - (side1 as u8 + side2 as u8 + corner as u8)
    }
}
