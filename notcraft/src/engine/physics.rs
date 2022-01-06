use crate::{engine::prelude::*, util::block_aabb};
use nalgebra::{vector, Vector3};
use std::{ops::RangeInclusive, sync::Arc};

use super::{
    render::renderer::{add_debug_box, Aabb, DebugBox, DebugBoxKind},
    transform::Transform,
    world::{
        chunk::ChunkSnapshotCache,
        registry::{BlockRegistry, CollisionType},
        BlockPos, VoxelWorld,
    },
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
    pub on_ground: bool,
    pub in_liquid: bool,
}

impl AabbCollider {
    pub fn new(aabb: Aabb) -> Self {
        Self {
            aabb,
            on_ground: false,
            in_liquid: false,
        }
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

    resolution: Vector3<f32>,
    in_liquid: bool,
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

            resolution: vector![0.0, 0.0, 0.0],
            in_liquid: false,
        }
    }
}

fn does_block_collide(ctx: &mut CollisionContext, block_pos: BlockPos) -> Option<bool> {
    let prev_intersects = util::block_aabb(block_pos).intersects(&Aabb {
        min: ctx.previous.min.map(f32::floor),
        max: ctx.previous.max.map(f32::ceil),
    });

    match ctx.registry.collision_type(ctx.cache.block(block_pos)?) {
        CollisionType::Solid => Some(!prev_intersects),
        _ => Some(false),
    }
}

fn resolve_terrain_collisions(ctx: &mut CollisionContext) -> Option<()> {
    let delta = ctx.current.center() - ctx.previous.center();

    for x in make_collision_range(ctx.previous.min.x, ctx.previous.max.x) {
        for y in make_collision_range(ctx.previous.min.y, ctx.previous.max.y) {
            for z in make_collision_range(ctx.previous.min.z, ctx.previous.max.z) {
                let block_pos = BlockPos { x, y, z };
                ctx.in_liquid |= ctx
                    .registry
                    .collision_type(ctx.cache.block(block_pos)?)
                    .is_liquid();
            }
        }
    }

    // let proj_x = ctx.previous.translated(delta.x * Vector3::x());
    // let proj_y = ctx.previous.translated(delta.y * Vector3::y());
    // let proj_z = ctx.previous.translated(delta.z * Vector3::z());

    // add_debug_box(DebugBox {
    //     bounds: proj_x,
    //     rgba: [1.0, 0.0, 0.0, 0.6],
    //     kind: DebugBoxKind::Dashed,
    // });

    // add_debug_box(DebugBox {
    //     bounds: proj_y,
    //     rgba: [0.0, 1.0, 0.0, 0.6],
    //     kind: DebugBoxKind::Dashed,
    // });

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
                    add_debug_box(DebugBox {
                        bounds: util::block_aabb(block_pos).inflate(0.003),
                        rgba: [1.0, 0.2, 0.2, 0.6],
                        kind: DebugBoxKind::Solid,
                    });
                    ctx.resolution.x = match delta.x < 0.0 {
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
                    add_debug_box(DebugBox {
                        bounds: util::block_aabb(block_pos).inflate(0.003),
                        rgba: [0.2, 1.0, 0.2, 0.6],
                        kind: DebugBoxKind::Solid,
                    });
                    ctx.resolution.y = match delta.y < 0.0 {
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
                    add_debug_box(DebugBox {
                        bounds: util::block_aabb(block_pos).inflate(0.003),
                        rgba: [0.2, 0.2, 1.0, 0.6],
                        kind: DebugBoxKind::Solid,
                    });
                    ctx.resolution.z = match delta.z < 0.0 {
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

    if xz && delta.x != 0.0 && delta.z != 0.0 && ctx.resolution.x == 0.0 && ctx.resolution.z == 0.0
    {
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
                    ctx.resolution.z = match delta.z < 0.0 {
                        true => z as f32 + 1.0 - ctx.current.min.z,
                        false => z as f32 - ctx.current.max.z,
                    };
                } else {
                    ctx.resolution.x = match delta.x < 0.0 {
                        true => x as f32 + 1.0 - ctx.current.min.x,
                        false => x as f32 - ctx.current.max.x,
                    };
                }
            }
        }
    }

    if yz && delta.y != 0.0 && delta.z != 0.0 && ctx.resolution.y == 0.0 && ctx.resolution.z == 0.0
    {
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
                    ctx.resolution.z = match delta.z < 0.0 {
                        true => z as f32 + 1.0 - ctx.current.min.z,
                        false => z as f32 - ctx.current.max.z,
                    };
                } else {
                    ctx.resolution.y = match delta.y < 0.0 {
                        true => y as f32 + 1.0 - ctx.current.min.y,
                        false => y as f32 - ctx.current.max.y,
                    };
                }
            }
        }
    }

    if xy && delta.x != 0.0 && delta.y != 0.0 && ctx.resolution.x == 0.0 && ctx.resolution.y == 0.0
    {
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
                    ctx.resolution.y = match delta.y < 0.0 {
                        true => y as f32 + 1.0 - ctx.current.min.y,
                        false => y as f32 - ctx.current.max.y,
                    };
                } else {
                    ctx.resolution.x = match delta.x < 0.0 {
                        true => x as f32 + 1.0 - ctx.current.min.x,
                        false => x as f32 - ctx.current.max.x,
                    };
                }
            }
        }
    }

    Some(())
}

fn resolve_terrain_collisions_main(
    ctx: &mut CollisionContext,
    collider: &mut AabbCollider,
    rigidbody: &mut RigidBody,
    transform: &mut Transform,
) {
    if resolve_terrain_collisions(ctx).is_none() {
        let reverse = ctx.previous.center() - ctx.current.center();
        transform.translation.vector += reverse;
    }

    // revert movement to previous state if we didn't resolve the collision;
    // probably because of an unloaded chunk.
    if ctx.resolution.x != 0.0 {
        rigidbody.velocity.x = 0.0;
        rigidbody.acceleration.x = 0.0;
    }
    if ctx.resolution.y != 0.0 {
        rigidbody.velocity.y = 0.0;
        rigidbody.acceleration.y = 0.0;
    }
    if ctx.resolution.z != 0.0 {
        rigidbody.velocity.z = 0.0;
        rigidbody.acceleration.z = 0.0;
    }

    collider.on_ground = ctx.resolution.y > 0.0;
    collider.in_liquid = ctx.in_liquid;

    transform.translation.vector += ctx.resolution;
}

pub struct PreviousCollider {
    aabb_world: Aabb,
}

pub fn fix_previous_colliders(
    mut cmd: Commands,
    query: Query<(Entity, &AabbCollider, &Transform), Without<PreviousCollider>>,
) {
    query.for_each_mut(|(entity, collider, transform)| {
        cmd.entity(entity).insert(PreviousCollider {
            aabb_world: collider.aabb.transformed(transform),
        });
    });
}

pub fn update_previous_colliders(query: Query<(&AabbCollider, &Transform, &mut PreviousCollider)>) {
    query.for_each_mut(|(collider, transform, mut previous)| {
        previous.aabb_world = collider.aabb.transformed(transform);
    });
}

// should happen after most code that deals with transforms happens.
pub fn terrain_collision(
    voxel_world: Res<Arc<VoxelWorld>>,
    query: Query<(
        &mut AabbCollider,
        &PreviousCollider,
        &mut RigidBody,
        &mut Transform,
    )>,
) {
    let mut cache = ChunkSnapshotCache::new(&voxel_world);
    query.for_each_mut(
        |(mut collider, previous_collider, mut rigidbody, mut transform)| {
            let prev_aabb = collider.aabb.transformed(&transform);
            let mut ctx = CollisionContext::new(
                &mut cache,
                &voxel_world.registry,
                prev_aabb,
                previous_collider.aabb_world,
            );

            resolve_terrain_collisions_main(
                &mut ctx,
                &mut collider,
                &mut rigidbody,
                &mut transform,
            );
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

pub fn apply_gravity(query: Query<&mut RigidBody>) {
    query.for_each_mut(|mut rigidbody| {
        rigidbody.acceleration.y -= 27.0;
    });
}

pub fn apply_rigidbody_motion(time: Res<Time>, query: Query<(&mut RigidBody, &mut Transform)>) {
    query.for_each_mut(|(mut rigidbody, mut transform)| {
        let dt = time.delta_seconds();

        let a = rigidbody.acceleration;
        rigidbody.acceleration = vector![0.0, 0.0, 0.0];

        let dv = a * dt;
        rigidbody.velocity += dv;

        let dp = rigidbody.velocity * dt;
        transform.translation.vector += dp;
    });
}

#[derive(Debug, Default)]
pub struct PhysicsPlugin {}

impl Plugin for PhysicsPlugin {
    fn build(&self, app: &mut AppBuilder) {
        app.add_system(apply_gravity.system());
        app.add_system_to_stage(
            CoreStage::PostUpdate,
            apply_rigidbody_motion.system().label(MotionApplication),
        );
    }
}

#[derive(Debug, Default)]
pub struct CollisionPlugin {}

impl Plugin for CollisionPlugin {
    fn build(&self, app: &mut AppBuilder) {
        app.add_system_to_stage(
            CoreStage::PostUpdate,
            terrain_collision
                .system()
                .label(CollisionResolution)
                .after(MotionApplication),
        );
        app.add_system_to_stage(CoreStage::PreUpdate, fix_previous_colliders.system());
        app.add_system_to_stage(CoreStage::PreUpdate, update_previous_colliders.system());
    }
}

#[derive(Debug, Hash, PartialEq, Eq, Clone, SystemLabel)]
pub struct MotionApplication;

#[derive(Debug, Hash, PartialEq, Eq, Clone, SystemLabel)]
pub struct CollisionResolution;
