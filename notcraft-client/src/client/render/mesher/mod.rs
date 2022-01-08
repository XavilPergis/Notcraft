use crate::client::render::renderer::{
    add_transient_debug_box, DebugBox, DebugBoxKind, MeshBuffers, RenderMeshComponent,
    SharedMeshContext, UploadableMesh,
};
use anyhow::Result;
use crossbeam_channel::{Receiver, Sender};
use glium::{backend::Facade, index::PrimitiveType, IndexBuffer, VertexBuffer};
use notcraft_common::{
    aabb::Aabb,
    prelude::*,
    world::{
        chunk::{ChunkData, ChunkPos, ChunkSnapshot, CHUNK_LENGTH},
        chunk_aabb,
        registry::BlockId,
        VoxelWorld,
    },
    Faces, Side,
};
use rayon::{ThreadPool, ThreadPoolBuilder};
use std::{collections::HashSet, str::FromStr, sync::Arc, time::Duration};

use self::{
    generation::{should_add_face, ChunkNeighbors, CompletedMesh, MeshCreationContext},
    tracker::{update_tracker, MeshTracker},
};

pub mod generation;
pub mod tracker;

#[derive(Debug)]
pub struct MesherContext {
    completed_meshes: HashSet<ChunkPos>,

    mesher_pool: ThreadPool,
    mesh_tx: Sender<CompletedMesh>,
    mesh_rx: Receiver<CompletedMesh>,
    mode: MesherMode,
}

impl MesherContext {
    fn new(mode: MesherMode) -> Self {
        let mesher_pool = ThreadPoolBuilder::new().build().unwrap();
        let (mesh_tx, mesh_rx) = crossbeam_channel::unbounded();
        Self {
            completed_meshes: Default::default(),
            mesher_pool,
            mesh_tx,
            mesh_rx,
            mode,
        }
    }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub enum MesherMode {
    Simple,
    /// greedy meshing doesn't play well with randomized textures
    Greedy,
}

impl FromStr for MesherMode {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s {
            "simple" => Self::Simple,
            "greedy" => Self::Greedy,
            other => bail!("unknown mesher mode '{}'", other),
        })
    }
}

#[derive(Debug)]
pub struct ChunkMesherPlugin {
    pub mode: MesherMode,
}

impl ChunkMesherPlugin {
    pub fn with_mode(mut self, mode: MesherMode) -> Self {
        self.mode = mode;
        self
    }
}

impl Default for ChunkMesherPlugin {
    fn default() -> Self {
        Self {
            mode: MesherMode::Simple,
        }
    }
}

impl Plugin for ChunkMesherPlugin {
    fn build(&self, app: &mut AppBuilder) {
        app.insert_resource(MeshTracker::default());
        app.insert_resource(MesherContext::new(self.mode));
        app.add_system(update_tracker.system());
        app.add_system(queue_mesh_jobs.system());
        app.add_system(update_completed_meshes.system());
    }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash, Default)]
pub struct HasTerrainMesh;

fn update_completed_meshes(
    mut cmd: Commands,
    ctx: Res<MesherContext>,
    mut tracker: ResMut<MeshTracker>,
    voxel_world: Res<Arc<VoxelWorld>>,
    mesh_context: Res<Arc<SharedMeshContext<TerrainMesh>>>,
) {
    for completed in ctx.mesh_rx.try_iter() {
        match completed {
            CompletedMesh::Completed { pos, terrain } => {
                if let Some(entity) = tracker.terrain_entity(pos) {
                    if voxel_world.chunk(pos).is_some() {
                        let mesh_handle = mesh_context.upload(terrain);
                        cmd.entity(entity)
                            .insert(RenderMeshComponent::new(mesh_handle));
                    }
                }
            }
            CompletedMesh::Failed { pos } => tracker.chunk_mesh_failed(pos),
        }
    }
}

fn homogenous_should_mesh(world: &Arc<VoxelWorld>, id: BlockId, pos: ChunkPos) -> Option<bool> {
    let faces = Faces {
        top: match world.chunk(pos.offset([0, 1, 0]))?.snapshot().blocks() {
            &ChunkData::Homogeneous(nid) => should_add_face(&world.registry, id, nid),
            _ => true,
        },
        bottom: match world.chunk(pos.offset([0, -1, 0]))?.snapshot().blocks() {
            &ChunkData::Homogeneous(nid) => should_add_face(&world.registry, id, nid),
            _ => true,
        },
        right: match world.chunk(pos.offset([1, 0, 0]))?.snapshot().blocks() {
            &ChunkData::Homogeneous(nid) => should_add_face(&world.registry, id, nid),
            _ => true,
        },
        left: match world.chunk(pos.offset([-1, 0, 0]))?.snapshot().blocks() {
            &ChunkData::Homogeneous(nid) => should_add_face(&world.registry, id, nid),
            _ => true,
        },
        front: match world.chunk(pos.offset([0, 0, 1]))?.snapshot().blocks() {
            &ChunkData::Homogeneous(nid) => should_add_face(&world.registry, id, nid),
            _ => true,
        },
        back: match world.chunk(pos.offset([0, 0, -1]))?.snapshot().blocks() {
            &ChunkData::Homogeneous(nid) => should_add_face(&world.registry, id, nid),
            _ => true,
        },
    };
    Some(faces.any(|&face| face))
}

fn queue_mesh_job(ctx: &mut MesherContext, world: &Arc<VoxelWorld>, chunk: &ChunkSnapshot) {
    let world = Arc::clone(world);
    let sender = ctx.mesh_tx.clone();
    let pos = chunk.pos();
    let mode = ctx.mode;

    // note that we explicittly dont move the locked chunk to the new thread,
    // because otherwise we would keep the chunk locked while no progress on
    // meshing the chunk would be made.
    ctx.mesher_pool.spawn(move || {
        if let Some(neighbors) = ChunkNeighbors::lock(&world, pos) {
            let mesher = MeshCreationContext::new(pos, neighbors, &world);
            match mode {
                MesherMode::Simple => mesher.mesh_simple(sender),
                MesherMode::Greedy => mesher.mesh_greedy(sender),
            }
            add_transient_debug_box(Duration::from_secs(1), DebugBox {
                bounds: chunk_aabb(pos),
                rgba: [1.0, 1.0, 0.0, 0.3],
                kind: DebugBoxKind::Dashed,
            });
        } else {
            sender.send(CompletedMesh::Failed { pos }).unwrap();
            add_transient_debug_box(Duration::from_secs(4), DebugBox {
                bounds: chunk_aabb(pos),
                rgba: [1.0, 0.0, 0.0, 1.0],
                kind: DebugBoxKind::Dashed,
            });
        }
    });

    ctx.completed_meshes.insert(pos);
}

// returns true if this mesh job was "cheap", meaning that this job shoudln't
// count towards the number of meshed chunks this frame.
fn mesh_one(ctx: &mut MesherContext, world: &Arc<VoxelWorld>, chunk: &ChunkSnapshot) -> bool {
    let pos = chunk.pos();
    match chunk.blocks() {
        &ChunkData::Homogeneous(id) => match homogenous_should_mesh(world, id, pos) {
            Some(true) => queue_mesh_job(ctx, world, chunk),
            Some(false) | None => {
                add_transient_debug_box(Duration::from_secs(1), DebugBox {
                    bounds: chunk_aabb(chunk.pos()),
                    rgba: [1.0, 0.0, 1.0, 0.3],
                    kind: DebugBoxKind::Dashed,
                });
                return true;
            }
        },

        ChunkData::Array(_) => queue_mesh_job(ctx, world, chunk),
    }

    false
}

fn queue_mesh_jobs(
    mut ctx: ResMut<MesherContext>,
    mut tracker: ResMut<MeshTracker>,
    voxel_world: Res<Arc<VoxelWorld>>,
) {
    let mut remaining_this_frame = 4;

    while remaining_this_frame > 0 {
        let chunk = match tracker.next(&voxel_world).map(|chunk| chunk.snapshot()) {
            Some(chunk) => chunk,
            None => break,
        };
        if !mesh_one(&mut ctx, &voxel_world, &chunk) {
            remaining_this_frame -= 1;
        }
    }
}

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

    // - 4 bits for sky light
    // - 4 bits for block light
    // (5 bit residual)
    // - 1 bit for side
    // - 2 bits for axis
    // we can compute the UV coordinates from the surface normal and the world position, and we can
    // get the normal via a lookup table using the side
    // - 16 bits for block id
    // this seems substantial enough to never ever be a problem
    pub light_side_id: u32,
}

glium::implement_vertex!(TerrainVertex, pos_ao, light_side_id);

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
    pub fn pack(
        pos: [u16; 3],
        side: Side,
        sky_light: u16,
        block_light: u16,
        id: u16,
        ao: u8,
    ) -> Self {
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

        let mut light = 0u32;
        light |= ((sky_light & 0xf) as u32) << 4;
        light |= (block_light & 0xf) as u32;

        // SSSS BBBB .... .DSS  IIII IIII IIII IIII
        let mut light_side_id = 0u32;
        light_side_id |= light << 8;
        light_side_id |= pack_side(side) as u32;
        light_side_id <<= 16;
        light_side_id |= id as u32;

        Self {
            pos_ao,
            light_side_id,
        }
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
                min: point![0.0, 0.0, 0.0],
                max: point![
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
                min: point![0.0, 0.0, 0.0],
                max: point![
                    CHUNK_LENGTH as f32,
                    CHUNK_LENGTH as f32,
                    CHUNK_LENGTH as f32
                ],
            },
        })
    }
}
