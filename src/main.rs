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

extern crate cgmath;
extern crate collision;
extern crate gl;
extern crate glutin;
extern crate image;
extern crate ndarray as nd;
extern crate noise;
extern crate ordered_float;
extern crate rand;
extern crate rayon;
extern crate shrev;
extern crate specs;
#[macro_use]
extern crate log;
#[macro_use]
extern crate specs_derive;
#[macro_use]
extern crate shred_derive;
extern crate serde;
#[macro_use]
extern crate serde_derive;
extern crate crossbeam;
extern crate rodio;
extern crate serde_json;
extern crate simple_logger;
extern crate test;

// need this due to weird quirk of shred_derive
use engine::world::ChunkPos;
pub use specs::shred;

#[macro_use]
pub mod gl_api;
pub mod engine;
pub mod handle;
pub mod util;

use cgmath::{Deg, Point3, Vector3};
use collision::Aabb3;
use engine::{
    audio::AudioManager,
    camera::Camera,
    components as comp,
    job::Worker,
    render::{
        mesher::{ChunkMesher, CullMesher},
        ui::DrawCrosshair,
    },
    resources as res,
    world::{
        block::{BlockRegistry, Faces},
        gen::NoiseGenerator,
        VoxelWorld,
    },
};
use gl_api::{
    context::Context,
    misc,
    shader::{shader::ShaderError, *},
};
use glutin::{dpi::*, GlContext, GlWindow};
use shrev::EventChannel;
use specs::prelude::*;
use std::time::Duration;

mod benches {
    use super::*;
    use test::Bencher;

    #[bench]
    fn bench_mesher(bencher: &mut Bencher) {
        let (registry, _) = BlockRegistry::load_from_file("resources/blocks.json").unwrap();
        let mut world = VoxelWorld::new(registry);
        let mut gen = NoiseGenerator::new_default();

        for x in -1..=1 {
            for y in -1..=1 {
                for z in -1..=1 {
                    let pos = ChunkPos(Point3::new(x, y, z));
                    world.set_chunk(pos, gen.compute(&pos));
                }
            }
        }

        bencher.iter(|| {
            let mut mesher = CullMesher::new(ChunkPos(Point3::new(0, 0, 0)), &world);
            mesher.mesh();
        });
    }

}

fn main() {
    simple_logger::init_with_level(log::Level::Debug).unwrap();
    let mut events_loop = glutin::EventsLoop::new();
    let window = glutin::WindowBuilder::new()
        .with_title("Hello, world!")
        .with_dimensions(LogicalSize::new(1024.0, 768.0));
    let context = glutin::ContextBuilder::new().with_vsync(true);
    let gl_window = glutin::GlWindow::new(window, context, &events_loop).unwrap();

    gl_window.grab_cursor(true).unwrap();

    unsafe {
        gl_window.make_current().unwrap();
    }

    // Load OpenGL function pointers.
    // good *god* this function takes a long time fo compile
    let mut ctx = Context::load(|symbol| gl_window.get_proc_address(symbol));
    println!("Context created!");

    let mut debug_program = match simple_pipeline(
        &mut ctx,
        "resources/shaders/debug.vs",
        "resources/shaders/debug.fs",
    ) {
        Ok(prog) => prog,
        Err(msg) => match msg {
            PipelineError::Shader(ShaderError::Shader(msg)) => {
                println!("{}", msg);
                panic!()
            }
            _ => panic!("Other error"),
        },
    };

    gl_call!(assert Disable(gl::MULTISAMPLE));
    gl_call!(assert Enable(gl::DEPTH_TEST));
    gl_call!(assert DepthFunc(gl::LESS));
    gl_call!(assert Enable(gl::CULL_FACE));
    gl_call!(assert FrontFace(gl::CW));
    gl_call!(assert CullFace(gl::BACK));

    gl_call!(assert Enable(gl::BLEND));
    gl_call!(assert BlendFunc(gl::SRC_ALPHA, gl::ONE_MINUS_SRC_ALPHA));

    let mut window_events = shrev::EventChannel::new();

    let projection = ::cgmath::perspective(Deg(70.0), 600.0 / 600.0, 0.1, 1000.0f32);
    debug_program.set_uniform(&mut ctx, "projection", &projection);

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

    use engine::{
        render::{debug::*, terrain::*},
        systems::*,
        world::gen::*,
    };

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

    let terrain_renderer = TerrainRenderer::new(&mut ctx, tex_names);

    let (debug_rendering_system, debug_accumulator) = DebugRenderer::new(&mut ctx);

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
    builder = attach_system(builder, CameraUpdater::default(), "camera updater", &[]);
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
    builder = attach_system(
        builder,
        ChunkMesher::new(),
        "chunk mesher",
        &["terrain generator"],
    );

    builder = attach_system_sync(
        builder,
        InputHandler::new(&mut window_events),
        "input handler",
    );
    builder = attach_system_sync(builder, terrain_renderer, "terrain renderer");
    builder = attach_system_sync(builder, debug_rendering_system, "debug renderer");
    builder = attach_system_sync(builder, DrawCrosshair::new(&ctx), "crosshair renderer");

    let mut dispatcher = builder.build();

    dispatcher.setup(&mut world.res);

    world.add_resource(debug_accumulator);
    world.add_resource(res::ActiveDirections::default());
    // world.add_resource(mesh_channel);
    world.add_resource(res::StopGameLoop(false));
    world.add_resource(window_events);
    world.add_resource(res::Dt(Duration::from_secs(1)));
    world.add_resource(Camera::default());

    world.add_resource(voxel_world);
    world.add_resource(gl_window);

    println!("World set up");

    use engine::world::block::BlockRegistry;
    use std::time::Instant;

    let mut window_size =
        world.exec(|window: WriteExpect<'_, GlWindow>| window.get_inner_size().unwrap());

    world.exec(|window: WriteExpect<'_, GlWindow>| window.hide_cursor(true));

    let mut samples = vec![];

    while !world.res.fetch::<res::StopGameLoop>().0 {
        let frame_start = Instant::now();

        // The way I programmed objects allows the types to be send and sync, but I need
        // to do some funky stuff in the drop impl so we don't leak gpu resources. I
        // also need to call this every frame for the same reason.
        ctx.drop_deleted();

        // Update viewport dimensions if the window has been resized.
        world.exec(|window: WriteExpect<'_, GlWindow>| {
            let size = window.get_inner_size().unwrap();
            if size != window_size {
                window_size = size;
                let size: (u32, u32) = size.to_physical(window.get_hidpi_factor()).into();
                gl_call!(Viewport(0, 0, size.0 as i32, size.1 as i32))
                    .expect("Failed to set viewport size");
            }
        });

        misc::clear(misc::ClearMode::Color(
            0.729411765,
            0.907843137,
            0.981568627,
            1.0,
        ));
        misc::clear(misc::ClearMode::Depth(1.0));

        world.exec(|mut channel: Write<'_, EventChannel<glutin::Event>>| {
            events_loop.poll_events(|event| channel.single_write(event))
        });

        // Update systems and the world.
        world.maintain();
        world.res.insert(res::StopGameLoop(false));
        dispatcher.dispatch(&world.res);
        let processing_end = Instant::now();

        // Swap the backbuffer
        world.exec(|window: WriteExpect<'_, GlWindow>| window.swap_buffers().unwrap());
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
