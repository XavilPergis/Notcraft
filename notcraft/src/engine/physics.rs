use std::{
    collections::{hash_map::Entry, HashMap},
    sync::Arc,
};

use legion::{systems::CommandBuffer, world::SubWorld, Entity, Query};
use nalgebra::{point, vector, Point3, Vector3};
use num_traits::Zero;

use super::{
    render::renderer::Aabb,
    transform::Transform,
    world::{
        chunk::{ChunkPos, ChunkSnapshot},
        registry::{BlockId, BlockRegistry},
        BlockPos, VoxelWorld,
    },
    Axis, Dt,
};

#[derive(Copy, Clone, Debug, PartialEq, Default)]
pub struct RigidBody {
    // pub mass: f32,
    // pub drag: Vector3<f32>,
    pub acceleration: Vector3<f32>,
    pub velocity: Vector3<f32>,
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub struct AabbCollider {
    pub aabb: Aabb,
}

/// a cache for multiple unaligned world accesses over a short period of time.
pub struct ChunkSnapshotCache {
    world: Arc<VoxelWorld>,
    chunks: HashMap<ChunkPos, ChunkSnapshot>,
}

impl ChunkSnapshotCache {
    pub fn new(world: &Arc<VoxelWorld>) -> Self {
        Self {
            world: Arc::clone(world),
            chunks: Default::default(),
        }
    }

    pub fn chunk(&mut self, pos: ChunkPos) -> Option<&ChunkSnapshot> {
        Some(match self.chunks.entry(pos) {
            Entry::Occupied(entry) => &*entry.into_mut(),
            Entry::Vacant(entry) => &*entry.insert(self.world.chunk(pos)?.snapshot()),
        })
    }

    pub fn block(&mut self, pos: BlockPos) -> Option<BlockId> {
        let (chunk_pos, chunk_index) = pos.chunk_and_offset();
        Some(self.chunk(chunk_pos)?.data().get(chunk_index))
    }
}

fn block_aabb(block: BlockPos) -> Aabb {
    let pos = point![block.x as f32, block.y as f32, block.z as f32];
    Aabb {
        min: pos,
        max: pos + vector![1.0, 1.0, 1.0],
    }
}

fn resolve_terrain_collisions_positive_face(
    cache: &mut ChunkSnapshotCache,
    registry: &BlockRegistry,
    aabb: &Aabb,
    prev_aabb: &Aabb,
) -> Option<Vector3<f32>> {
    let delta = aabb.center() - prev_aabb.center();
    let mut displacement = vector![0.0, 0.0, 0.0];

    if delta.x < 0.0 {
        // for +X face, only check the bottom of the collision box (YZ-plane slice)
        'outer_px: for y in prev_aabb.min.y.floor() as i32..=prev_aabb.max.y.floor() as i32 {
            for z in prev_aabb.min.z.floor() as i32..=prev_aabb.max.z.floor() as i32 {
                let x = aabb.min.x.floor() as i32;
                let block_pos = BlockPos { x, y, z };

                let prev_intersects = block_aabb(block_pos).intersects(&Aabb {
                    min: prev_aabb.min.map(f32::floor),
                    max: prev_aabb.max.map(f32::ceil),
                });
                if !prev_intersects && registry.collidable(cache.block(block_pos)?) {
                    displacement.x += x as f32 + 1.0 - aabb.min.x;
                    break 'outer_px;
                }
            }
        }
    }

    if delta.y < 0.0 {
        // for +Y face, only check the bottom of the collision box (XZ-plane slice)
        'outer_py: for x in prev_aabb.min.x.floor() as i32..=prev_aabb.max.x.floor() as i32 {
            for z in prev_aabb.min.z.floor() as i32..=prev_aabb.max.z.floor() as i32 {
                let y = aabb.min.y.floor() as i32;
                let block_pos = BlockPos { x, y, z };

                let prev_intersects = block_aabb(block_pos).intersects(&Aabb {
                    min: prev_aabb.min.map(f32::floor),
                    max: prev_aabb.max.map(f32::ceil),
                });
                if !prev_intersects && registry.collidable(cache.block(block_pos)?) {
                    displacement.y += y as f32 + 1.0 - aabb.min.y;
                    break 'outer_py;
                }
            }
        }
    }

    if delta.z < 0.0 {
        // for +Z face, only check the bottom of the collision box (XY-plane slice)
        'outer_pz: for x in prev_aabb.min.x.floor() as i32..=prev_aabb.max.x.floor() as i32 {
            for y in prev_aabb.min.y.floor() as i32..=prev_aabb.max.y.floor() as i32 {
                let z = aabb.min.z.floor() as i32;
                let block_pos = BlockPos { x, y, z };

                let prev_intersects = block_aabb(block_pos).intersects(&Aabb {
                    min: prev_aabb.min.map(f32::floor),
                    max: prev_aabb.max.map(f32::ceil),
                });
                if !prev_intersects && registry.collidable(cache.block(block_pos)?) {
                    displacement.z += z as f32 + 1.0 - aabb.min.z;
                    break 'outer_pz;
                }
            }
        }
    }

    if delta.x > 0.0 {
        // for -X face, only check the bottom of the collision box (YZ-plane slice)
        'outer_nx: for y in prev_aabb.min.y.floor() as i32..=prev_aabb.max.y.floor() as i32 {
            for z in prev_aabb.min.z.floor() as i32..=prev_aabb.max.z.floor() as i32 {
                let x = aabb.max.x.floor() as i32;
                let block_pos = BlockPos { x, y, z };

                let prev_intersects = block_aabb(block_pos).intersects(&Aabb {
                    min: prev_aabb.min.map(f32::floor),
                    max: prev_aabb.max.map(f32::ceil),
                });
                if !prev_intersects && registry.collidable(cache.block(block_pos)?) {
                    displacement.x += x as f32 - aabb.max.x;
                    break 'outer_nx;
                }
            }
        }
    }

    if delta.y > 0.0 {
        // for -Y face, only check the bottom of the collision box (XZ-plane slice)
        'outer_ny: for x in prev_aabb.min.x.floor() as i32..=prev_aabb.max.x.floor() as i32 {
            for z in prev_aabb.min.z.floor() as i32..=prev_aabb.max.z.floor() as i32 {
                let y = aabb.max.y.floor() as i32;
                let block_pos = BlockPos { x, y, z };

                let prev_intersects = block_aabb(block_pos).intersects(&Aabb {
                    min: prev_aabb.min.map(f32::floor),
                    max: prev_aabb.max.map(f32::ceil),
                });
                if !prev_intersects && registry.collidable(cache.block(block_pos)?) {
                    displacement.y += y as f32 - aabb.max.y;
                    break 'outer_ny;
                }
            }
        }
    }

    if delta.z > 0.0 {
        // for -Z face, only check the bottom of the collision box (XY-plane slice)
        'outer_nz: for x in prev_aabb.min.x.floor() as i32..=prev_aabb.max.x.floor() as i32 {
            for y in prev_aabb.min.y.floor() as i32..=prev_aabb.max.y.floor() as i32 {
                let z = aabb.max.z.floor() as i32;
                let block_pos = BlockPos { x, y, z };

                let prev_intersects = block_aabb(block_pos).intersects(&Aabb {
                    min: prev_aabb.min.map(f32::floor),
                    max: prev_aabb.max.map(f32::ceil),
                });
                if !prev_intersects && registry.collidable(cache.block(block_pos)?) {
                    displacement.z += z as f32 - aabb.max.z;
                    break 'outer_nz;
                }
            }
        }
    }

    Some(displacement)
}

fn resolve_terrain_collisions(
    world: &Arc<VoxelWorld>,
    aabb: &Aabb,
    prev_aabb: &Aabb,
    transform: &mut Transform,
) {
    let mut cache = ChunkSnapshotCache::new(world);
    if let Some(positive_contrib) =
        resolve_terrain_collisions_positive_face(&mut cache, &world.registry, aabb, prev_aabb)
    {
        transform.translation.vector += positive_contrib;
    } else {
        log::debug!("not resolved!!");
        // revert movement to previous state if we didn't resolve the collision;
        // probably because of an unloaded chunk.
        transform.translation.vector += prev_aabb.center() - aabb.center();
    }
}

pub struct PreviousCollider {
    aabb_world: Aabb,
}

// should happen after most code that deals with transforms happens.
#[legion::system]
pub fn terrain_collision(
    #[resource] Dt(dt): &Dt,
    #[resource] voxel_world: &Arc<VoxelWorld>,
    world: &mut SubWorld,
    query: &mut Query<(&AabbCollider, &PreviousCollider, &mut Transform)>,
) {
    query.for_each_mut(world, |(collider, previous_collider, transform)| {
        resolve_terrain_collisions(
            voxel_world,
            &collider.aabb.transformed(transform),
            &previous_collider.aabb_world,
            transform,
        );
    });
}

#[legion::system(for_each)]
pub fn apply_gravity(rigidbody: &mut RigidBody) {
    rigidbody.acceleration.y -= 8.0;
}

#[legion::system(for_each)]
pub fn apply_rigidbody_motion(
    #[resource] Dt(dt): &Dt,
    rigidbody: &mut RigidBody,
    transform: &mut Transform,
) {
    let dt = dt.as_secs_f32();

    let a = rigidbody.acceleration;
    rigidbody.acceleration = vector![0.0, 0.0, 0.0];

    let dv = a * dt;
    rigidbody.velocity += dv;

    let dp = rigidbody.velocity * dt;
    transform.translation.vector += dp;
}

#[legion::system]
pub fn update_previous_colliders(
    cmd: &mut CommandBuffer,
    world: &mut SubWorld,
    query: &mut Query<(Entity, &AabbCollider, &Transform)>,
) {
    query.for_each_mut(world, |(&entity, collider, transform)| {
        cmd.add_component(entity, PreviousCollider {
            aabb_world: collider.aabb.transformed(transform),
        });
    });
}
