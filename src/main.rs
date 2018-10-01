#![feature(const_fn, trace_macros, nll, optin_builtin_traits, crate_visibility_modifier)]

extern crate gl;
extern crate glfw;
extern crate image;
extern crate cgmath;
extern crate noise;
extern crate collision;
extern crate rayon;
extern crate specs;
extern crate shrev;
extern crate rand;
extern crate ndarray as nd;
#[macro_use] extern crate lazy_static;

#[macro_use] pub mod gl_api;
pub mod engine;
pub mod util;
pub mod debug;
pub mod chunk_manager;
pub mod handle;

use engine::components as comp;
use engine::resources as res;
use engine::systems::mesher::{BlockVertex, ChunkMesher};
use engine::world::VoxelWorld;
use engine::mesh::{Mesh, GlMesh};

use gl_api::shader::program::LinkedProgram;
use gl_api::shader::shader::ShaderError;
use gl_api::shader::*;
use gl_api::misc;

use shrev::EventChannel;
use handle::{LocalPool, Handle};
use cgmath::{Matrix4, Deg, Vector3, Point3};
use glfw::{SwapInterval, Context, WindowEvent, WindowHint};
use collision::Aabb3;

use specs::shred::PanicHandler;
use specs::prelude::*;

use std::rc::Rc;
use std::time::Duration;
use std::sync::{Arc, Mutex};

impl Component for Handle<GlMesh<BlockVertex, u32>> {
    type Storage = DenseVecStorage<Self>;
}

pub struct TerrainRenderSystem {
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
        Read<'a, res::FramebufferSize>,
        Read<'a, res::ViewFrustum, PanicHandler>,
        Read<'a, EventChannel<(Entity, Mesh<BlockVertex, u32>)>>,
    );

    fn run(&mut self, (mut meshes, transforms, player_marker, client_controlled_marker, framebuffer_size, frustum, new_meshes): Self::SystemData) {
        let player_transform = (&player_marker, &client_controlled_marker, &transforms).join().map(|(_, _, tfm)| tfm).next();

        use gl_api::buffer::UsageType;

        for (entity, mesh) in new_meshes.read(&mut self.mesh_recv) {
            meshes.insert(*entity, self.pool.insert(mesh.to_gl_mesh(UsageType::StaticDraw).unwrap())).unwrap();
        }

        if let Some(player_transform) = player_transform {
            let aspect_ratio = framebuffer_size.x as f32 / framebuffer_size.y as f32;
            let projection = ::cgmath::perspective(Deg(frustum.fov.0 as f32), aspect_ratio, frustum.near_plane as f32, frustum.far_plane as f32);
            self.program.set_uniform("u_Projection", &projection);
            self.program.set_uniform("u_CameraPosition", &-player_transform.position.cast::<f32>().unwrap());
            for (mesh, tfm) in (&meshes, &transforms).join() {
                let mesh = self.pool.fetch(mesh);
                let tfm: Matrix4<f32> = tfm.as_matrix().cast::<f32>().unwrap();
                let view_matrix: Matrix4<f32> = player_transform.as_matrix().cast::<f32>().unwrap();
                self.program.set_uniform("u_View", &view_matrix);
                self.program.set_uniform("u_Transform", &tfm);
                // println!("{:?}", mesh);
                mesh.draw_with(&self.program).unwrap();
            }
        }
    }
}

fn main() {
    // let _remotery = remotery::init_remotery();
    let mut glfw = glfw::init(glfw::FAIL_ON_ERRORS).unwrap();
    println!("GLFW init");
    
    glfw.window_hint(WindowHint::ContextVersion(4, 5));
    glfw.window_hint(WindowHint::DepthBits(Some(24)));
    glfw.window_hint(WindowHint::Samples(Some(4)));

    let (mut window, events) = glfw.create_window(600, 600, "Not Minecraft", glfw::WindowMode::Windowed)
        .expect("Failed to create GLFW window.");
    println!("Window created");

    window.set_all_polling(true);
    window.make_current();
    glfw.set_swap_interval(SwapInterval::Sync(1));

    // Load OpenGL function pointers.
    // good *god* this function takes a long time fo compile
    gl::load_with(|symbol| window.get_proc_address(symbol) as *const _);
    println!("OpenGL symbols loaded");

    let mut program = match simple_pipeline("resources/terrain.vs", "resources/terrain.fs") {
        Ok(prog) => prog,
        Err(msg) => match msg {
            PipelineError::Shader(ShaderError::Shader(msg)) => { println!("{}", msg); panic!() },
            _ => panic!("Other error")
        }
    };
    let mut debug_program = match simple_pipeline("resources/debug.vs", "resources/debug.fs") {
        Ok(prog) => prog,
        Err(msg) => match msg {
            PipelineError::Shader(ShaderError::Shader(msg)) => { println!("{}", msg); panic!() },
            _ => panic!("Other error")
        }
    };

    unsafe {
        gl_call!(Disable(gl::MULTISAMPLE)).expect("glEnable failed");
        gl_call!(Enable(gl::DEPTH_TEST)).expect("glEnable failed");
        gl_call!(DepthFunc(gl::LESS)).expect("glDepthFunc failed");
        gl_call!(Enable(gl::CULL_FACE)).expect("glEnable failed");
        gl_call!(FrontFace(gl::CW)).expect("glFrontFace failed");
        gl_call!(CullFace(gl::BACK)).expect("glCullFace failed");
    }

    let window = Arc::new(Mutex::new(window));
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
    let mut player_tfm = comp::Transform::default();
    player_tfm.position.z -= 10.0; 
    world.create_entity()
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
    let terrain_renderer = TerrainRenderSystem { pool, program, mesh_recv: mesh_channel.register_reader() };

    use engine::systems::*;
    use engine::systems::terrain_gen::*;
    let mut dispatcher = DispatcherBuilder::new()
        .with_thread_local(ViewportUpdater::new(&window))
        .with_thread_local(InputHandler::new(&window, &mut window_events))
        .with_thread_local(PlayerController)
        .with_thread_local(SmoothCamera)
        .with_thread_local(RigidBodyUpdater)
        .with_thread_local(TerrainGenerator::new(NoiseGenerator::new_default()))
        .with_thread_local(ChunkMesher::new())
        .with_thread_local(terrain_renderer)
        .build();
    
    dispatcher.setup(&mut world.res);

    world.add_resource(mesh_channel);
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

    use std::time::Instant;
    use engine::world::block::BlockRegistry;

    while !world.res.fetch::<res::StopGameLoop>().0 {
        let frame_start = Instant::now();
        misc::clear(misc::ClearMode::Color(0.729411765, 0.907843137, 0.981568627, 1.0));
        misc::clear(misc::ClearMode::Depth(1.0));

        // Poll for new events, and fill the event buffer.
        glfw.poll_events();
        world.write_resource::<EventChannel<WindowEvent>>().iter_write(glfw::flush_messages(&events).map(|(_, event)| event).collect::<Vec<_>>());

        // Update systems and the world.
        world.maintain();
        world.res.insert(res::StopGameLoop(false));
        dispatcher.dispatch(&world.res);

        // Swap the backbuffer
        window.lock().unwrap().swap_buffers();
        let frame_end = Instant::now();
        let dt = frame_end - frame_start;
        *world.write_resource::<res::Dt>() = res::Dt(dt);
    }
}
