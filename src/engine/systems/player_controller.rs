use engine::{
    camera::Camera,
    prelude::*,
    render::debug::{DebugAccumulator, Shape},
    world::chunk::SIZE,
};
use specs::prelude::*;

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash, Default)]
pub struct PlayerController;

impl<'a> System<'a> for PlayerController {
    type SystemData = (
        ReadStorage<'a, comp::Player>,
        WriteStorage<'a, comp::Transform>,
        WriteStorage<'a, comp::RigidBody>,
        ReadStorage<'a, comp::MoveDelta>,
        ReadExpect<'a, Camera>,
        Read<'a, res::ActiveDirections>,
        WriteExpect<'a, DebugAccumulator>,
        ReadExpect<'a, res::Dt>,
    );

    fn run(
        &mut self,
        (player, mut player_transform, mut rigidbody, move_delta, camera, directions, debug, dt): Self::SystemData,
    ) {
        let mut section = debug.section("chunk grid");
        for (_, tfm, move_delta) in (&player, &mut player_transform, &move_delta).join() {
            tfm.position += move_delta.0;
        }

        for (_, tfm) in (&player, &player_transform).join() {
            let cpos: ChunkPos = WorldPos(tfm.position).into();
            let center = cpos
                .base()
                .offset((SIZE as i32 / 2, SIZE as i32 / 2, SIZE as i32 / 2))
                .center();
            section.draw(Shape::GriddedChunk(
                2.0,
                cpos,
                Vector4::new(0.5, 0.5, 1.0, 1.0),
            ));
            section.draw(Shape::Line(
                5.0,
                center,
                Vector3::unit_x(),
                Vector4::new(1.0, 0.0, 0.0, 1.0),
            ));
            section.draw(Shape::Line(
                5.0,
                center,
                Vector3::unit_y(),
                Vector4::new(0.0, 1.0, 0.0, 1.0),
            ));
            section.draw(Shape::Line(
                5.0,
                center,
                Vector3::unit_z(),
                Vector4::new(0.0, 0.0, 1.0, 1.0),
            ));
        }

        let dt = dt.as_secs();

        for (_, rigidbody) in (&player, &mut rigidbody).join() {
            if directions.front {
                rigidbody.velocity += 20.0 * dt * camera.basis_vectors().0;
            };
            if directions.back {
                rigidbody.velocity -= 20.0 * dt * camera.basis_vectors().0;
            };
            if directions.left {
                rigidbody.velocity -= 20.0 * dt * camera.basis_vectors().1;
            };
            if directions.right {
                rigidbody.velocity += 20.0 * dt * camera.basis_vectors().1;
            };
            if directions.up {
                rigidbody.velocity += Vector3::unit_y();
            }
            if directions.down {
                rigidbody.velocity -= Vector3::unit_y();
            }
        }
    }
}
