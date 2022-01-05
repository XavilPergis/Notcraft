use crate::engine::{
    math::*,
    prelude::*,
    transform::Transform,
    world::{
        chunk::{ChunkData, ChunkPos, ChunkSnapshot, CHUNK_LENGTH},
        chunk_aabb,
        registry::{BlockId, BlockMeshType, BlockRegistry, Faces},
        ChunkEvent, VoxelWorld,
    },
    Side,
};
use anyhow::Result;
use crossbeam_channel::{Receiver, Sender};
use glium::{backend::Facade, index::PrimitiveType, IndexBuffer, VertexBuffer};
use na::OPoint;
use rand::{prelude::SliceRandom, rngs::SmallRng, FromEntropy, SeedableRng};
use rayon::{ThreadPool, ThreadPoolBuilder};
use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
    time::Duration,
};

use super::renderer::{
    add_transient_debug_box, Aabb, DebugBox, DebugBoxKind, MeshBuffers, RenderMeshComponent,
    SharedMeshContext, UploadableMesh,
};

#[derive(Debug)]
struct MeshTracker {
    constraining: HashMap<ChunkPos, HashSet<ChunkPos>>,
    constrained_by: HashMap<ChunkPos, HashSet<ChunkPos>>,
    unconstrained: HashSet<ChunkPos>,

    have_data: HashSet<ChunkPos>,
}

impl MeshTracker {
    fn new() -> Self {
        Self {
            constraining: Default::default(),
            constrained_by: Default::default(),
            unconstrained: Default::default(),
            have_data: Default::default(),
        }
    }

    // INVARIANT: if `have_data` does not contain X, then `constrained_by` also does
    // not contain X

    // INVARIANT: if `have_data` contains X, then `constraining` does NOT contain X

    // INVARIANT: for a chunk X and each value Y of `constraining[X]`,
    // `constrained_by[Y]` must contain X

    fn chunk_mesh_failed(&mut self, chunk: ChunkPos) {
        // by the time it gets here, the failed chunk might have been unloaded itself,
        // or might have had its neighbors been unloaded. if it was unloaded itself,
        // there is nothing to do because of the `have_data` invariants.
        if !self.have_data.contains(&chunk) {
            return;
        }

        neighbors(chunk, |neighbor| {
            if !self.have_data.contains(&neighbor) {
                self.constraining.entry(neighbor).or_default().insert(chunk);
                self.constrained_by
                    .entry(chunk)
                    .or_default()
                    .insert(neighbor);
            }
        });

        // it might be the case that a mesh failed because of unloaded neighbors, but
        // between the time that the failed response was queued and now, the neighbors
        // became loaded.
        if !self.constrained_by.contains_key(&chunk) {
            self.unconstrained.insert(chunk);
        }
    }

    fn chunk_added(&mut self, chunk: ChunkPos) {
        self.have_data.insert(chunk);

        // set up constraints for the newly-added chunk
        neighbors(chunk, |neighbor| {
            if !self.have_data.contains(&neighbor) {
                self.constraining.entry(neighbor).or_default().insert(chunk);
                self.constrained_by
                    .entry(chunk)
                    .or_default()
                    .insert(neighbor);
            }
        });

        // it may be the case that we get a new chunk where all its neighbors already
        // have data, in which case the new chunk is already unconstrained.
        if !self.constrained_by.contains_key(&chunk) {
            self.unconstrained.insert(chunk);
        }

        // remove constraints for neighbors that depended on us
        if let Some(constraining) = self.constraining.get_mut(&chunk) {
            for &neighbor in constraining.iter() {
                let neighbor_constraints = self
                    .constrained_by
                    .get_mut(&neighbor)
                    .expect("(add) constraints not bidirectional");

                neighbor_constraints.remove(&chunk);
                if neighbor_constraints.is_empty() {
                    self.unconstrained.insert(neighbor);
                    self.constrained_by.remove(&neighbor);
                }
            }

            self.constraining.remove(&chunk);
        }

        assert!(!self.constraining.contains_key(&chunk));
    }

    fn chunk_removed(&mut self, chunk: ChunkPos) {
        self.have_data.remove(&chunk);

        // add constraints to neighbors of the newly-removed chunk
        neighbors(chunk, |neighbor| {
            if self.have_data.contains(&neighbor) {
                self.constraining.entry(chunk).or_default().insert(neighbor);
                self.constrained_by
                    .entry(neighbor)
                    .or_default()
                    .insert(chunk);

                self.unconstrained.remove(&neighbor);
            }
        });

        // remove old `constraining` entries that pointed to the removed chunk,
        // upholding one of our `have_data` invariants.
        if let Some(constrainers) = self.constrained_by.get(&chunk) {
            for &constrainer in constrainers.iter() {
                let neighbor_constraining = self
                    .constraining
                    .get_mut(&constrainer)
                    .expect("(remove) constraints not bidirectional");

                neighbor_constraining.remove(&chunk);
                if neighbor_constraining.is_empty() {
                    self.constraining.remove(&constrainer);
                }
            }
        }

        // uphold our second `have_data` invariant.
        self.constrained_by.remove(&chunk);
    }
}

#[derive(Debug)]
pub struct MesherContext {
    terrain_entities: HashMap<ChunkPos, Entity>,
    tracker: MeshTracker,
    completed_meshes: HashSet<ChunkPos>,

    mesher_pool: ThreadPool,
    mesh_tx: Sender<CompletedMesh>,
    mesh_rx: Receiver<CompletedMesh>,
}

impl Default for MesherContext {
    fn default() -> Self {
        let mesher_pool = ThreadPoolBuilder::new().build().unwrap();
        let (mesh_tx, mesh_rx) = crossbeam_channel::unbounded();
        Self {
            terrain_entities: Default::default(),
            tracker: MeshTracker::new(),
            completed_meshes: Default::default(),
            mesher_pool,
            mesh_tx,
            mesh_rx,
        }
    }
}

#[derive(Debug, Default)]
pub struct ChunkMesherPlugin {}

impl Plugin for ChunkMesherPlugin {
    fn build(&self, app: &mut AppBuilder) {
        app.add_system(chunk_mesher.system());
    }
}

fn neighbors<F>(pos: ChunkPos, mut func: F)
where
    F: FnMut(ChunkPos),
{
    for x in pos.x - 1..=pos.x + 1 {
        for y in pos.y - 1..=pos.y + 1 {
            for z in pos.z - 1..=pos.z + 1 {
                let neighbor = ChunkPos { x, y, z };
                if neighbor != pos {
                    func(neighbor);
                }
            }
        }
    }
}

fn update_tracker(
    ctx: &mut MesherContext,
    cmd: &mut Commands,
    mut events: EventReader<ChunkEvent>,
) {
    for event in events.iter() {
        match event {
            ChunkEvent::Added(chunk) => ctx.tracker.chunk_added(chunk.pos()),
            ChunkEvent::Removed(chunk) => {
                ctx.tracker.chunk_removed(chunk.pos());
                if let Some(entity) = ctx.terrain_entities.remove(&chunk.pos()) {
                    cmd.entity(entity).despawn();
                }
            }
            ChunkEvent::Modified(chunk) => {
                if ctx.tracker.constrained_by.contains_key(&chunk.pos()) {
                    if let Some(entity) = ctx.terrain_entities.remove(&chunk.pos()) {
                        cmd.entity(entity).despawn();
                    }
                } else if ctx.tracker.have_data.contains(&chunk.pos()) {
                    ctx.tracker.unconstrained.insert(chunk.pos());
                }
            }
        }
    }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash, Default)]
pub struct HasTerrainMesh;

fn update_completed_meshes(
    ctx: &mut MesherContext,
    cmd: &mut Commands,
    mesh_context: &Arc<SharedMeshContext<TerrainMesh>>,
) {
    for completed in ctx.mesh_rx.try_iter() {
        match completed {
            CompletedMesh::Completed { pos, terrain } => {
                if ctx.tracker.have_data.contains(&pos) {
                    let entity = *ctx
                        .terrain_entities
                        .entry(pos)
                        .or_insert_with(|| cmd.spawn().id());

                    let world_pos: Point3<f32> = pos.origin().origin().into();
                    let transform = Transform::from(world_pos);

                    let mesh_handle = mesh_context.upload(terrain);
                    cmd.entity(entity)
                        .insert(RenderMeshComponent::new(mesh_handle))
                        .insert(transform);
                }
            }
            CompletedMesh::Failed { pos } => ctx.tracker.chunk_mesh_failed(pos),
        }
    }
}

fn next_mesh_chunk(ctx: &mut MesherContext, world: &Arc<VoxelWorld>) -> Option<ChunkSnapshot> {
    let &pos = ctx.tracker.unconstrained.iter().next()?;
    match world.chunk(pos) {
        Some(chunk) => Some(chunk.snapshot()),
        None => {
            log::warn!(
                "chunk {:?} was tracked for meshing but didnt exist in the world",
                pos
            );
            ctx.tracker.unconstrained.remove(&pos);
            None
        }
    }
}

fn homogenous_should_mesh(
    world: &Arc<VoxelWorld>,
    id: BlockId,
    pos: ChunkPos,
) -> Option<Faces<bool>> {
    Some(Faces {
        top: match world.chunk(pos.offset([0, 1, 0]))?.snapshot().data() {
            &ChunkData::Homogeneous(nid) => should_add_face(&world.registry, id, nid),
            _ => true,
        },
        bottom: match world.chunk(pos.offset([0, -1, 0]))?.snapshot().data() {
            &ChunkData::Homogeneous(nid) => should_add_face(&world.registry, id, nid),
            _ => true,
        },
        right: match world.chunk(pos.offset([1, 0, 0]))?.snapshot().data() {
            &ChunkData::Homogeneous(nid) => should_add_face(&world.registry, id, nid),
            _ => true,
        },
        left: match world.chunk(pos.offset([-1, 0, 0]))?.snapshot().data() {
            &ChunkData::Homogeneous(nid) => should_add_face(&world.registry, id, nid),
            _ => true,
        },
        front: match world.chunk(pos.offset([0, 0, 1]))?.snapshot().data() {
            &ChunkData::Homogeneous(nid) => should_add_face(&world.registry, id, nid),
            _ => true,
        },
        back: match world.chunk(pos.offset([0, 0, -1]))?.snapshot().data() {
            &ChunkData::Homogeneous(nid) => should_add_face(&world.registry, id, nid),
            _ => true,
        },
    })
}

fn queue_mesh_jobs(ctx: &mut MesherContext, world: &Arc<VoxelWorld>) {
    let mut remaining_this_frame = 4;

    while let Some(chunk) = next_mesh_chunk(ctx, world) {
        if remaining_this_frame == 0 {
            break;
        }

        let pos = chunk.pos();
        match chunk.data() {
            &ChunkData::Homogeneous(id) => {
                match homogenous_should_mesh(world, id, pos).map(|faces| {
                    faces.top
                        || faces.bottom
                        || faces.right
                        || faces.left
                        || faces.front
                        || faces.back
                }) {
                    Some(true) => {
                        let world = Arc::clone(world);
                        let sender = ctx.mesh_tx.clone();

                        // note that we explicittly dont move the locked chunk to the new thread,
                        // because otherwise we would keep the chunk locked while no progress on
                        // meshing the chunk would be made.
                        ctx.mesher_pool.spawn(move || {
                            if let Some(neighbors) = ChunkNeighbors::lock(&world, pos) {
                                MeshCreationContext::new(pos, neighbors, &world).mesh(sender);
                                add_transient_debug_box(Duration::from_secs(1), DebugBox {
                                    bounds: chunk_aabb(chunk.pos()),
                                    rgba: [1.0, 1.0, 0.0, 1.0],
                                    kind: DebugBoxKind::Dashed,
                                });
                            } else {
                                sender.send(CompletedMesh::Failed { pos }).unwrap();
                                add_transient_debug_box(Duration::from_secs(1), DebugBox {
                                    bounds: chunk_aabb(chunk.pos()),
                                    rgba: [1.0, 0.0, 0.0, 1.0],
                                    kind: DebugBoxKind::Dashed,
                                });
                            }
                        });

                        ctx.completed_meshes.insert(pos);
                        ctx.tracker.unconstrained.remove(&pos);

                        remaining_this_frame -= 1;
                    }
                    Some(false) | None => {
                        ctx.tracker.unconstrained.remove(&pos);
                        add_transient_debug_box(Duration::from_secs(1), DebugBox {
                            bounds: chunk_aabb(chunk.pos()),
                            rgba: [1.0, 0.0, 1.0, 0.3],
                            kind: DebugBoxKind::Dashed,
                        });
                    }
                }
            }

            ChunkData::Array(_) => {
                let world = Arc::clone(world);
                let sender = ctx.mesh_tx.clone();

                // note that we explicittly dont move the locked chunk to the new thread,
                // because otherwise we would keep the chunk locked while no progress on meshing
                // the chunk would be made.
                ctx.mesher_pool.spawn(move || {
                    if let Some(neighbors) = ChunkNeighbors::lock(&world, pos) {
                        MeshCreationContext::new(pos, neighbors, &world).mesh(sender);
                        add_transient_debug_box(Duration::from_secs(1), DebugBox {
                            bounds: chunk_aabb(chunk.pos()),
                            rgba: [1.0, 1.0, 0.0, 1.0],
                            kind: DebugBoxKind::Solid,
                        });
                    } else {
                        sender.send(CompletedMesh::Failed { pos }).unwrap();
                        add_transient_debug_box(Duration::from_secs(1), DebugBox {
                            bounds: chunk_aabb(chunk.pos()),
                            rgba: [1.0, 0.0, 0.0, 1.0],
                            kind: DebugBoxKind::Dashed,
                        });
                    }
                });

                ctx.completed_meshes.insert(pos);
                ctx.tracker.unconstrained.remove(&pos);

                remaining_this_frame -= 1;
            }
        }
    }
}

pub fn chunk_mesher(
    mut cmd: Commands,
    mut ctx: Local<MesherContext>,
    voxel_world: Res<Arc<VoxelWorld>>,
    mesh_context: Res<Arc<SharedMeshContext<TerrainMesh>>>,
    events: EventReader<ChunkEvent>,
) {
    update_tracker(&mut ctx, &mut cmd, events);
    queue_mesh_jobs(&mut ctx, &voxel_world);
    update_completed_meshes(&mut ctx, &mut cmd, &mesh_context);
}

struct ChunkNeighbors {
    chunks: Vec<ChunkSnapshot>,
}

fn chunks_index_and_offset(n: ChunkAxisOffset) -> (usize, usize) {
    const LEN: ChunkAxisOffset = CHUNK_LENGTH as ChunkAxisOffset;
    match n {
        _ if n < 0 => (0, (n + LEN) as usize),
        _ if n >= LEN => (2, (n - LEN) as usize),
        _ => (1, n as usize),
    }
}

impl ChunkNeighbors {
    fn lock(world: &Arc<VoxelWorld>, pos: ChunkPos) -> Option<Self> {
        let mut chunks = Vec::with_capacity(27);

        for dx in -1..=1 {
            for dy in -1..=1 {
                for dz in -1..=1 {
                    chunks.push(world.chunk(pos.offset([dx, dy, dz]))?.snapshot());
                }
            }
        }

        Some(Self { chunks })
    }

    fn lookup<I: Into<[ChunkAxisOffset; 3]>>(&self, pos: I) -> BlockId {
        let [x, y, z] = pos.into();
        let (cx, mx) = chunks_index_and_offset(x);
        let (cy, my) = chunks_index_and_offset(y);
        let (cz, mz) = chunks_index_and_offset(z);

        match self.chunks[9 * cx + 3 * cy + cz].data() {
            ChunkData::Homogeneous(id) => *id,
            ChunkData::Array(arr) => arr[[mx, my, mz]],
        }
    }
}

type ChunkAxis = u16;
type ChunkAxisOffset = i16;

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

impl VoxelFace {
    fn new(ao: FaceAo, id: BlockId) -> Self {
        Self {
            ao,
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

struct MeshCreationContext {
    registry: Arc<BlockRegistry>,
    chunks: ChunkNeighbors,
    mesh_constructor: MeshConstructor,
    pos: ChunkPos,
    slice: Vec<VoxelFace>,
}

// index into the flat voxel face slice using a 2D coordinate
const fn idx(u: ChunkAxis, v: ChunkAxis) -> usize {
    CHUNK_LENGTH * u as usize + v as usize
}

fn should_add_face(registry: &BlockRegistry, current: BlockId, neighbor: BlockId) -> bool {
    let cur_solid = matches!(registry.mesh_type(current), BlockMeshType::FullCube);
    let other_solid = matches!(registry.mesh_type(neighbor), BlockMeshType::FullCube);

    let cur_liquid = registry.liquid(current);
    let other_liquid = registry.liquid(neighbor);

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
    pub fn new(pos: ChunkPos, neighbors: ChunkNeighbors, world: &Arc<VoxelWorld>) -> Self {
        let mesh_constructor = MeshConstructor {
            registry: Arc::clone(&world.registry),
            terrain_mesh: Default::default(),
            transparency_mesh: Default::default(),
            rng: SmallRng::from_entropy(),
        };

        MeshCreationContext {
            registry: Arc::clone(&world.registry),
            chunks: neighbors,
            pos,
            slice: vec![VoxelFace::default(); crate::engine::world::chunk::CHUNK_AREA],
            mesh_constructor,
        }
    }

    fn face_ao(&self, pos: Point3<ChunkAxis>, side: Side) -> FaceAo {
        let pos = pos.cast::<ChunkAxisOffset>();
        let contributes_ao = |pos| {
            let id = self.chunks.lookup(pos);
            matches!(self.registry.mesh_type(id), BlockMeshType::FullCube)
                && !self.registry.liquid(id)
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
        for u in 0..(CHUNK_LENGTH as ChunkAxis) {
            for v in 0..(CHUNK_LENGTH as ChunkAxis) {
                let cur = self.slice[idx(u, v)];

                let is_liquid = self.registry.liquid(cur.id);

                // if the face has been expanded onto already, skip it.
                if cur.visited
                    || !(matches!(self.registry.mesh_type(cur.id), BlockMeshType::FullCube)
                        || is_liquid)
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
                mesh_full_cube(
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
                    let cur_id = self.chunks.lookup(pos.cast());
                    let neighbor_id = self.chunks.lookup(pos.cast() + normal);

                    let face = should_add_face(&self.registry, cur_id, neighbor_id)
                        .then(|| VoxelFace::new(self.face_ao(pos, side), cur_id))
                        .unwrap_or(VoxelFace::visited());
                    self.slice[idx(u, v)] = face;
                }
            }

            self.submit_quads(side, |u, v| make_coordinate(layer, u, v));
        }
    }

    pub fn mesh(mut self, sender: Sender<CompletedMesh>) {
        for x in 0..(CHUNK_LENGTH as ChunkAxis) {
            for z in 0..(CHUNK_LENGTH as ChunkAxis) {
                for y in 0..(CHUNK_LENGTH as ChunkAxis) {
                    let pos = na::point![x, y, z];
                    let id = self.chunks.lookup(pos.cast());
                    if matches!(self.registry.mesh_type(id), BlockMeshType::Cross) {
                        mesh_cross(&mut self.mesh_constructor, id, pos)
                    }
                }
            }
        }
        self.mesh_slice(Side::Right, |layer, u, v| na::point!(layer, u, v));
        self.mesh_slice(Side::Left, |layer, u, v| na::point!(layer, u, v));

        self.mesh_slice(Side::Top, |layer, u, v| na::point!(u, layer, v));
        self.mesh_slice(Side::Bottom, |layer, u, v| na::point!(u, layer, v));

        self.mesh_slice(Side::Front, |layer, u, v| na::point!(u, v, layer));
        self.mesh_slice(Side::Back, |layer, u, v| na::point!(u, v, layer));

        sender
            .send(CompletedMesh::Completed {
                pos: self.pos,
                terrain: self.mesh_constructor.terrain_mesh,
            })
            .unwrap();
    }
}

enum CompletedMesh {
    Completed { pos: ChunkPos, terrain: TerrainMesh },
    Failed { pos: ChunkPos },
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
    // 5 bits of precisions gets 1-block resolution, an additonal 5 bits gets 16 subdivisions of a
    // block.
    // - 2 bits for AO
    // AO only has 3 possible values, [0,3]
    // lower AO values mean darker shadows
    pub pos_ao: u32,

    // (13 bit residual)
    // - 1 bit for side
    // - 2 bits for axis
    // we can compute the UV coordinates from the surface normal and the world position, and we can
    // get the normal via a lookup table using the side
    // - 16 bits for block id
    // this seems substantial enough to never ever be a problem
    pub side_id: u32,
}

glium::implement_vertex!(TerrainVertex, pos_ao, side_id);

fn pack_side(side: Side) -> u8 {
    match side {
        // sides with positive facing normals wrt their own axes have a 0 in their MSB
        Side::Top => 0b001,
        Side::Left => 0b000,
        Side::Front => 0b010,
        // sides with negative facing normals have a 1 in their MSB
        Side::Bottom => 0b101,
        Side::Right => 0b100,
        Side::Back => 0b110,
    }
}

impl TerrainVertex {
    pub fn pack(pos: [u16; 3], side: Side, id: u16, ao: u8) -> Self {
        let [x, y, z] = pos;
        let mut pos_ao = 0u32;
        // while 10 bits are reserved for each axis, we only use 5 of them currently.
        // xxxx xXXX XXyy  yyyY YYYY zzzz zZZZ ZZAA
        pos_ao |= x as u32 & 0x7ff;
        pos_ao <<= 10;
        pos_ao |= y as u32 & 0x7ff;
        pos_ao <<= 10;
        pos_ao |= z as u32 & 0x7ff;
        pos_ao <<= 2;
        pos_ao |= ao as u32;

        // .... .... .... .DSS  IIII IIII IIII IIII
        let mut side_id = 0u32;
        side_id |= pack_side(side) as u32;
        side_id <<= 16;
        side_id |= id as u32;

        Self { pos_ao, side_id }
    }
}

#[derive(Clone, Debug, PartialEq, Default)]
pub struct TerrainTransparencyMesh {
    vertices: Vec<TerrainVertex>,
    // TODO: use u16s when possible
    indices: Vec<u32>,
}

impl UploadableMesh for TerrainTransparencyMesh {
    type Vertex = TerrainVertex;

    fn upload<F: Facade>(&self, ctx: &F) -> Result<MeshBuffers<Self::Vertex>> {
        Ok(MeshBuffers {
            vertices: VertexBuffer::immutable(ctx, &self.vertices)?,
            indices: IndexBuffer::immutable(ctx, PrimitiveType::TrianglesList, &self.indices)?,

            aabb: Aabb {
                min: na::point![0.0, 0.0, 0.0],
                max: na::point![
                    CHUNK_LENGTH as f32,
                    CHUNK_LENGTH as f32,
                    CHUNK_LENGTH as f32
                ],
            },
        })
    }
}

#[derive(Clone, Debug, PartialEq, Default)]
pub struct TerrainMesh {
    vertices: Vec<TerrainVertex>,
    // TODO: use u16s when possible
    indices: Vec<u32>,
}

impl UploadableMesh for TerrainMesh {
    type Vertex = TerrainVertex;

    fn upload<F: Facade>(&self, ctx: &F) -> Result<MeshBuffers<Self::Vertex>> {
        Ok(MeshBuffers {
            vertices: VertexBuffer::immutable(ctx, &self.vertices)?,
            indices: IndexBuffer::immutable(ctx, PrimitiveType::TrianglesList, &self.indices)?,

            aabb: Aabb {
                min: na::point![0.0, 0.0, 0.0],
                max: na::point![
                    CHUNK_LENGTH as f32,
                    CHUNK_LENGTH as f32,
                    CHUNK_LENGTH as f32
                ],
            },
        })
    }
}

#[derive(Debug)]
struct MeshConstructor {
    // liquid_mesh: LiquidMesh,
    terrain_mesh: TerrainMesh,
    transparency_mesh: TerrainTransparencyMesh,
    registry: Arc<BlockRegistry>,
    rng: SmallRng,
}

impl MeshConstructor {
    fn add_liquid(&mut self, quad: VoxelQuad, side: Side, pos: Point3<ChunkAxis>) {
        // let ao_pp = quad.ao.corner_ao(FaceAo::AO_POS_POS);
        // let ao_pn = quad.ao.corner_ao(FaceAo::AO_POS_NEG);
        // let ao_nn = quad.ao.corner_ao(FaceAo::AO_NEG_NEG);
        // let ao_np = quad.ao.corner_ao(FaceAo::AO_NEG_POS);
        // let flipped = ao_pp + ao_nn > ao_pn + ao_np;

        // let clockwise = match side {
        //     Side::Top => false,
        //     Side::Bottom => true,
        //     Side::Front => true,
        //     Side::Back => false,
        //     Side::Right => false,
        //     Side::Left => true,
        // };

        // let indices = match (flipped, clockwise) {
        //     (true, true) => FLIPPED_QUAD_CW,
        //     (true, false) => FLIPPED_QUAD_CCW,
        //     (false, true) => NORMAL_QUAD_CW,
        //     (false, false) => NORMAL_QUAD_CCW,
        // };

        // let idx_start = self.transparency_mesh.vertices.len() as u32;
        // self.transparency_mesh
        //     .indices
        //     .extend(indices.iter().copied().map(|idx| idx_start + idx));

        // let face = self.registry.block_texture(quad.id, side);
        // if face.is_none() {
        //     log::error!("???: id={:?}, side={:?}", quad.id, side);
        // }
        // let face = face.unwrap();
        // let tex_id = *face.texture.select() as u16;

        // let mut vert = |offset: Vector3<_>, ao| {
        //     let pos = pos + offset;
        //     self.transparency_mesh
        //         .vertices
        //         .push(TerrainVertex::pack(pos.into(), side, tex_id, ao));
        // };

        // let h = if side.facing_positive() { 1 } else { 0 };
        // let qw = quad.width;
        // let qh = quad.height;

        // match side {
        //     Side::Left | Side::Right => {
        //         vert(na::vector!(h, qw, 0), ao_pn);
        //         vert(na::vector!(h, qw, qh), ao_pp);
        //         vert(na::vector!(h, 0, 0), ao_nn);
        //         vert(na::vector!(h, 0, qh), ao_np);
        //     }

        //     Side::Top | Side::Bottom => {
        //         vert(na::vector!(0, h, qh), ao_pn);
        //         vert(na::vector!(qw, h, qh), ao_pp);
        //         vert(na::vector!(0, h, 0), ao_nn);
        //         vert(na::vector!(qw, h, 0), ao_np);
        //     }

        //     Side::Front | Side::Back => {
        //         vert(na::vector!(0, qh, h), ao_np);
        //         vert(na::vector!(qw, qh, h), ao_pp);
        //         vert(na::vector!(0, 0, h), ao_nn);
        //         vert(na::vector!(qw, 0, h), ao_pn);
        //     }
        // }
    }
}

fn mesh_cross(ctx: &mut MeshConstructor, id: BlockId, pos: Point3<ChunkAxis>) {
    let tex_id = {
        let faces = ctx.registry.block_textures(id).unwrap();
        let faces = faces.choose(&mut ctx.rng).unwrap();
        // FIXME: not this!
        faces[Side::Top] as u16
    };

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

    let mut vert = |offset: Vector3<_>| {
        let pos = (16 * pos) + offset;
        ctx.terrain_mesh
            .vertices
            .push(TerrainVertex::pack(pos.into(), Side::Right, tex_id, 3));
    };

    // we dont just use 1 here because of some weird wrapping behavior in the
    // terrain shader. we end up getting artifacts at the top of crosses if we do.
    let l = 1;
    let h = 15;

    vert(na::vector![l, 0, l]);
    vert(na::vector![l, h, l]);
    vert(na::vector![h, h, h]);
    vert(na::vector![h, 0, h]);

    vert(na::vector![l, 0, h]);
    vert(na::vector![l, h, h]);
    vert(na::vector![h, h, l]);
    vert(na::vector![h, 0, l]);
}

fn mesh_full_cube(ctx: &mut MeshConstructor, quad: VoxelQuad, side: Side, pos: Point3<ChunkAxis>) {
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

    let faces = ctx.registry.block_textures(quad.id).unwrap();
    let faces = faces.choose(&mut ctx.rng).unwrap();
    let tex_id = faces[side] as u16;

    let mut vert = |offset: Vector3<_>, ao| {
        let pos: Point3<u16> = (16 * pos) + (16 * offset);
        ctx.terrain_mesh
            .vertices
            .push(TerrainVertex::pack(pos.into(), side, tex_id, ao));
    };

    let h = if side.facing_positive() { 1 } else { 0 };
    let qw = quad.width;
    let qh = quad.height;

    match side {
        Side::Left | Side::Right => {
            vert(na::vector!(h, qw, 0), ao_pn);
            vert(na::vector!(h, qw, qh), ao_pp);
            vert(na::vector!(h, 0, 0), ao_nn);
            vert(na::vector!(h, 0, qh), ao_np);
        }

        Side::Top | Side::Bottom => {
            vert(na::vector!(0, h, qh), ao_pn);
            vert(na::vector!(qw, h, qh), ao_pp);
            vert(na::vector!(0, h, 0), ao_nn);
            vert(na::vector!(qw, h, 0), ao_np);
        }

        Side::Front | Side::Back => {
            vert(na::vector!(0, qh, h), ao_np);
            vert(na::vector!(qw, qh, h), ao_pp);
            vert(na::vector!(0, 0, h), ao_nn);
            vert(na::vector!(qw, 0, h), ao_pn);
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
