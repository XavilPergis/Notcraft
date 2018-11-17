use engine::prelude::*;
use engine::systems::debug_render::DebugAccumulator;
use engine::systems::debug_render::Shape;
use engine::world::chunk::SIZE;
use shrev::EventChannel;
use specs::prelude::*;

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash, Default)]
pub struct PlayerController;

impl<'a> System<'a> for PlayerController {
    type SystemData = (
        ReadStorage<'a, comp::Player>,
        WriteStorage<'a, comp::Transform>,
        WriteStorage<'a, comp::RigidBody>,
        ReadStorage<'a, comp::MoveDelta>,
        Read<'a, res::ActiveDirections>,
        WriteExpect<'a, DebugAccumulator>,
    );

    fn run(
        &mut self,
        (player, mut player_transform, mut rigidbody, move_delta, directions, debug): Self::SystemData,
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

        for (_, tfm, rigidbody) in (&player, &player_transform, &mut rigidbody).join() {
            if directions.front {
                rigidbody.velocity -= tfm.basis_vectors().0
            };
            if directions.back {
                rigidbody.velocity += tfm.basis_vectors().0
            };
            if directions.left {
                rigidbody.velocity -= tfm.basis_vectors().1
            };
            if directions.right {
                rigidbody.velocity += tfm.basis_vectors().1
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
