use crate::prelude::*;
use nalgebra::{vector, Vector3};
use std::{ops::RangeInclusive, sync::Arc};

use super::{
    aabb::Aabb,
    transform::Transform,
    world::{
        chunk::ChunkAccess,
        registry::{BlockRegistry, CollisionType},
        BlockPos,
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
    access: &'a mut ChunkAccess,
    registry: Arc<BlockRegistry>,
    current: Aabb,
    previous: Aabb,
}

impl<'a> CollisionContext<'a> {
    fn new(access: &'a mut ChunkAccess, current: Aabb, previous: Aabb) -> Self {
        Self {
            registry: Arc::clone(access.registry()),
            access,
            current,
            previous,
        }
    }
}

fn does_block_collide(ctx: &mut CollisionContext, block_pos: BlockPos) -> Option<bool> {
    let prev_intersects = util::block_aabb(block_pos).intersects(&Aabb {
        min: ctx.previous.min.map(f32::floor),
        max: ctx.previous.max.map(f32::ceil),
    });

    match ctx
        .registry
        .get(ctx.access.block(block_pos)?)
        .collision_type()
    {
        CollisionType::Solid => Some(!prev_intersects),
        _ => Some(false),
    }
}

fn detect_terrain_collisions(ctx: &mut CollisionContext) -> Option<Vector3<f32>> {
    let delta = ctx.current.center() - ctx.previous.center();

    let mut resolution = vector![0.0, 0.0, 0.0];

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
                    // add_debug_box(DebugBox {
                    //     bounds: util::block_aabb(block_pos).inflate(0.003),
                    //     rgba: [1.0, 0.2, 0.2, 0.6],
                    //     kind: DebugBoxKind::Solid,
                    // });
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
                    // add_debug_box(DebugBox {
                    //     bounds: util::block_aabb(block_pos).inflate(0.003),
                    //     rgba: [0.2, 1.0, 0.2, 0.6],
                    //     kind: DebugBoxKind::Solid,
                    // });
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
                    // add_debug_box(DebugBox {
                    //     bounds: util::block_aabb(block_pos).inflate(0.003),
                    //     rgba: [0.2, 0.2, 1.0, 0.6],
                    //     kind: DebugBoxKind::Solid,
                    // });
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

fn detect_liquid_collisions(access: &mut ChunkAccess, prev: &Aabb) -> Option<bool> {
    let registry = Arc::clone(access.registry());
    for x in make_collision_range(prev.min.x, prev.max.x) {
        for y in make_collision_range(prev.min.y, prev.max.y) {
            for z in make_collision_range(prev.min.z, prev.max.z) {
                let block_pos = BlockPos { x, y, z };
                if registry
                    .get(access.block(block_pos)?)
                    .collision_type()
                    .is_liquid()
                {
                    return Some(true);
                }
            }
        }
    }

    Some(false)
}

fn do_terrain_collision(
    access: &mut ChunkAccess,
    collider: &mut AabbCollider,
    prev_collider: &PreviousCollider,
    rigidbody: &mut RigidBody,
    transform: &mut Transform,
) -> Option<()> {
    let original_aabb = prev_collider.aabb_world;
    let target_aabb = collider.aabb.transformed(transform);

    let original_pos =
        transform.translation.vector + (original_aabb.center() - target_aabb.center());
    let end_pos = transform.translation.vector;

    collider.in_liquid = detect_liquid_collisions(access, &original_aabb)?;

    // we set the entity's position back to the previous position, and then step
    // through in increments. if there are no collisions, we usually reach the final
    // position just as if we didn't modify the translation at all.
    transform.translation.vector = original_pos;

    const MAX_COLLISION_STEPS: usize = 32;
    const MAX_STEP_DISTANCE: f32 = 0.5;

    let desired_num_steps = {
        let len_x = (original_pos.x - end_pos.x).abs();
        let len_y = (original_pos.y - end_pos.y).abs();
        let len_z = (original_pos.z - end_pos.z).abs();

        let max_axis_length = f32::max(len_x, f32::max(len_y, len_z));
        (max_axis_length / MAX_STEP_DISTANCE) as usize
    };

    // for perf reasons, we probably want to limit the number of steps we take. note
    // that this means that for very large deltas, we will not end up moving the
    // full distance!
    // TODO: do we actually want to limit steps, or do we want to increase the size
    // of each step instead? or do we just want to skip steps after a certain point?
    let num_steps = usize::max(1, usize::min(desired_num_steps, MAX_COLLISION_STEPS));
    let step = (end_pos - original_pos) / (num_steps as f32);

    if desired_num_steps > MAX_COLLISION_STEPS {
        log::debug!(
            "desired number of collision detection steps ({desired_num_steps}) was limited"
        );
    }

    collider.on_ground = false;

    for _ in 0..num_steps {
        let prev_aabb = collider.aabb.transformed(transform);
        transform.translation.vector += step;
        let cur_aabb = collider.aabb.transformed(transform);

        let mut ctx = CollisionContext::new(access, cur_aabb, prev_aabb);
        let resolution = detect_terrain_collisions(&mut ctx)?;

        if resolution.magnitude_squared() > 0.0 {
            if resolution.x != 0.0 {
                rigidbody.velocity.x = 0.0;
                rigidbody.acceleration.x = 0.0;
            }
            if resolution.y != 0.0 {
                rigidbody.velocity.y = 0.0;
                rigidbody.acceleration.y = 0.0;
            }
            if resolution.z != 0.0 {
                rigidbody.velocity.z = 0.0;
                rigidbody.acceleration.z = 0.0;
            }

            collider.on_ground = resolution.y > 0.0;
            transform.translation.vector += resolution;

            break;
        }
    }

    Some(())
}

fn do_terrain_collision_wrapper(
    access: &mut ChunkAccess,
    collider: &mut AabbCollider,
    prev_collider: &PreviousCollider,
    rigidbody: &mut RigidBody,
    transform: &mut Transform,
) {
    let reverse = prev_collider.aabb_world.center() - collider.aabb.transformed(transform).center();
    let prev_pos = transform.translation.vector + reverse;
    if do_terrain_collision(access, collider, prev_collider, rigidbody, transform).is_none() {
        // revert movement to previous state if we didn't resolve the collision;
        // probably because of an unloaded chunk.
        transform.translation.vector = prev_pos;
    }
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
    mut access: ResMut<ChunkAccess>,
    query: Query<(
        &mut AabbCollider,
        &PreviousCollider,
        &mut RigidBody,
        &mut Transform,
    )>,
) {
    query.for_each_mut(
        |(mut collider, previous_collider, mut rigidbody, mut transform)| {
            do_terrain_collision_wrapper(
                &mut access,
                &mut collider,
                &previous_collider,
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
