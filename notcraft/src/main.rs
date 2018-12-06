#![feature(
    const_fn,
    trace_macros,
    nll,
    optin_builtin_traits,
    crate_visibility_modifier,
    duration_float,
    transpose_result,
    test
)]

#[macro_use]
extern crate log;
#[macro_use]
extern crate specs_derive;
#[macro_use]
extern crate serde_derive;

pub mod engine;
pub mod handle;
pub mod util;

use crate::engine::{
    audio::AudioManager,
    camera::Camera,
    components as comp,
    job::Worker,
    render::{
        debug::*,
        mesher::{ChunkMesher, Mesher},
        terrain::*,
        GraphicsData,
    },
    resources as res,
    systems::*,
    world::{
        block::{BlockRegistry, Faces},
        gen::*,
        ChunkPos, VoxelWorld,
    },
};
use cgmath::{Deg, Point3, Vector3};
use collision::Aabb3;
use glium::{
    texture::{RawImage2d, Texture2dArray},
    Surface,
};
use shrev::EventChannel;
use specs::prelude::*;
pub use specs::shred;
use std::{cell::RefCell, rc::Rc, time::Duration};

// mod benches {
//     use super::*;
//     use test::Bencher;

//     #[bench]
//     fn bench_mesher(bencher: &mut Bencher) {
//         let (registry, _) =
// BlockRegistry::load_from_file("resources/blocks.json").unwrap();         let
// mut world = VoxelWorld::new(registry);         let mut gen =
// NoiseGenerator::new_default();

//         for x in -1..=1 {
//             for y in -1..=1 {
//                 for z in -1..=1 {
//                     let pos = ChunkPos(Point3::new(x, y, z));
//                     world.set_chunk(pos, gen.compute(&pos));
//                 }
//             }
//         }

//         bencher.iter(|| {
//             let mut mesher = Mesher::new(ChunkPos(Point3::new(0, 0, 0)),
// &world);             mesher.mesh();
//         });
//     }

// }

fn main() {
    simple_logger::init_with_level(log::Level::Debug).unwrap();
    let mut events_loop = glium::glutin::EventsLoop::new();
    let window = glium::glutin::WindowBuilder::new()
        .with_title("Hello, world!")
        .with_dimensions(glium::glutin::dpi::LogicalSize::new(1024.0, 768.0));
    let context = glium::glutin::ContextBuilder::new().with_vsync(true);
    let display = glium::Display::new(window, context, &events_loop).unwrap();

    display.gl_window().grab_cursor(true).unwrap();
    println!("Context created!");

    let mut window_events = shrev::EventChannel::new();

    // let projection = ::cgmath::perspective(Deg(70.0), 600.0 / 600.0, 0.1,
    // 1000.0f32); debug_program.set_uniform(&mut ctx, "projection",
    // &projection);

    let mut world = World::default();

    // world.register::<Handle<::engine::render::terrain::GpuChunkMesh>>();
    world.register::<comp::Transform>();
    world.register::<comp::LookTarget>();
    world.register::<comp::ClientControlled>();
    world.register::<comp::Player>();
    world.register::<comp::RigidBody>();
    world.register::<comp::ChunkId>();
    world.register::<comp::DirtyMesh>();
    world.register::<comp::Collidable>();

    let (registry, tex_names) = BlockRegistry::load_from_file("resources/blocks.json").unwrap();
    let voxel_world = VoxelWorld::new(registry);

    let player_tfm = comp::Transform::default();
    world
        .create_entity()
        .with(comp::ClientControlled)
        .with(comp::Player)
        .with(player_tfm)
        .with(comp::Collidable {
            aabb: Aabb3::new(Point3::new(-0.4, -1.6, -0.4), Point3::new(0.4, 0.2, 0.4)),
        })
        .with(comp::RigidBody {
            mass: 100.0,
            drag: Vector3::new(3.0, 6.0, 3.0),
            velocity: Vector3::new(0.0, 0.0, 0.0),
        })
        .with(comp::LookTarget::default())
        .build();

    let graphics = Rc::new(RefCell::new(GraphicsData::new(
        Texture2dArray::new(
            &display,
            tex_names
                .into_iter()
                .map(|name| {
                    image::open(format!("resources/textures/{}", name)).map(|image| {
                        let image = image.to_rgba();
                        RawImage2d::from_raw_rgba_reversed(&image, image.dimensions())
                    })
                })
                .collect::<Result<_, _>>()
                .unwrap(),
        )
        .unwrap(),
    )));

    // let mut mesh_channel = EventChannel::<(ChunkPos, CpuChunkMesh)>::new();

    // let terrain_renderer = TerrainRenderSystem {
    //     ctx: ctx.clone(),
    //     pool,
    //     program,
    //     mesh_recv: mesh_channel.register_reader(),
    // };

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

                debug!(
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
    builder = attach_system(builder, ChunkUnloader::default(), "chunk unloader", &[]);
    builder = attach_system(builder, AudioManager::new(), "audio manager", &[]);
    builder = attach_system(
        builder,
        CameraRotationUpdater::new(&mut window_events),
        "cursor input handler",
        &[],
    );
    builder = attach_system(builder, PlayerController, "player controller", &[]);
    builder = attach_system(builder, Physics::new(), "physics", &[]);
    builder = attach_system(
        builder,
        BlockInteraction::new(&mut window_events),
        "block interactions",
        &["physics"],
    );
    builder = attach_system(builder, TerrainGenerator::new(), "terrain generator", &[]);

    builder = attach_system_sync(builder, ChunkMesher::new(&graphics), "chunk mesher");
    builder = attach_system_sync(
        builder,
        InputHandler::new(&mut window_events),
        "input handler",
    );

    let mut dispatcher = builder.build();

    dispatcher.setup(&mut world.res);

    // world.add_resource(debug_accumulator);
    world.add_resource(res::ActiveDirections::default());
    // world.add_resource(mesh_channel);
    world.add_resource(res::StopGameLoop(false));
    world.add_resource(window_events);
    world.add_resource(res::Dt(Duration::from_secs(1)));
    world.add_resource(Camera::default());

    world.add_resource(voxel_world);
    world.add_resource(DebugAccumulator::default());

    println!("World set up");

    use crate::engine::world::block::BlockRegistry;
    use std::time::Instant;

    let mut samples = vec![];

    let mut debug_renderer = DebugRenderer::new(&display).unwrap();
    let mut terrain_renderer = TerrainRenderer::new(&display, &graphics).unwrap();

    while !world.res.fetch::<res::StopGameLoop>().0 {
        let frame_start = Instant::now();

        world.exec(
            |mut channel: Write<'_, EventChannel<glium::glutin::Event>>| {
                events_loop.poll_events(|event| channel.single_write(event))
            },
        );

        // Update camera position and aspect ratio
        world.exec(
            |(mut camera, player, client, transforms): (
                Write<'_, Camera>,
                ReadStorage<'_, comp::Player>,
                ReadStorage<'_, comp::ClientControlled>,
                ReadStorage<'_, comp::Transform>,
            )| {
                for (tfm, _, _) in (&transforms, &player, &client).join() {
                    let pos = tfm.position;
                    let aspect = util::aspect_ratio(&display.gl_window()).unwrap();

                    camera.projection.aspect = aspect;
                    camera.position = pos;
                }
            },
        );

        // Update systems and the world.
        world.maintain();
        world.res.insert(res::StopGameLoop(false));
        dispatcher.dispatch(&world.res);

        graphics
            .borrow_mut()
            .update(&display, ChunkPos(Point3::new(0, 0, 0)));

        let mut frame = display.draw();

        frame.clear_color_and_depth((0.729411765, 0.907843137, 0.981568627, 1.0), 1.0);

        world.exec(
            |(mut accum, camera): (Write<'_, DebugAccumulator>, Read<'_, Camera>)| {
                debug_renderer.draw(&mut frame, &mut *accum, *camera);
            },
        );

        world.exec(|camera: Read<'_, Camera>| {
            terrain_renderer.draw(&mut frame, *camera);
        });

        let processing_end = Instant::now();

        // Swap the backbuffer
        frame.finish().unwrap();

        let frame_end = Instant::now();
        let dt = frame_end - frame_start;
        *world.write_resource::<res::Dt>() = res::Dt(dt);

        samples.push(processing_end - frame_start);

        if samples.len() >= 100 {
            let len = samples.len() as f64;
            let sum: f64 = samples.drain(..).map(duration_as_ms).sum();

            debug!(
                "Frame took {} ms on average ({} fps)",
                sum / len,
                1000.0 * len / sum
            );
        }
    }
}
