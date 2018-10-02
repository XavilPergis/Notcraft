use engine::world::chunk::SIZE;
use engine::systems::debug_render::Shape;
use shrev::EventChannel;
use specs::prelude::*;
use engine::components::*;
use cgmath::{Vector3, Vector4};

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash, Default)]
pub struct PlayerController;

impl<'a> System<'a> for PlayerController {
    type SystemData = (
        ReadStorage<'a, Player>,
        WriteStorage<'a, Transform>,
        WriteStorage<'a, RigidBody>,
        ReadStorage<'a, MoveDelta>,
        Read<'a, ActiveDirections>,
        WriteExpect<'a, EventChannel<Shape>>,
    );

    fn run(&mut self, (player, mut player_transform, mut rigidbody, move_delta, directions, mut debug_channel): Self::SystemData) {
        for (_, tfm, move_delta) in (&player, &mut player_transform, &move_delta).join() {
            tfm.position += move_delta.0;
        }

        for (_, tfm) in (&player, &player_transform).join() {
            let cpos = ::util::to_point(-tfm.position.cast().unwrap() / SIZE as i32);
            debug_channel.single_write(Shape::GriddedChunk(2.0, cpos, Vector4::new(0.5, 0.5, 1.0, 1.0)));
        }


        for (_, tfm, rigidbody) in (&player, &player_transform, &mut rigidbody).join() {
            if directions.front { rigidbody.velocity += tfm.basis_vectors().0 };
            if directions.back { rigidbody.velocity -= tfm.basis_vectors().0 };
            if directions.left { rigidbody.velocity += tfm.basis_vectors().1 };
            if directions.right { rigidbody.velocity -= tfm.basis_vectors().1 };
            if directions.up { rigidbody.velocity -= Vector3::unit_y(); }
            if directions.down { rigidbody.velocity += Vector3::unit_y(); }
        }
    }
}