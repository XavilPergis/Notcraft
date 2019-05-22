#[macro_use]
extern crate log;
#[macro_use]
extern crate serde_derive;

pub mod engine;
pub mod handle;
pub mod util;

use crate::engine::{
    audio::AudioManager,
    components as comp,
    input::{keys, InputHandler, InputState},
    render::{
        camera::{ActiveCamera, Camera},
        mesher::ChunkMesher,
    },
    resources as res,
    world::{VoxelWorld, WorldPos},
};
use shrev::EventChannel;
use specs::prelude::*;
pub use specs::shred;
use std::time::Duration;

fn main() {
    simple_logger::init_with_level(log::Level::Debug).unwrap();

    let mut event_loop = glium::glutin::EventsLoop::new();

    let mut world = World::default();
    world.register::<comp::Player>();
    // world.register::<crate::engine::physics::RigidBody>();
    // world.register::<crate::engine::physics::Collidable>();
    world.register::<engine::render::mesher::TerrainMesh>();
    world.register::<comp::Parent>();
    world.register::<comp::Transform>();
    world.register::<comp::GlobalTransform>();
    world.register::<Camera>();

    let mut window_events = shrev::EventChannel::new();

    // world.register::<Handle<::engine::render::terrain::GpuChunkMesh>>();

    let registry = BlockRegistry::load_from_file("resources/blocks.json").unwrap();
    let voxel_world = VoxelWorld::new(registry);

    let player = world
        .create_entity()
        .with(comp::Player)
        .with(comp::Transform::default())
        // .with(Collidable {
        //     aabb: Aabb3::new(Point3::new(-0.4, -1.6, -0.4), Point3::new(0.4, 0.2, 0.4)),
        // })
        // .with(RigidBody {
        //     mass: 100.0,
        //     drag: na::Vector3::new(3.0, 6.0, 3.0),
        //     velocity: na::Vector3::new(0.0, 0.0, 0.0),
        // })
        .build();

    let camera_entity = world
        .create_entity()
        .with(comp::Parent(player))
        .with(Camera::default())
        .with(comp::Transform::default())
        .build();

    world.add_resource(ActiveCamera(Some(camera_entity)));

    fn duration_as_ms(duration: Duration) -> f64 {
        (duration.as_secs() as f64 + duration.subsec_nanos() as f64 * 1e-9) * 1000.0
    }

    struct TraceSystem<S> {
        inner: S,
        name: &'static str,
        samples: Vec<Duration>,
    }

    impl<S> TraceSystem<S> {
        fn new(inner: S, name: &'static str) -> Self {
            TraceSystem {
                inner,
                name,
                samples: Vec::new(),
            }
        }
    }

    impl<'a, S> System<'a> for TraceSystem<S>
    where
        S: System<'a>,
    {
        type SystemData = S::SystemData;

        fn run(&mut self, data: Self::SystemData) {
            if self.samples.len() >= 100 {
                let len = self.samples.len() as f64;
                let sum: f64 = self.samples.drain(..).map(duration_as_ms).sum();

                log::trace!(
                    "Timing: Took {} ms on average for system \"{}\" ",
                    sum / len,
                    self.name,
                );
            }

            let before = Instant::now();
            self.inner.run(data);
            let after = Instant::now();

            self.samples.push(after - before);
        }
    }

    fn attach_system<'a, 'b, T>(
        builder: DispatcherBuilder<'a, 'b>,
        sys: T,
        name: &'static str,
        deps: &[&str],
    ) -> DispatcherBuilder<'a, 'b>
    where
        T: for<'c> System<'c> + Send + 'a,
    {
        builder.with(TraceSystem::new(sys, name), name, deps)
    }

    fn attach_system_sync<'a, 'b, T>(
        builder: DispatcherBuilder<'a, 'b>,
        sys: T,
        name: &'static str,
    ) -> DispatcherBuilder<'a, 'b>
    where
        T: for<'c> System<'c> + 'b,
    {
        builder.with_thread_local(TraceSystem::new(sys, name))
    }

    let mut builder = DispatcherBuilder::new();
    builder = attach_system(builder, AudioManager::new(), "audio manager", &[]);
    // builder = attach_system(
    //     builder,
    //     crate::engine::physics::Physics::new(),
    //     "physics",
    //     &[],
    // );

    builder = attach_system(builder, ChunkMesher::default(), "chunk mesher", &[]);
    builder = attach_system(
        builder,
        comp::TransformHierarchyManager::new(),
        "global transform manager",
        &["chunk mesher"],
    );
    builder = attach_system_sync(
        builder,
        InputHandler::new(&mut window_events),
        "input handler",
    );

    let mut dispatcher = builder.build();

    dispatcher.setup(&mut world.res);

    world.add_resource(res::StopGameLoop(false));
    world.add_resource(window_events);
    world.add_resource(res::Dt(Duration::from_secs(1)));
    world.add_resource(voxel_world);
    // world.add_resource(DebugAccumulator::default());

    println!("World set up");

    use crate::engine::world::block::BlockRegistry;
    use std::time::Instant;

    let mut samples = vec![];

    let mut renderer = engine::render::renderer::Renderer::new(&event_loop, &mut world).unwrap();

    while !world.res.fetch::<res::StopGameLoop>().0 {
        let frame_start = Instant::now();

        world.exec(
            |mut channel: Write<'_, EventChannel<glium::glutin::Event>>| {
                event_loop.poll_events(|event| channel.single_write(event))
            },
        );

        world.exec(
            |(input, mut transforms): (
                ReadExpect<'_, InputState>,
                WriteStorage<'_, comp::Transform>,
                // WriteStorage<'a, comp::RigidBody>,
            )| {
                use std::f32::consts::PI;
                let (dx, dy) = input.cursor_delta();
                if let Some(mut transform) = transforms.get_mut(player) {
                    if dx != 0.0 || dy != 0.0 {
                        println!("cursor movement: {}, {}", dx, dy);
                    }

                    // X - roll -> pitch
                    // Y - pitch -> yaw
                    // Z - yaw -> roll
                    let pitch = dy * (PI / 180.0);
                    let yaw = dx * (PI / 180.0);

                    // roll pitch yaw
                    // let rot = nalgebra::UnitQuaternion::from_euler_angles(pitch, yaw, 0.0);
                    // transform.iso.rotation = nalgebra::UnitQuaternion::face_towards(
                    //     &transform.iso.translation.vector,
                    //     &nalgebra::Vector3::y(),
                    // );
                    // let rot = nalgebra::UnitQuaternion::from_euler_angles(pitch, yaw, 0.0);
                    transform.rotation.x -= pitch;
                    transform.rotation.x = f32::min(transform.rotation.x, PI / 2.0);
                    transform.rotation.x = f32::max(transform.rotation.x, -PI / 2.0);
                    transform.rotation.y -= yaw;

                    if input.is_pressed(keys::ARROW_UP, None) {
                        transform.rotation.z += 1.0 * (PI / 180.0);
                    }
                    if input.is_pressed(keys::ARROW_DOWN, None) {
                        transform.rotation.z -= 1.0 * (PI / 180.0);
                    }
                    // if input.is_rising(keys::ARROW_LEFT, None) {
                    //     let rot = nalgebra::UnitQuaternion::from_euler_angles(
                    //         0.0,
                    //         5.0 * (PI / 180.0),
                    //         0.0,
                    //     );
                    //     transform.rotate(&rot);
                    // }
                    // if input.is_rising(keys::ARROW_RIGHT, None) {
                    //     let rot = nalgebra::UnitQuaternion::from_euler_angles(
                    //         0.0,
                    //         -5.0 * (PI / 180.0),
                    //         0.0,
                    //     );
                    //     transform.rotate(&rot);
                    // }

                    if input.is_pressed(keys::FORWARD, None) {
                        comp::creative_flight(&mut transform, nalgebra::Vector2::new(0.0, -0.05));
                    }
                    if input.is_pressed(keys::BACKWARD, None) {
                        comp::creative_flight(&mut transform, nalgebra::Vector2::new(0.0, 0.05));
                    }
                    if input.is_pressed(keys::RIGHT, None) {
                        comp::creative_flight(&mut transform, nalgebra::Vector2::new(0.05, 0.0));
                    }
                    if input.is_pressed(keys::LEFT, None) {
                        comp::creative_flight(&mut transform, nalgebra::Vector2::new(-0.05, 0.0));
                    }
                    if input.is_pressed(keys::UP, None) {
                        transform.translate_global(&nalgebra::Vector3::new(0.0, 0.05, 0.0).into());
                    }
                    if input.is_pressed(keys::DOWN, None) {
                        transform.translate_global(&nalgebra::Vector3::new(0.0, -0.05, 0.0).into());
                    }
                }
            },
        );

        world.exec(
            |(mut world, players, transforms): (
                WriteExpect<'_, VoxelWorld>,
                ReadStorage<'_, comp::Player>,
                ReadStorage<'_, comp::Transform>,
            )| {
                for (_, transform) in (&players, &transforms).join() {
                    let pos = WorldPos(transform.translation.vector.into()).into();
                    crate::engine::world::load_chunks(&mut world, pos, 5);
                }
            },
        );

        // Update systems and the world.
        world.maintain();
        world.res.insert(res::StopGameLoop(false));
        dispatcher.dispatch(&world.res);

        renderer.draw(&mut world).unwrap();

        let processing_end = Instant::now();

        // TODO: Swap the backbuffer

        let frame_end = Instant::now();
        let dt = frame_end - frame_start;
        *world.write_resource::<res::Dt>() = res::Dt(dt);

        samples.push(processing_end - frame_start);

        if samples.len() >= 1000 {
            let len = samples.len() as f64;
            let sum: f64 = samples.drain(..).map(duration_as_ms).sum();

            log::debug!(
                "Frame took {} ms on average ({} fps)",
                sum / len,
                1000.0 * len / sum
            );
        }
    }
}
