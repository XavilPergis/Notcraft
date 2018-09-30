#![feature(const_fn, trace_macros, nll, optin_builtin_traits, crate_visibility_modifier)]

extern crate gl;
extern crate glfw;
extern crate image;
extern crate cgmath;
extern crate noise;
extern crate smallvec;
extern crate collision;
extern crate rayon;
extern crate specs;
extern crate shrev;
extern crate rand;
extern crate flame;
extern crate ndarray as nd;
#[macro_use] extern crate smallbitvec;
#[macro_use] extern crate lazy_static;

#[macro_use] pub mod gl_api;
pub mod engine;
pub mod util;
pub mod debug;
pub mod chunk_manager;
pub mod handle;

use engine::components::DirtyMesh;
use engine::components::ChunkId;
use engine::mesher::ChunkMesher;
use engine::world::block::BlockId;
use engine::world::VoxelWorld;
use engine::resources::Dt;
use std::time::Duration;
use engine::components::ActiveDirections;
use engine::components::RigidBody;
use specs::shred::PanicHandler;
use engine::resources::FramebufferSize;
use engine::resources::ViewFrustum;
use engine::components::ClientControlled;
use engine::components::Player;
use shrev::EventChannel;
use engine::resources::StopGameLoop;
use gl_api::shader::program::LinkedProgram;
use engine::mesher::BlockVertex;
use engine::mesh::{Mesh, GlMesh};
use std::rc::Rc;
use handle::{LocalPool, Handle};
use std::sync::Arc;
use std::sync::Mutex;
use engine::ChunkPos;
use engine::world::{chunk, Chunk};
use noise::NoiseFn;
use engine::terrain::ChunkGenerator;
use cgmath::{Matrix4, Deg, Vector3, Point3};
use glfw::{Context, WindowEvent, WindowHint};
use gl_api::shader::*;
use specs::prelude::*;
use gl_api::misc;
use engine::components::{Transform, LookTarget};
use collision::Aabb3;

// type Mesh = EngineMesh<BlockVertex, u32>;



// #[derive(Copy, Clone, Debug, Eq, PartialEq, Hash, Default)]
// pub struct Modes {
//     wireframe: bool,
//     capture_mouse: bool,
//     show_debug_frames: bool,
//     noclip: bool,
// }

use glfw::SwapInterval;
use gl_api::shader::shader::ShaderError;

// use std::sync::mpsc::Receiver;

// pub struct MultiReciever<T: 'static> {
//     queue: Vec<T>,
//     recv: Mutex<Receiver<T>>,
// }

// impl<T: 'static> From<Receiver<T>> for MultiReciever<T> {
//     fn from(recv: Receiver<T>) -> Self { MultiReciever { queue: Vec::new(), recv: Mutex::new(recv) } }
// }

// impl<T: 'static + Send> MultiReciever<T> {
//     pub fn fill_queue(&mut self) {
//         self.queue.extend(::glfw::flush_messages(&self.recv.lock().unwrap()));
//     }

//     pub fn items(&self) -> &[T] { &*self.queue }

//     pub fn clear(&mut self) {
//         self.queue.clear();
//     }
// }

// #[derive(Clone)]
// struct SharedEvents<T: 'static> {
//     inner: Arc<Mutex<MultiReciever<T>>>,
// }

// impl<T: 'static + Send> SharedEvents<T> {
//     pub fn new(recv: MultiReciever<T>) -> Self { SharedEvents { inner: Arc::new(Mutex::new(recv)) } }

//     pub fn events(&self) -> ::std::sync::MutexGuard<'_, MultiReciever<T>> { self.inner.lock().unwrap() }
// }

// #[derive(Clone)]
// pub struct SharedWindow {
//     inner: Arc<Mutex<Window>>,
// }

// impl SharedWindow {
//     pub fn new(window: Window) -> Self { SharedWindow { inner: Arc::new(Mutex::new(window)) } }
//     pub fn window(&self) -> ::std::sync::MutexGuard<'_, Window> { self.inner.lock().unwrap() }
// }

// type SharedWindowEvents = SharedEvents<(f64, ::glfw::WindowEvent)>;

// #[derive(Copy, Clone, Debug, Default)]
// pub struct MouseDelta(f64, f64);

// pub struct InputHandler {
//     window: SharedWindow,
//     events: SharedWindowEvents,
//     last_x: Option<f64>,
//     last_y: Option<f64>,
// }

// impl<'a> System<'a> for InputHandler {
//     type SystemData = (Write<'a, MouseDelta>, Write<'a, Modes>, Write<'a, StopGameLoop>);

//     fn run(&mut self, (mut mouse_delta, mut modes, mut stop_flag): Self::SystemData) {
//         let mut window = self.window.window();
//         for (_, event) in self.events.events().items() {
//             match event {
//                 WindowEvent::Close | WindowEvent::Key(Key::Escape, _, Action::Press, _) => { stop_flag.0 = true; break; },
//                 WindowEvent::Key(Key::F1, _, Action::Press, _) => modes.show_debug_frames = !modes.show_debug_frames,
//                 WindowEvent::Key(Key::F2, _, Action::Press, _) => { modes.wireframe = !modes.wireframe; set_wireframe(modes.wireframe); },
//                 WindowEvent::Key(Key::F3, _, Action::Press, _) => { modes.capture_mouse = !modes.capture_mouse; set_mouse_capture(&mut *window, modes.capture_mouse) },
//                 WindowEvent::Key(Key::F4, _, Action::Press, _) => modes.noclip = !modes.noclip,
//                 WindowEvent::CursorPos(x, y) => match (self.last_x, self.last_y) {
//                     (Some(lx), Some(ly)) => {
//                         *mouse_delta = MouseDelta(lx - x, ly - y);
//                     },
//                     _ => {
//                         self.last_x = Some(*x);
//                         self.last_y = Some(*y);
//                     }
//                 },
//                 _ => (),
//             }
//         }
//     }
// }

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
        ReadStorage<'a, Transform>,
        ReadStorage<'a, Player>,
        ReadStorage<'a, ClientControlled>,
        Read<'a, FramebufferSize>,
        Read<'a, ViewFrustum, PanicHandler>,
        Read<'a, EventChannel<(Entity, Mesh<BlockVertex, u32>)>>,
        Entities<'a>,
    );

    fn run(&mut self, (mut meshes, transforms, player_marker, client_controlled_marker, framebuffer_size, frustum, new_meshes, entities): Self::SystemData) {
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
    world.register::<Transform>();
    world.register::<LookTarget>();
    world.register::<ClientControlled>();
    world.register::<Player>();
    world.register::<RigidBody>();
    world.register::<ActiveDirections>();
    world.register::<ChunkId>();
    world.register::<DirtyMesh>();

    let mut voxel_world = VoxelWorld::default();

    let pool = Rc::new(LocalPool::default());
    let mut player_tfm = Transform::default();
    player_tfm.position.z -= 10.0; 
    world.create_entity()
        .with(ClientControlled)
        .with(Player)
        .with(player_tfm)
        .with(RigidBody {
            mass: 100.0,
            drag: Vector3::new(3.0, 6.0, 3.0),
            velocity: Vector3::new(0.0, 0.0, 0.0),
            aabb: Aabb3::new(Point3::new(-1.0, -1.0, -1.0), Point3::new(1.0, 1.0, 1.0)),
        })
        .with(ActiveDirections::default())
        .with(LookTarget::default())
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
        .with_thread_local(ChunkMesher {})
        .with_thread_local(terrain_renderer)
        .build();
    
    dispatcher.setup(&mut world.res);

    world.add_resource(mesh_channel);
    world.add_resource(StopGameLoop(false));
    world.add_resource(window_events);
    world.add_resource(Dt(Duration::from_secs(1)));
    world.add_resource(ViewFrustum {
        fov: Deg(80.0),
        near_plane: 0.001,
        far_plane: 1000.0,
    });

    world.add_resource(voxel_world);
    let registry = BlockRegistry::new().with_defaults();
    world.add_resource(registry);

    use std::time::Instant;
    use engine::world::block::BlockRegistry;

    while !world.res.fetch::<StopGameLoop>().0 {
        let frame_start = Instant::now();
        misc::clear(misc::ClearMode::Color(0.729411765, 0.907843137, 0.981568627, 1.0));
        misc::clear(misc::ClearMode::Depth(1.0));

        // Poll for new events, and fill the event buffer.
        glfw.poll_events();
        world.write_resource::<EventChannel<WindowEvent>>().iter_write(glfw::flush_messages(&events).map(|(_, event)| event).collect::<Vec<_>>());

        // Update systems and the world.
        world.maintain();
        world.res.insert(StopGameLoop(false));
        dispatcher.dispatch(&world.res);

        // Swap the backbuffer
        window.lock().unwrap().swap_buffers();
        let frame_end = Instant::now();
        let dt = frame_end - frame_start;
        *world.write_resource::<Dt>() = Dt(dt);
    }
}
