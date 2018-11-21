use collision::{prelude::*, Aabb3};
use engine::{
    prelude::*,
    render::debug::{DebugAccumulator, DebugSection, Shape},
    world::VoxelWorld,
};

pub struct Physics;

impl Physics {
    pub fn new() -> Self {
        Physics
    }
}

fn collidable_blocks_in_aabb(world: &VoxelWorld, aabb: Aabb3<f64>) -> Vec<BlockPos> {
    let min: BlockPos = WorldPos(aabb.min).into();
    let max: BlockPos = WorldPos(aabb.max).into();
    let mut found = vec![];

    for x in min.0.x..=max.0.x {
        for y in min.0.y..=max.0.y {
            for z in min.0.z..=max.0.z {
                let pos = BlockPos(Point3::new(x, y, z));
                if let Some(props) = world.get_block_properties(pos) {
                    if props.collidable {
                        found.push(pos);
                    }
                }
            }
        }
    }

    found
}

// take two ranges (like an aabb projected down to a single axis) and find how
// far `a` needs to move so that the ranges do not overlap
fn resolve_collision(a: Aabb3<f64>, b: Aabb3<f64>, axis: usize) -> f64 {
    // the ranges are already disjoint, no resolution needs to be applied.
    if !a.intersects(&b) {
        return 0.0;
    }

    // find the center point of the ranges
    let a_center = (a.min[axis] + a.max[axis]) / 2.0;
    let b_center = (b.min[axis] + b.max[axis]) / 2.0;

    // if a in on the "left" side of b, then we project out to the "left" side
    // otherwise we project "right"
    if a_center < b_center {
        b.min[axis] - a.max[axis]
    } else {
        b.max[axis] - a.min[axis]
    }
}

struct PhysicsStepContext<'a> {
    world: &'a VoxelWorld,
    transform: &'a mut comp::Transform,
    body: &'a mut comp::RigidBody,
    collision_box: &'a comp::Collidable,
    dt: f64,
}

impl<'a> PhysicsStepContext<'a> {
    fn entity_aabb(&self) -> Aabb3<f64> {
        self.collision_box
            .aabb
            .add_v(::util::to_vector(self.transform.position))
    }
}

fn cube_aabb(pos: BlockPos) -> Aabb3<f64> {
    let cube_base = Aabb3::new(Point3::new(0.0, 0.0, 0.0), Point3::new(1.0, 1.0, 1.0));
    cube_base.add_v(::util::to_vector(pos.base().0))
}

fn physics_step_x(ctx: &mut PhysicsStepContext, debug: &mut DebugSection) {
    // Apply step along the X axis
    ctx.body.velocity.x *= 1.0 / (1.0 + ctx.body.drag.x * ctx.dt);
    ctx.transform.position.x += ctx.body.velocity.x * ctx.dt;

    // get the possible collisions
    let blocks = collidable_blocks_in_aabb(ctx.world, ctx.entity_aabb());
    debug.draw(Shape::Box(
        5.0,
        ctx.entity_aabb(),
        Vector4::new(1.0, 0.0, 0.0, 1.0),
    ));

    let num_blocks = blocks.len();
    let mut dbg_aabb = |aabb, i| {
        let val = i as f64 / num_blocks as f64;
        debug.draw(Shape::Box(5.0, aabb, Vector4::new(val, 0.0, 0.0, 1.0)));
    };

    // try to resolve the collisions
    for (i, block) in blocks.iter().enumerate() {
        let entity = ctx.entity_aabb();
        let cube = cube_aabb(*block);

        dbg_aabb(entity, i);
        dbg_aabb(cube, i);

        let resolution = resolve_collision(entity, cube, 0);

        if resolution != 0.0 {
            ctx.transform.position.x += resolution;
            ctx.body.velocity.x = 0.0;
        }
    }
}
fn physics_step_y(ctx: &mut PhysicsStepContext, debug: &mut DebugSection) {
    // Apply step along the Y axis
    ctx.body.velocity.y -= 25.0 * ctx.dt;
    ctx.body.velocity.y = ctx.body.velocity.y.min(50.0).max(-50.0);
    // ctx.body.velocity.y *= 1.0 / (1.0 + ctx.body.drag.y * ctx.dt);
    ctx.transform.position.y += ctx.body.velocity.y * ctx.dt;

    // get the possible collisions
    let blocks = collidable_blocks_in_aabb(ctx.world, ctx.entity_aabb());
    debug.draw(Shape::Box(
        5.0,
        ctx.entity_aabb(),
        Vector4::new(0.0, 1.0, 0.0, 1.0),
    ));

    let num_blocks = blocks.len();
    let mut dbg_aabb = |aabb, i| {
        let val = i as f64 / num_blocks as f64;
        debug.draw(Shape::Box(5.0, aabb, Vector4::new(0.0, val, 0.0, 1.0)));
    };

    // try to resolve the collisions
    for (i, block) in blocks.iter().enumerate() {
        let entity = ctx.entity_aabb();
        let cube = cube_aabb(*block);

        dbg_aabb(entity, i);
        dbg_aabb(cube, i);

        let resolution = resolve_collision(entity, cube, 1);

        if resolution != 0.0 {
            ctx.transform.position.y += resolution;
            ctx.body.velocity.y = 0.0;
        }
    }
}
fn physics_step_z(ctx: &mut PhysicsStepContext, debug: &mut DebugSection) {
    // Apply step along the Y axis
    ctx.body.velocity.z *= 1.0 / (1.0 + ctx.body.drag.z * ctx.dt);
    ctx.transform.position.z += ctx.body.velocity.z * ctx.dt;

    // get the possible collisions
    let blocks = collidable_blocks_in_aabb(ctx.world, ctx.entity_aabb());
    debug.draw(Shape::Box(
        5.0,
        ctx.entity_aabb(),
        Vector4::new(0.0, 0.0, 1.0, 1.0),
    ));

    let num_blocks = blocks.len();
    let mut dbg_aabb = |aabb, i| {
        let val = i as f64 / num_blocks as f64;
        debug.draw(Shape::Box(5.0, aabb, Vector4::new(0.0, 0.0, val, 1.0)));
    };

    // try to resolve the collisions
    for (i, block) in blocks.iter().enumerate() {
        let entity = ctx.entity_aabb();
        let cube = cube_aabb(*block);

        dbg_aabb(entity, i);
        dbg_aabb(cube, i);

        let resolution = resolve_collision(entity, cube, 2);

        if resolution != 0.0 {
            ctx.transform.position.z += resolution;
            ctx.body.velocity.z = 0.0;
        }
    }
}

impl<'a> System<'a> for Physics {
    type SystemData = (
        WriteStorage<'a, comp::Transform>,
        WriteStorage<'a, comp::RigidBody>,
        ReadStorage<'a, comp::Collidable>,
        ReadExpect<'a, VoxelWorld>,
        Read<'a, res::Dt>,
        WriteExpect<'a, DebugAccumulator>,
    );

    fn run(
        &mut self,
        (mut transforms, mut rigidbodies, collidables, world, dt, debug): Self::SystemData,
    ) {
        for (transform, rigidbody, collidable) in
            (&mut transforms, &mut rigidbodies, collidables.maybe()).join()
        {
            let mut section = debug.section("physics");
            let steps = if collidable.is_some() { 4 } else { 1 };
            // adjusted dt for smaller steps when there are more of them
            let dt = dt.as_secs() / steps as f64;

            for _step in 0..steps {
                // apply_physics_step(&mut rigidbody, &mut transform, dt);
                if let Some(collidable) = collidable {
                    let mut ctx = PhysicsStepContext {
                        world: &world,
                        body: rigidbody,
                        collision_box: collidable,
                        transform,
                        dt,
                    };

                    physics_step_x(&mut ctx, &mut section);
                    physics_step_y(&mut ctx, &mut section);
                    physics_step_z(&mut ctx, &mut section);
                }
            }
        }
    }
}
