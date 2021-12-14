// use crate::engine::{prelude::*, world::VoxelWorld};
// use specs::prelude::*;

// #[derive(Copy, Clone, Debug, PartialEq)]
// pub struct RigidBody {
//     pub mass: f32,
//     pub drag: Vector3<f32>,
//     pub velocity: Vector3<f32>,
// }

// impl Component for RigidBody {
//     type Storage = DenseVecStorage<Self>;
// }

// #[derive(Copy, Clone, Debug, PartialEq)]
// pub struct Collidable {
//     pub aabb: Aabb3<f32>,
// }

// impl Component for Collidable {
//     type Storage = DenseVecStorage<Self>;
// }

// pub struct Physics;

// impl Physics {
//     pub fn new() -> Self {
//         Physics
//     }
// }

// fn collidable_blocks_in_aabb(world: &VoxelWorld, aabb: Aabb3<f32>) ->
// Vec<BlockPos> {     let min: BlockPos = WorldPos(aabb.min).into();
//     let max: BlockPos = WorldPos(aabb.max).into();
//     let mut found = vec![];

//     for x in min.0.x..=max.0.x {
//         for y in min.0.y..=max.0.y {
//             for z in min.0.z..=max.0.z {
//                 let pos = BlockPos(Point3::new(x, y, z));
//                 if let Some(props) = world.registry(pos) {
//                     if props.collidable() {
//                         found.push(pos);
//                     }
//                 }
//             }
//         }
//     }

//     found
// }

// // take two ranges (like an aabb projected down to a single axis) and find
// how // far `a` needs to move so that the ranges do not overlap
// fn resolve_collision(a: Aabb3<f32>, b: Aabb3<f32>, axis: usize) -> f32 {
//     // the ranges are already disjoint, no resolution needs to be applied.
//     if !a.intersects(&b) {
//         return 0.0;
//     }

//     // find the center point of the ranges
//     let a_center = (a.min[axis] + a.max[axis]) / 2.0;
//     let b_center = (b.min[axis] + b.max[axis]) / 2.0;

//     // if a in on the "left" side of b, then we project out to the "left"
// side     // otherwise we project "right"
//     if a_center < b_center {
//         b.min[axis] - a.max[axis]
//     } else {
//         b.max[axis] - a.min[axis]
//     }
// }

// struct PhysicsStepContext<'a> {
//     world: &'a VoxelWorld,
//     pos: &'a mut comp::GlobalTransform,
//     body: &'a mut RigidBody,
//     collision_box: &'a Collidable,
//     dt: f32,
// }

// impl<'a> PhysicsStepContext<'a> {
//     fn entity_aabb(&self) -> Aabb3<f32> {
//         self.collision_box.aabb.add_v(Vector3 {
//             x: self.pos.x,
//             y: self.pos.y,
//             z: self.pos.z,
//         })
//     }
// }

// fn cube_aabb(pos: BlockPos) -> Aabb3<f32> {
//     let cube_base = Aabb3::new(Point3::new(0.0, 0.0, 0.0), Point3::new(1.0,
// 1.0, 1.0));     cube_base.add_v(crate::util::to_vector(pos.base().0))
// }

// // fn physics_step(ctx: &mut PhysicsStepContext, axis: usize, debug: &mut
// // DebugSection) {     ctx.body.velocity[axis]
// // }

// fn physics_step_x(ctx: &mut PhysicsStepContext) {
//     // Apply step along the X axis
//     ctx.body.velocity.x *= 1.0 / (1.0 + ctx.body.drag.x * ctx.dt);
//     ctx.pos.x += ctx.body.velocity.x * ctx.dt;

//     // get the possible collisions
//     let blocks = collidable_blocks_in_aabb(ctx.world, ctx.entity_aabb());
//     // debug.aabb(ctx.entity_aabb(), 5.0, Vector4::new(1.0, 0.0, 0.0, 1.0));

//     // let num_blocks = blocks.len();
//     // let mut dbg_aabb = |aabb, i| {
//     //     let val = i as f32 / num_blocks as f32;
//     //     debug.aabb(aabb, 5.0, Vector4::new(val, 0.0, 0.0, 1.0));
//     // };

//     // try to resolve the collisions
//     for (_, block) in blocks.iter().enumerate() {
//         let entity = ctx.entity_aabb();
//         let cube = cube_aabb(*block);

//         // dbg_aabb(entity, i);
//         // dbg_aabb(cube, i);

//         let resolution = resolve_collision(entity, cube, 0);

//         if resolution != 0.0 {
//             ctx.pos.x += resolution;
//             ctx.body.velocity.x = 0.0;
//         }
//     }
// }
// fn physics_step_y(ctx: &mut PhysicsStepContext) {
//     // Apply step along the Y axis
//     // ctx.body.velocity.y -= 25.0 * ctx.dt;
//     // ctx.body.velocity.y = ctx.body.velocity.y.min(50.0).max(-50.0);
//     ctx.body.velocity.y *= 1.0 / (1.0 + ctx.body.drag.y * ctx.dt);
//     ctx.pos.y += ctx.body.velocity.y * ctx.dt;

//     // get the possible collisions
//     let blocks = collidable_blocks_in_aabb(ctx.world, ctx.entity_aabb());
//     // debug.aabb(ctx.entity_aabb(), 5.0, Vector4::new(0.0, 1.0, 0.0, 1.0));

//     // let num_blocks = blocks.len();
//     // let mut dbg_aabb = |aabb, i| {
//     //     let val = i as f32 / num_blocks as f32;
//     //     debug.aabb(aabb, 5.0, Vector4::new(0.0, val, 0.0, 1.0));
//     // };

//     // try to resolve the collisions
//     for (_, block) in blocks.iter().enumerate() {
//         let entity = ctx.entity_aabb();
//         let cube = cube_aabb(*block);

//         // dbg_aabb(entity, i);
//         // dbg_aabb(cube, i);

//         let resolution = resolve_collision(entity, cube, 1);

//         if resolution != 0.0 {
//             ctx.pos.y += resolution;
//             ctx.body.velocity.y = 0.0;
//         }
//     }
// }
// fn physics_step_z(ctx: &mut PhysicsStepContext) {
//     // Apply step along the Y axis
//     ctx.body.velocity.z *= 1.0 / (1.0 + ctx.body.drag.z * ctx.dt);
//     ctx.pos.z += ctx.body.velocity.z * ctx.dt;

//     // get the possible collisions
//     let blocks = collidable_blocks_in_aabb(ctx.world, ctx.entity_aabb());
//     // debug.aabb(ctx.entity_aabb(), 5.0, Vector4::new(0.0, 0.0, 1.0, 1.0));

//     // let num_blocks = blocks.len();
//     // let mut dbg_aabb = |aabb, i| {
//     //     let val = i as f32 / num_blocks as f32;
//     //     debug.aabb(aabb, 5.0, Vector4::new(0.0, 0.0, val, 1.0));
//     // };

//     // try to resolve the collisions
//     for (_, block) in blocks.iter().enumerate() {
//         let entity = ctx.entity_aabb();
//         let cube = cube_aabb(*block);

//         // dbg_aabb(entity, i);
//         // dbg_aabb(cube, i);

//         let resolution = resolve_collision(entity, cube, 2);

//         if resolution != 0.0 {
//             ctx.pos.z += resolution;
//             ctx.body.velocity.z = 0.0;
//         }
//     }
// }

// impl<'a> System<'a> for Physics {
//     type SystemData = (
//         WriteStorage<'a, comp::Transform>,
//         ReadStorage<'a, comp::Parent>,
//         WriteStorage<'a, RigidBody>,
//         ReadStorage<'a, Collidable>,
//         ReadExpect<'a, VoxelWorld>,
//         Read<'a, res::Dt>,
//         // WriteExpect<'a, DebugAccumulator>,
//     );

//     fn run(&mut self, (mut positions, mut rigidbodies, collidables, world,
// dt): Self::SystemData) {         for (pos, rigidbody, collidable) in
//             (&mut positions, &mut rigidbodies, collidables.maybe()).join()
//         {
//             // let mut section = debug.section("physics");
//             let steps = if collidable.is_some() { 4 } else { 1 };
//             // adjusted dt for smaller steps when there are more of them
//             let dt = dt.as_secs() / steps as f32;

//             for _step in 0..steps {
//                 // apply_physics_step(&mut rigidbody, &mut transform, dt);
//                 if let Some(collidable) = collidable {
//                     let mut ctx = PhysicsStepContext {
//                         world: &world,
//                         body: rigidbody,
//                         collision_box: collidable,
//                         pos,
//                         dt,
//                     };

//                     physics_step_x(&mut ctx);
//                     physics_step_y(&mut ctx);
//                     physics_step_z(&mut ctx);
//                 }
//             }
//         }
//     }
// }
