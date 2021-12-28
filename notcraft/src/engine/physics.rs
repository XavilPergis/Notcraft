use std::{
    collections::{hash_map::Entry, HashMap},
    ops::RangeInclusive,
    sync::Arc,
    time::Duration,
};

use legion::{systems::CommandBuffer, world::SubWorld, Entity, Query};
use nalgebra::{point, vector, Vector3};

use super::{
    render::renderer::{add_debug_box, add_transient_debug_box, Aabb, DebugBox, DebugBoxKind},
    transform::Transform,
    world::{
        chunk::{ChunkPos, ChunkSnapshot},
        registry::{BlockId, BlockRegistry},
        BlockPos, VoxelWorld,
    },
    Dt,
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

fn make_collision_bound(max: f32) -> i32 {
    if max.floor() == max {
        max.floor() as i32 - 1
    } else {
        max.floor() as i32
    }
}

fn make_collision_range(min: f32, max: f32) -> RangeInclusive<i32> {
    assert!(min < max);
    min.floor() as i32..=make_collision_bound(max)
}

struct CollisionContext<'a> {
    cache: &'a mut ChunkSnapshotCache,
    registry: &'a BlockRegistry,
    current: Aabb,
    previous: Aabb,
}

impl<'a> CollisionContext<'a> {
    fn new(
        cache: &'a mut ChunkSnapshotCache,
        registry: &'a BlockRegistry,
        current: Aabb,
        previous: Aabb,
    ) -> Self {
        Self {
            cache,
            registry,
            current,
            previous,
        }
    }
}

fn does_block_collide(ctx: &mut CollisionContext, block_pos: BlockPos) -> Option<bool> {
    let prev_intersects = block_aabb(block_pos).intersects(&Aabb {
        min: ctx.previous.min.map(f32::floor),
        max: ctx.previous.max.map(f32::ceil),
    });

    Some(!prev_intersects && ctx.registry.collidable(ctx.cache.block(block_pos)?))
}

fn resolve_terrain_collisions(ctx: &mut CollisionContext) -> Option<Vector3<f32>> {
    let delta = ctx.current.center() - ctx.previous.center();
    let mut resolution = vector![0.0, 0.0, 0.0];

    // let proj_x = ctx.previous.translated(delta.x * Vector3::x());
    // add_debug_box(DebugBox {
    //     bounds: proj_x,
    //     rgba: [1.0, 0.0, 0.0, 0.6],
    //     kind: DebugBoxKind::Dashed,
    // });

    // let proj_y = ctx.previous.translated(delta.y * Vector3::y());
    // add_debug_box(DebugBox {
    //     bounds: proj_y,
    //     rgba: [0.0, 1.0, 0.0, 0.6],
    //     kind: DebugBoxKind::Dashed,
    // });

    // let proj_z = ctx.previous.translated(delta.z * Vector3::z());
    // add_debug_box(DebugBox {
    //     bounds: proj_z,
    //     rgba: [0.0, 0.0, 1.0, 0.6],
    //     kind: DebugBoxKind::Dashed,
    // });

    {
        let x = match delta.x < 0.0 {
            true => ctx.current.min.x.floor() as i32,
            false => make_collision_bound(ctx.current.max.x),
        };
        for y in make_collision_range(ctx.previous.min.y, ctx.previous.max.y) {
            for z in make_collision_range(ctx.previous.min.z, ctx.previous.max.z) {
                let block_pos = BlockPos { x, y, z };
                if does_block_collide(ctx, block_pos)? {
                    add_transient_debug_box(Duration::from_secs(1), DebugBox {
                        bounds: block_aabb(block_pos),
                        rgba: [1.0, 0.2, 0.2, 0.6],
                        kind: DebugBoxKind::Solid,
                    });
                    resolution.x = match delta.x < 0.0 {
                        true => x as f32 + 1.0 - ctx.current.min.x,
                        false => x as f32 - ctx.current.max.x,
                    };
                }
            }
        }
    }

    {
        let y = match delta.y < 0.0 {
            true => ctx.current.min.y.floor() as i32,
            false => make_collision_bound(ctx.current.max.y),
        };
        for x in make_collision_range(ctx.previous.min.x, ctx.previous.max.x) {
            for z in make_collision_range(ctx.previous.min.z, ctx.previous.max.z) {
                let block_pos = BlockPos { x, y, z };
                if does_block_collide(ctx, block_pos)? {
                    add_transient_debug_box(Duration::from_secs(1), DebugBox {
                        bounds: block_aabb(block_pos),
                        rgba: [0.2, 1.0, 0.2, 0.6],
                        kind: DebugBoxKind::Solid,
                    });
                    resolution.y = match delta.y < 0.0 {
                        true => y as f32 + 1.0 - ctx.current.min.y,
                        false => y as f32 - ctx.current.max.y,
                    };
                }
            }
        }
    }

    {
        let z = match delta.z < 0.0 {
            true => ctx.current.min.z.floor() as i32,
            false => make_collision_bound(ctx.current.max.z),
        };
        for x in make_collision_range(ctx.previous.min.x, ctx.previous.max.x) {
            for y in make_collision_range(ctx.previous.min.y, ctx.previous.max.y) {
                let block_pos = BlockPos { x, y, z };
                if does_block_collide(ctx, block_pos)? {
                    add_transient_debug_box(Duration::from_secs(1), DebugBox {
                        bounds: block_aabb(block_pos),
                        rgba: [0.2, 0.2, 1.0, 0.6],
                        kind: DebugBoxKind::Solid,
                    });
                    resolution.z = match delta.z < 0.0 {
                        true => z as f32 + 1.0 - ctx.current.min.z,
                        false => z as f32 - ctx.current.max.z,
                    };
                }
            }
        }
    }

    // movement in the XZ plane but no collisions there mean we have to check for
    // "the" corner case, where, since no collisions are detected when moving
    // directly into an edge, that movement is allowed, and you end up clipping into
    // the block. in the next frame, that collision is dectected, and the player is
    // ejected from the block. this leads to a sort of stuck jittering.
    //
    // to mitigate this, we:
    //     A) check the corner we're moving into for potential collisions
    //     B) choose an axis to act like we slid on
    let dotx = delta.normalize().dot(&Vector3::x()).abs();
    let doty = delta.normalize().dot(&Vector3::y()).abs();
    let dotz = delta.normalize().dot(&Vector3::z()).abs();

    let yz = doty > dotx || dotz > dotx;
    let xz = dotx > doty || dotz > doty;
    let xy = dotx > dotz || doty > dotz;

    if xz && delta.x != 0.0 && delta.z != 0.0 && resolution.x == 0.0 && resolution.z == 0.0 {
        let x = match delta.x > 0.0 {
            true => make_collision_bound(ctx.current.max.x),
            false => ctx.current.min.x.floor() as i32,
        };
        let z = match delta.z > 0.0 {
            true => make_collision_bound(ctx.current.max.z),
            false => ctx.current.min.z.floor() as i32,
        };
        for y in make_collision_range(ctx.previous.min.y, ctx.previous.max.y) {
            let block_pos = BlockPos { x, y, z };
            if does_block_collide(ctx, block_pos)? {
                if dotx > dotz {
                    resolution.z = match delta.z < 0.0 {
                        true => z as f32 + 1.0 - ctx.current.min.z,
                        false => z as f32 - ctx.current.max.z,
                    };
                } else {
                    resolution.x = match delta.x < 0.0 {
                        true => x as f32 + 1.0 - ctx.current.min.x,
                        false => x as f32 - ctx.current.max.x,
                    };
                }
            }
        }
    }

    if yz && delta.y != 0.0 && delta.z != 0.0 && resolution.y == 0.0 && resolution.z == 0.0 {
        let y = match delta.y > 0.0 {
            true => make_collision_bound(ctx.current.max.y),
            false => ctx.current.min.y.floor() as i32,
        };
        let z = match delta.z > 0.0 {
            true => make_collision_bound(ctx.current.max.z),
            false => ctx.current.min.z.floor() as i32,
        };
        for x in make_collision_range(ctx.previous.min.x, ctx.previous.max.x) {
            let block_pos = BlockPos { x, y, z };
            if does_block_collide(ctx, block_pos)? {
                if doty > dotz {
                    resolution.z = match delta.z < 0.0 {
                        true => z as f32 + 1.0 - ctx.current.min.z,
                        false => z as f32 - ctx.current.max.z,
                    };
                } else {
                    resolution.y = match delta.y < 0.0 {
                        true => y as f32 + 1.0 - ctx.current.min.y,
                        false => y as f32 - ctx.current.max.y,
                    };
                }
            }
        }
    }

    if xy && delta.x != 0.0 && delta.y != 0.0 && resolution.x == 0.0 && resolution.y == 0.0 {
        let x = match delta.x > 0.0 {
            true => make_collision_bound(ctx.current.max.x),
            false => ctx.current.min.x.floor() as i32,
        };
        let y = match delta.y > 0.0 {
            true => make_collision_bound(ctx.current.max.y),
            false => ctx.current.min.y.floor() as i32,
        };
        for z in make_collision_range(ctx.previous.min.z, ctx.previous.max.z) {
            let block_pos = BlockPos { x, y, z };
            if does_block_collide(ctx, block_pos)? {
                if dotx > doty {
                    resolution.y = match delta.y < 0.0 {
                        true => y as f32 + 1.0 - ctx.current.min.y,
                        false => y as f32 - ctx.current.max.y,
                    };
                } else {
                    resolution.x = match delta.x < 0.0 {
                        true => x as f32 + 1.0 - ctx.current.min.x,
                        false => x as f32 - ctx.current.max.x,
                    };
                }
            }
        }
    }

    Some(resolution)
}

fn resolve_terrain_collisions_main(
    ctx: &mut CollisionContext,
    rigidbody: &mut RigidBody,
    transform: &mut Transform,
) {
    // revert movement to previous state if we didn't resolve the collision;
    // probably because of an unloaded chunk.
    if let Some(resolved) = resolve_terrain_collisions(ctx) {
        if resolved.x != 0.0 {
            rigidbody.velocity.x = 0.0;
            rigidbody.acceleration.x = 0.0;
        }
        if resolved.y != 0.0 {
            rigidbody.velocity.y = 0.0;
            rigidbody.acceleration.y = 0.0;
        }
        if resolved.z != 0.0 {
            rigidbody.velocity.z = 0.0;
            rigidbody.acceleration.z = 0.0;
        }
        transform.translation.vector += resolved;
    } else {
        let reverse = ctx.previous.center() - ctx.current.center();
        transform.translation.vector += reverse;
    }
}

pub struct PreviousCollider {
    aabb_world: Aabb,
}

// should happen after most code that deals with transforms happens.
#[legion::system]
pub fn terrain_collision(
    #[resource] voxel_world: &Arc<VoxelWorld>,
    world: &mut SubWorld,
    query: &mut Query<(
        &AabbCollider,
        &PreviousCollider,
        &mut RigidBody,
        &mut Transform,
    )>,
) {
    let mut cache = ChunkSnapshotCache::new(voxel_world);
    query.for_each_mut(
        world,
        |(collider, previous_collider, rigidbody, transform)| {
            let prev_aabb = collider.aabb.transformed(transform);
            let mut ctx = CollisionContext::new(
                &mut cache,
                &voxel_world.registry,
                prev_aabb,
                previous_collider.aabb_world,
            );

            resolve_terrain_collisions_main(&mut ctx, rigidbody, transform);
            // let post_aabb = collider.aabb.transformed(transform);

            // add_debug_box(DebugBox {
            //     bounds: prev_aabb,
            //     rgba: [0.8, 0.0, 1.0, 1.0],
            //     kind: DebugBoxKind::Dashed,
            // });
            // add_debug_box(DebugBox {
            //     bounds: post_aabb,
            //     rgba: [0.0, 0.8, 1.0, 1.0],
            //     kind: DebugBoxKind::Dashed,
            // });
        },
    );
}

#[legion::system(for_each)]
pub fn apply_gravity(rigidbody: &mut RigidBody) {
    rigidbody.acceleration.y -= 36.0;
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
