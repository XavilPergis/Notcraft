#![feature(
    const_fn,
    trace_macros,
    nll,
    optin_builtin_traits,
    crate_visibility_modifier
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
extern crate simple_logger;

// need this due to weird quirk of shred_derive
pub use specs::shred;

#[macro_use]
pub mod gl_api;
pub mod engine;
pub mod handle;
pub mod util;

use cgmath::{Deg, Point3, Vector3};
use collision::Aabb3;
use engine::{
    components as comp, render::mesher::ChunkMesher, resources as res, world::VoxelWorld,
};
use gl_api::context::Context;

use gl_api::{
    misc,
    shader::{shader::ShaderError, *},
};
use glutin::{dpi::*, GlContext, GlWindow};
use shrev::EventChannel;
use specs::prelude::*;
use std::time::Duration;

fn main() {
    simple_logger::init_with_level(log::Level::Debug).unwrap();
    let mut events_loop = glutin::EventsLoop::new();
    let window = glutin::WindowBuilder::new()
        .with_title("Hello, world!")
        .with_dimensions(LogicalSize::new(1024.0, 768.0));
    let context = glutin::ContextBuilder::new().with_vsync(true);
    let gl_window = glutin::GlWindow::new(window, context, &events_loop).unwrap();

    // gl_window.grab_cursor(true).unwrap();

    unsafe {
        gl_window.make_current().unwrap();
    }

    // Load OpenGL function pointers.
    // good *god* this function takes a long time fo compile
    let ctx = Context::load(|symbol| gl_window.get_proc_address(symbol));
    println!("Context created!");

    let mut debug_program = match simple_pipeline("resources/debug.vs", "resources/debug.fs") {
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

    let mut window_events = shrev::EventChannel::new();

    let projection = ::cgmath::perspective(Deg(70.0), 600.0 / 600.0, 0.1, 1000.0f32);
    debug_program.set_uniform("projection", &projection);

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

    let registry = BlockRegistry::new().with_defaults();
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

    let terrain_renderer = TerrainRenderer::new(&ctx);

    let (debug_rendering_system, debug_accumulator) = DebugRenderer::new(&ctx);

    let mut dispatcher = DispatcherBuilder::new()
        .with(ChunkUnloader::default(), "chunk unloader", &[])
        .with(
            LockCursor::new(&mut window_events),
            "cursor input handler",
            &[],
        )
        .with(PlayerController, "player controller", &[])
        .with(SmoothCamera, "smooth camera", &[])
        .with(Physics::new(), "physics", &[])
        .with(
            BlockInteraction::new(&mut window_events),
            "block interactions",
            &["physics"],
        )
        .with(TerrainGenerator::new(), "terrain generator", &[])
        .with(ChunkMesher::new(), "chunk mesher", &["terrain generator"])
        .with_thread_local(InputHandler::new(&mut window_events))
        .with_thread_local(terrain_renderer)
        .with_thread_local(debug_rendering_system)
        .build();

    dispatcher.setup(&mut world.res);

    world.add_resource(debug_accumulator);
    world.add_resource(res::ActiveDirections::default());
    // world.add_resource(mesh_channel);
    world.add_resource(res::StopGameLoop(false));
    world.add_resource(window_events);
    world.add_resource(res::Dt(Duration::from_secs(1)));
    world.add_resource(res::ViewFrustum {
        fov: Deg(80.0),
        near_plane: 0.01,
        far_plane: 1000.0,
    });

    world.add_resource(voxel_world);
    world.add_resource(gl_window);

    println!("World set up");

    use engine::world::block::BlockRegistry;
    use std::time::Instant;

    let mut window_size =
        world.exec(|window: WriteExpect<'_, GlWindow>| window.get_inner_size().unwrap());

    world.exec(|window: WriteExpect<'_, GlWindow>| window.hide_cursor(true));

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

        // Swap the backbuffer
        world.exec(|window: WriteExpect<'_, GlWindow>| window.swap_buffers().unwrap());
        let frame_end = Instant::now();
        let dt = frame_end - frame_start;
        *world.write_resource::<res::Dt>() = res::Dt(dt);
    }
}
