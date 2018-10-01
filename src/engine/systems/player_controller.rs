use specs::prelude::*;
use engine::components::*;
use cgmath::Vector3;

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash, Default)]
pub struct PlayerController;

impl<'a> System<'a> for PlayerController {
    type SystemData = (
        ReadStorage<'a, Player>,
        WriteStorage<'a, Transform>,
        WriteStorage<'a, RigidBody>,
        ReadStorage<'a, MoveDelta>,
        Read<'a, ActiveDirections>,
    );

    fn run(&mut self, (player, mut player_transform, mut rigidbody, move_delta, directions): Self::SystemData) {
        for (_, tfm, move_delta) in (&player, &mut player_transform, &move_delta).join() {
            tfm.position += move_delta.0;
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