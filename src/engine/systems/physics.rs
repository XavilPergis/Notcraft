use specs::prelude::*;
use engine::components::*;
use engine::resources::*;

pub struct RigidBodyUpdater;

impl<'a> System<'a> for RigidBodyUpdater {
    type SystemData = (WriteStorage<'a, Transform>, WriteStorage<'a, RigidBody>, Read<'a, Dt>);

    fn run(&mut self, (mut transforms, mut rigidbodies, dt): Self::SystemData) {
        for (transform, rigidbody) in (&mut transforms, &mut rigidbodies).join() {
            let dt = dt.as_secs();
            transform.position += rigidbody.velocity * dt;
            rigidbody.velocity.x *= 1.0 / (1.0 + rigidbody.drag.x * dt);
            rigidbody.velocity.y *= 1.0 / (1.0 + rigidbody.drag.y * dt);
            rigidbody.velocity.z *= 1.0 / (1.0 + rigidbody.drag.z * dt);
        }
    }
}