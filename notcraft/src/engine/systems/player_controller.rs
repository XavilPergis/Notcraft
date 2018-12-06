use crate::engine::{
    camera::Camera,
    prelude::*,
    render::debug::{DebugAccumulator, Shape},
    systems::input::{keys, InputState},
    world::chunk::SIZE,
};
use specs::prelude::*;

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash, Default)]
pub struct PlayerController;

impl<'a> System<'a> for PlayerController {
    type SystemData = (
        ReadStorage<'a, comp::Player>,
        ReadStorage<'a, comp::ClientControlled>,
        WriteStorage<'a, comp::Transform>,
        WriteStorage<'a, comp::RigidBody>,
        ReadExpect<'a, InputState>,
        ReadExpect<'a, Camera>,
        ReadExpect<'a, res::Dt>,
        WriteExpect<'a, DebugAccumulator>,
    );

    fn run(
        &mut self,
        (player, client_controlled, mut transforms, mut rigidbodies, input, camera, dt, debug): Self::SystemData,
    ) {
        let mut section = debug.section("chunk grid");

        let dt = dt.as_secs();
        for (tfm, rb, _, _) in (
            &mut transforms,
            &mut rigidbodies,
            &player,
            &client_controlled,
        )
            .join()
        {
            if input.is_pressed(keys::FORWARD, None) {
                rb.velocity += 20.0 * dt * camera.basis_vectors().0;
            }
            if input.is_pressed(keys::BACKWARD, None) {
                rb.velocity -= 20.0 * dt * camera.basis_vectors().0;
            }
            if input.is_pressed(keys::LEFT, None) {
                rb.velocity -= 20.0 * dt * camera.basis_vectors().1;
            }
            if input.is_pressed(keys::RIGHT, None) {
                rb.velocity += 20.0 * dt * camera.basis_vectors().1;
            }
            if input.is_pressed(keys::UP, None) {
                rb.velocity += Vector3::unit_y();
            }
            if input.is_pressed(keys::DOWN, None) {
                rb.velocity -= Vector3::unit_y();
            }

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
    }
}
