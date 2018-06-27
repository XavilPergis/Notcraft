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
#[macro_use] extern crate smallbitvec;
#[macro_use] extern crate lazy_static;

#[macro_use] pub mod gl_api;
pub mod engine;
pub mod util;
pub mod debug;
pub mod chunk_manager;
pub mod application;

use engine::block::Block;
use engine::ChunkPos;
use engine::chunk::Chunk;
use noise::NoiseFn;
use engine::terrain::ChunkGenerator;
use std::collections::HashSet;
use cgmath::{MetricSpace, Matrix4, Deg, Vector3, Vector2, Point3, Quaternion};
use glfw::{Action, Context, Key, Window, MouseButton, WindowEvent, WindowHint};
use gl_api::shader::*;
use application::Application;
use gl_api::misc;

pub struct Inputs {
    active_keys: HashSet<Key>,
}

impl Inputs {
    fn new() -> Self {
        Inputs { active_keys: HashSet::new() }
    }

    fn set_key(&mut self, key: Key, active: bool) {
        if active {
            self.active_keys.insert(key);
        } else {
            self.active_keys.remove(&key);
        }
    }

    fn is_down(&self, key: Key) -> bool {
        self.active_keys.contains(&key)
    }
}

use noise::{Fbm, SuperSimplex, MultiFractal};

pub struct NoiseGenerator {
    noise: Fbm,
    biome_noise: SuperSimplex,
}

fn smoothstep(x: f64, curve: f64, center: f64) -> f64 {
    let c = (2.0 / (1.0 - curve)) - 1.0;
    let f = |x: f64, n: f64| x.powf(c) - n.powf(c - 1.0);

    if x > center {
        f(x, center)
    } else {
        1.0 - f(1.0 - x, 1.0 - center)
    }
}

impl NoiseGenerator {
    pub fn new_default() -> Self {
        let noise = Fbm::default().set_frequency(0.125);
        // noise = noise.set_persistence(0.9);
        let biome_noise = SuperSimplex::new();
        NoiseGenerator { noise, biome_noise }
    }

    fn block_at(&self, pos: Point3<f64>) -> Block {
        let biome_noise = smoothstep((self.biome_noise.get([pos.x / 512.0, pos.z / 512.0]) + 1.0) / 2.0, 0.7, 0.5);

        let noise1 = (256.0 * self.noise.get([pos.x * 2.0, pos.z * 2.0]) + 1.0) / 2.0;
        let noise2 = (64.0 * self.noise.get([pos.x, pos.z]) + 1.0) / 2.0;
        let min = ::util::min(noise1, noise2);
        let max = ::util::max(noise1, noise2);

        let noise = (min + biome_noise * (max - min)) - pos.y;

        if noise > 4.0 { Block::Stone }
        else if noise > 1.0 { Block::Dirt }
        else if noise > 0.0 { Block::Grass }
        else { Block::Air }
    }

    fn pos_at_block(pos: ChunkPos, offset: Vector3<i32>) -> Point3<f64> {
        const SIZE: i32 = engine::chunk::CHUNK_SIZE as i32;
        let x = ((SIZE*pos.x) as f64 + offset.x as f64) / SIZE as f64;
        let y = (SIZE*pos.y) as f64 + offset.y as f64;
        let z = ((SIZE*pos.z) as f64 + offset.z as f64) / SIZE as f64;
        Point3::new(x, y, z)
    }
}

impl ChunkGenerator<Block> for NoiseGenerator {
    fn generate(&self, pos: Point3<i32>) -> Chunk<Block> {
        const SIZE: i32 = engine::chunk::CHUNK_SIZE as i32;
        const SIZE_USIZE: usize = engine::chunk::CHUNK_SIZE;
        let mut buffer = Vec::with_capacity(SIZE_USIZE*SIZE_USIZE*SIZE_USIZE);
        for by in 0..SIZE {
            for bz in 0..SIZE {
                for bx in 0..SIZE {
                    let pos = Self::pos_at_block(pos, Vector3::new(bx, by, bz));
                    // pos.y /= SIZE as f64;
                    buffer.push(self.block_at(pos));
                }
            }
        }

        Chunk::new(buffer)
    }
}

use glfw::SwapInterval;
use gl_api::shader::shader::ShaderError;

fn main() {
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

    let program = match simple_pipeline("resources/terrain.vs", "resources/terrain.fs") {
        Ok(prog) => prog,
        Err(msg) => match msg {
            PipelineError::Shader(ShaderError::Shader(msg)) => { println!("{}", msg); panic!() },
            _ => panic!("Other error")
        }
    };
    let debug_program = match simple_pipeline("resources/debug.vs", "resources/debug.fs") {
        Ok(prog) => prog,
        Err(msg) => match msg {
            PipelineError::Shader(ShaderError::Shader(msg)) => { println!("{}", msg); panic!() },
            _ => panic!("Other error")
        }
    };

    let mut application = Application::new(program, debug_program);
    let mut inputs = Inputs::new();

    unsafe {
        gl_call!(Disable(gl::MULTISAMPLE)).expect("glEnable failed");
        gl_call!(Enable(gl::DEPTH_TEST)).expect("glEnable failed");
        gl_call!(Enable(gl::CULL_FACE)).expect("glEnable failed");
        gl_call!(DepthFunc(gl::LESS)).expect("glDepthFunc failed");
        gl_call!(FrontFace(gl::CW)).expect("glFrontFace failed");
        gl_call!(CullFace(gl::BACK)).expect("glCullFace failed");
        gl_call!(Viewport(0, 0, 600, 600)).expect("glViewport failed");
    }

    application.set_viewport(600, 600);
    let mut prev_time = glfw.get_time();

    while !window.should_close() {
        misc::clear(misc::ClearMode::Color(0.729411765, 0.907843137, 0.981568627, 1.0));
        misc::clear(misc::ClearMode::Depth(1.0));
        
        glfw.poll_events();
        for (_, event) in glfw::flush_messages(&events) {
            if let WindowEvent::Key(key, _, action, _) = event {
                inputs.set_key(key, match action {
                    Action::Press | Action::Repeat => true,
                    _ => false,
                })
            }

            if application.handle_event(&mut window, event) {
                window.set_should_close(true);
            }
        }

        let now = glfw.get_time();
        let dt = now - prev_time;
        prev_time = now;

        application.handle_inputs(&inputs, dt);
        application.update(dt);
        application.draw(dt);

        window.swap_buffers();
    }
}
