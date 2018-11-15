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

use cgmath::{Deg, Matrix4, Point3, Vector3};
use collision::Aabb3;
use engine::components as comp;
use engine::mesh::{GlMesh, Mesh};
use engine::resources as res;
use engine::systems::mesher::{BlockVertex, ChunkMesher};
use engine::world::VoxelWorld;
use gl_api::context::Context;
use gl_api::context::Context as DrawContext;
use gl_api::misc;
use gl_api::shader::program::LinkedProgram;
use gl_api::shader::shader::ShaderError;
use gl_api::shader::*;
use glutin::dpi::*;
use glutin::GlContext;
use glutin::GlWindow;
use handle::{Handle, LocalPool};
use shrev::EventChannel;
use specs::prelude::*;
use specs::shred::PanicHandler;
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use std::time::Duration;

impl Component for Handle<GlMesh<BlockVertex, u32>> {
    type Storage = DenseVecStorage<Self>;
}

pub struct TerrainRenderSystem {
    ctx: DrawContext,
    pool: Rc<LocalPool<GlMesh<BlockVertex, u32>>>,
    program: LinkedProgram,
    mesh_recv: ReaderId<(Entity, Mesh<BlockVertex, u32>)>,
}

impl<'a> System<'a> for TerrainRenderSystem {
    type SystemData = (
        WriteStorage<'a, Handle<GlMesh<BlockVertex, u32>>>,
        ReadStorage<'a, comp::Transform>,
        ReadStorage<'a, comp::Player>,
        ReadStorage<'a, comp::ClientControlled>,
        ReadExpect<'a, GlWindow>,
        Read<'a, res::ViewFrustum, PanicHandler>,
        Read<'a, EventChannel<(Entity, Mesh<BlockVertex, u32>)>>,
    );

    fn run(
        &mut self,
        (
            mut meshes,
            transforms,
            player_marker,
            client_controlled_marker,
            window,
            frustum,
            new_meshes,
        ): Self::SystemData,
    ) {
        let player_transform = (&player_marker, &client_controlled_marker, &transforms)
            .join()
            .map(|(_, _, tfm)| tfm)
            .next();

        use gl_api::buffer::UsageType;

        for (entity, mesh) in new_meshes.read(&mut self.mesh_recv) {
            trace!("Inserted new mesh for entity #{:?}", entity);
            meshes
                .insert(
                    *entity,
                    self.pool
                        .insert(mesh.to_gl_mesh(&self.ctx, UsageType::StaticDraw).unwrap()),
                )
                .unwrap();
        }

        let mut counter = 0;
        if let Some(player_transform) = player_transform {
            let aspect_ratio = ::util::aspect_ratio(&window).unwrap() as f32;
            let projection = ::cgmath::perspective(
                Deg(frustum.fov.0 as f32),
                aspect_ratio,
                frustum.near_plane as f32,
                frustum.far_plane as f32,
            );
            self.program.set_uniform("u_Projection", &projection);
            self.program.set_uniform(
                "u_CameraPosition",
                &player_transform.position.cast::<f32>().unwrap(),
            );
            for (mesh, tfm) in (&meshes, &transforms).join() {
                let mesh = self.pool.fetch(mesh);
                let tfm: Matrix4<f32> = tfm.as_matrix().cast::<f32>().unwrap();
                let view_matrix: Matrix4<f32> = player_transform.as_matrix().cast::<f32>().unwrap();
                self.program.set_uniform("u_View", &view_matrix);
                self.program.set_uniform("u_Transform", &tfm);
                // println!("{:?}", mesh);
                mesh.draw_with(&self.program);
                counter += 1;
            }
        }
    }
}

fn main() {
    simple_logger::init().unwrap();
    let mut events_loop = glutin::EventsLoop::new();
    let window = glutin::WindowBuilder::new()
        .with_title("Hello, world!")
        .with_dimensions(LogicalSize::new(1024.0, 768.0));
    let context = glutin::ContextBuilder::new().with_vsync(true);
    let gl_window = glutin::GlWindow::new(window, context, &events_loop).unwrap();

    // gl_window.grab_cursor(true).unwrap();
    // let mut glfw = glfw::init(glfw::FAIL_ON_ERRORS).unwrap();

    // glfw.window_hint(WindowHint::ContextVersion(4, 5));
    // glfw.window_hint(WindowHint::DepthBits(Some(24)));
    // glfw.window_hint(WindowHint::Samples(Some(4)));

    // let (mut window, events) = glfw.create_window(600, 600, "Not Minecraft", glfw::WindowMode::Windowed)
    //     .expect("Failed to create GLFW window.");
    // println!("Window created");

    unsafe {
        gl_window.make_current().unwrap();
    }
    // glfw.set_swap_interval(SwapInterval::Sync(1));

    // Load OpenGL function pointers.
    // good *god* this function takes a long time fo compile
    let ctx = Context::load(|symbol| gl_window.get_proc_address(symbol));
    println!("Context created!");

    let mut program = match simple_pipeline("resources/terrain.vs", "resources/terrain.fs") {
        Ok(prog) => prog,
        Err(msg) => match msg {
            PipelineError::Shader(ShaderError::Shader(msg)) => {
                println!("{}", msg);
                panic!()
            }
            _ => panic!("Other error"),
        },
    };
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

    use gl_api::texture::*;
    let texture = Texture2D::new();
    texture.source_from_image("resources/textures.png").unwrap();
    texture.min_filter(MinimizationFilter::Nearest);
    texture.mag_filter(MagnificationFilter::Nearest);
    texture.texture_wrap_behavior(TextureAxis::R, WrapMode::Repeat);
    texture.texture_wrap_behavior(TextureAxis::S, WrapMode::Repeat);
    texture.set_texture_bank(0);

    let projection = ::cgmath::perspective(Deg(70.0), 600.0 / 600.0, 0.1, 1000.0f32);
    program.set_uniform("u_Time", &0.0f32);
    program.set_uniform("u_LightAmbient", &Vector3::<f32>::new(0.8, 0.8, 0.8));
    program.set_uniform("u_CameraPosition", &Vector3::new(0.0f32, 10.0, 0.0));
    program.set_uniform("u_TextureMap", &texture);

    debug_program.set_uniform("projection", &projection);

    let mut world = World::default();

    world.register::<Handle<GlMesh<BlockVertex, u32>>>();
    world.register::<comp::Transform>();
    world.register::<comp::LookTarget>();
    world.register::<comp::ClientControlled>();
    world.register::<comp::Player>();
    world.register::<comp::RigidBody>();
    world.register::<comp::ActiveDirections>();
    world.register::<comp::ChunkId>();
    world.register::<comp::DirtyMesh>();

    let voxel_world = VoxelWorld::default();

    let pool = Rc::new(LocalPool::default());
    let player_tfm = comp::Transform::default();
    world
        .create_entity()
        .with(comp::ClientControlled)
        .with(comp::Player)
        .with(player_tfm)
        .with(comp::RigidBody {
            mass: 100.0,
            drag: Vector3::new(3.0, 6.0, 3.0),
            velocity: Vector3::new(0.0, 0.0, 0.0),
            aabb: Aabb3::new(Point3::new(-1.0, -1.0, -1.0), Point3::new(1.0, 1.0, 1.0)),
        })
        .with(comp::ActiveDirections::default())
        .with(comp::LookTarget::default())
        .build();

    let mut mesh_channel = EventChannel::<(Entity, Mesh<BlockVertex, u32>)>::new();
    let terrain_renderer = TerrainRenderSystem {
        ctx: ctx.clone(),
        pool,
        program,
        mesh_recv: mesh_channel.register_reader(),
    };

    let mut debug_request_channel = EventChannel::new();

    use engine::systems::debug_render::*;
    use engine::systems::terrain_gen::*;
    use engine::systems::*;
    let mut dispatcher = DispatcherBuilder::new()
        .with(
            LockCursor::new(&mut window_events),
            "cursor input handler",
            &[],
        )
        .with(PlayerController, "player controller", &[])
        .with(SmoothCamera, "smooth camera", &[])
        .with(RigidBodyUpdater, "rigidbody updater", &[])
        .with(
            TerrainGenerator::new(NoiseGenerator::new_default()),
            "terrain generator",
            &[],
        )
        .with(ChunkMesher::new(), "chunk mesher", &["terrain generator"])
        .with_thread_local(InputHandler::new(&mut window_events))
        .with_thread_local(terrain_renderer)
        .with_thread_local(DebugRenderer::new(
            &ctx,
            debug_request_channel.register_reader(),
        ))
        .build();

    dispatcher.setup(&mut world.res);

    world.add_resource(mesh_channel);
    world.add_resource(debug_request_channel);
    world.add_resource(res::StopGameLoop(false));
    world.add_resource(window_events);
    world.add_resource(res::Dt(Duration::from_secs(1)));
    world.add_resource(res::ViewFrustum {
        fov: Deg(80.0),
        near_plane: 0.001,
        far_plane: 1000.0,
    });

    world.add_resource(voxel_world);
    let registry = BlockRegistry::new().with_defaults();
    world.add_resource(registry);
    world.add_resource(gl_window);

    println!("World set up");

    use engine::world::block::BlockRegistry;
    use std::time::Instant;

    let mut window_size =
        world.exec(|window: WriteExpect<'_, GlWindow>| window.get_inner_size().unwrap());

    world.exec(|window: WriteExpect<'_, GlWindow>| window.hide_cursor(true));

    while !world.res.fetch::<res::StopGameLoop>().0 {
        let frame_start = Instant::now();

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
