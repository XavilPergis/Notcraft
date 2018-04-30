#![feature(const_fn, trace_macros, nll, match_beginning_vert, optin_builtin_traits)]

extern crate gl;
extern crate glfw;
extern crate image;
extern crate cgmath;
extern crate noise;
extern crate smallvec;
extern crate collision;
extern crate rayon;
// extern crate imgui;
#[macro_use] extern crate error_chain;
#[macro_use] extern crate lazy_static;

#[macro_use] mod gl_api;
pub mod engine;
pub mod util;

use cgmath::MetricSpace;
use collision::Discrete;
use collision::Continuous;
use collision::{Aabb3, Ray3};
use cgmath::Point3;
use gl_api::shader::program::LinkedProgram;
use std::collections::HashSet;
use glfw::{Action, Context, Key, Window, MouseButton, WindowEvent, WindowHint};
use cgmath::{Deg, InnerSpace, Matrix4, Vector3};
use noise::{Perlin, NoiseFn};
use engine::chunk_manager::{ChunkGenerator, ChunkManager};
use engine::chunk::Chunk;
use engine::mesh::{IndexingType, Mesh};
use engine::camera::Rotation;
use gl_api::layout::InternalLayout;
use gl_api::shader::*;
use gl_api::misc;

struct Entity<V: InternalLayout, I: IndexingType> {
    mesh: Mesh<V, I>,
    translation: Vector3<f32>,
    rotation: Vector3<Deg<f32>>,
    scale: f32
}

impl<V: InternalLayout, I: IndexingType> Entity<V, I> {
    fn transform_matrix(&self) -> Matrix4<f32> {
        let rotation_y = Matrix4::from_axis_angle(Vector3::unit_y(), self.rotation.y);
        let rotation_z = Matrix4::from_axis_angle(Vector3::unit_z(), self.rotation.z);
        let rotation_x = Matrix4::from_axis_angle(Vector3::unit_x(), self.rotation.x);
        
        Matrix4::from_translation(self.translation)
            * rotation_x * rotation_y * rotation_z
            * Matrix4::from_scale(self.scale)
    }

    pub fn draw_with(&self, pipeline: &mut LinkedProgram) {
        pipeline.set_uniform("u_Transform", &self.transform_matrix());
        self.mesh.draw_with(pipeline).unwrap();
    }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
enum Block {
    Air,
    Stone,
    Dirt,
    Grass,
    Water,
}

impl engine::chunk::Voxel for Block {
    fn has_transparency(&self) -> bool { *self == Block::Air }
    fn color(&self) -> Vector3<f32> {
        match *self {
            Block::Air => Vector3::new(0.0, 0.0, 0.0),
            Block::Stone => Vector3::new(0.545098039, 0.552941176, 0.478431373),
            Block::Dirt => Vector3::new(0.250980392, 0.160784314, 0.0196078431),
            Block::Grass => Vector3::new(0.376, 0.502, 0.220),
            Block::Water => Vector3::new(0.1, 0.2, 0.9),
        }
    }
}

use glfw::CursorMode;
use engine::camera::Camera;

struct Inputs {
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

struct NoiseGenerator {
    noise: noise::SuperSimplex,
    lacunarity: f64,
    persistance: f64,
    scale: f64,
    octaves: usize,
}

impl ChunkGenerator<Block> for NoiseGenerator {
    fn generate(&self, pos: Vector3<i32>) -> Chunk<Block> {
        let mut buffer = Vec::with_capacity(50*50*50);
        for z in 0..50 {
            for y in 0..50 {
                for x in 0..50 {
                    let x = ((50*pos.x) as f64 + x as f64) / 50.0;
                    let y = (pos.y*50) as f64 + y as f64;
                    let z = ((50*pos.z) as f64 + z as f64) / 50.0;
                    let mut total = 0.0;
                    
                    for octave in 0..self.octaves-1 {
                        let x = x * self.lacunarity.powf(octave as f64);
                        let z = z * self.lacunarity.powf(octave as f64);
                        total += self.scale * self.persistance.powf(octave as f64) * self.noise.get([x, z]);
                    }

                    buffer.push(
                        if total-3.0 >= y { Block::Stone }
                        else if total-1.0 >= y { Block::Dirt }
                        else if total >= y { Block::Grass }
                        else if y <= -35.0 { Block::Water }
                        else { Block::Air }
                    );
                }
            }
        }

        Chunk::new(pos.x, pos.y, pos.z, buffer)
    }
}


struct Application {
    time: f32,
    wireframe: bool,
    mouse_capture: bool,
    debug_frames: bool,
    camera: Camera,
    previous_cursor_x: f32,
    previous_cursor_y: f32,
    frames: i32,

    speed_x: f32,
    speed_y: f32,
    speed_z: f32,
    max_speed: f32,
    cam_acceleration: f32,
    looking_at: Option<Vector3<f32>>,

    pipeline: LinkedProgram,
    debug_pipeline: LinkedProgram,
    chunk_manager: ChunkManager<Block>,
}

impl Application {
    fn new(mut pipeline: LinkedProgram, mut debug_pipeline: LinkedProgram) -> Self {
        unsafe { gl_call!(Viewport(0, 0, 600, 600)).expect("glViewport failed"); }
        let projection = cgmath::perspective(Deg(70.0), 600.0 / 600.0, 0.1, 1000.0f32);
        pipeline.set_uniform("u_Projection", &projection);
        debug_pipeline.set_uniform("projection", &projection);

        let mut poses = Vec::new();
        let mut colors = Vec::new();
        let mut attenuations = Vec::new();

        for i in 0..3 {
            poses.push(Vector3::new(i as f32 * 10.0, 0.5, 0.5));
            colors.push(Vector3::new(i as f32/3.0, 0.0, 0.0));
            attenuations.push(0.5f32);
        }

        pipeline.set_uniform("u_Light", &poses.as_slice());
        pipeline.set_uniform("u_LightColor", &colors.as_slice());
        pipeline.set_uniform("u_LightAttenuation", &attenuations.as_slice());
        pipeline.set_uniform("u_LightAmbient", &Vector3::<f32>::new(0.2, 0.25, 0.3));

        let chunk_manager = ChunkManager::new(NoiseGenerator {
            noise: noise::SuperSimplex::new(),
            lacunarity: 2.0,
            persistance: 0.9,
            scale: 20.0,
            octaves: 4,
        });

        Application {
            speed_x: 0.0,
            speed_y: 0.0,
            speed_z: 0.0,
            max_speed: 1.5,
            cam_acceleration: 0.03,
            frames: 0,
            looking_at: None,
            time: 0.0,
            wireframe: false,
            debug_frames: false,
            mouse_capture: false,
            camera: Camera::default(),
            previous_cursor_x: 0.0,
            previous_cursor_y: 0.0,
            pipeline,
            debug_pipeline,
            chunk_manager,
        }
    }

    fn toggle_wireframe(&mut self) {
        self.wireframe = !self.wireframe;
        misc::polygon_mode(if self.wireframe { misc::PolygonMode::Line } else { misc::PolygonMode::Fill });
    }

    fn toggle_mouse_capture(&mut self, window: &mut Window) {
        self.mouse_capture = !self.mouse_capture;
        window.set_cursor_mode(if self.mouse_capture { CursorMode::Disabled } else { CursorMode::Normal });
    }

    fn toggle_debug_frames(&mut self) {
        self.debug_frames = !self.debug_frames;
    }

    fn update_camera_rotation(&mut self, x: f32, y: f32) {
        let dx = self.previous_cursor_x - x;
        let dy = self.previous_cursor_y - y;
        self.previous_cursor_x = x;
        self.previous_cursor_y = y;

        self.camera.rotate(Rotation::AboutY(Deg(-dx as f32/3.0)));
        self.camera.rotate(Rotation::AboutX(Deg(-dy as f32/3.0)));
    }

    fn set_viewport(&mut self, width: i32, height: i32) {
        unsafe { gl_call!(Viewport(0, 0, width, height)).expect("glViewport failed"); }

        let projection = cgmath::perspective(Deg(70.0), width as f32 / height as f32, 0.1, 1000.0);
        self.pipeline.set_uniform("u_Projection", &projection);
        self.debug_pipeline.set_uniform("projection", &projection);
    }

    fn handle_event(&mut self, window: &mut Window, event: WindowEvent) -> bool {
        match event {
            WindowEvent::CursorPos(x, y) => self.update_camera_rotation(x as f32, y as f32),
            WindowEvent::MouseButton(MouseButton::Button1, Action::Press, _) => self.destroy_looking_at(),
            // WindowEvent::MouseButton(MouseButton::Button2, Action::Press, _) => self.place_looking_at(),

            WindowEvent::Key(Key::Escape, _, Action::Press, _) => return true,
            WindowEvent::Key(Key::F1, _, Action::Press, _) => self.toggle_debug_frames(),
            WindowEvent::Key(Key::F2, _, Action::Press, _) => self.toggle_wireframe(),
            WindowEvent::Key(Key::F3, _, Action::Press, _) => self.toggle_mouse_capture(window),
            
            WindowEvent::Size(width, height) => self.set_viewport(width, height),
            _ => {}
        }
        false
    }

    fn handle_inputs(&mut self, inputs: &Inputs) {
        // println!("camera={:?}", self.camera);
        if inputs.is_down(Key::Right) { self.camera.rotate(Rotation::AboutY(Deg(1.0))); }
        if inputs.is_down(Key::Left) { self.camera.rotate(Rotation::AboutY(-Deg(1.0))); }
        if inputs.is_down(Key::Up) { self.camera.rotate(Rotation::AboutX(-Deg(1.0))); }
        if inputs.is_down(Key::Down) { self.camera.rotate(Rotation::AboutX(Deg(1.0))); }

        if inputs.is_down(Key::W) {
            let look_vec = self.camera.get_horizontal_look_vec();
            self.speed_x = ::util::clamp(self.speed_x + self.cam_acceleration * look_vec.x, -self.max_speed, self.max_speed);
            self.speed_z = ::util::clamp(self.speed_z + self.cam_acceleration * look_vec.z, -self.max_speed, self.max_speed);
        }

        if inputs.is_down(Key::S) {
            let look_vec = self.camera.get_horizontal_look_vec();
            self.speed_x = ::util::clamp(self.speed_x - self.cam_acceleration * look_vec.x, -self.max_speed, self.max_speed);
            self.speed_z = ::util::clamp(self.speed_z - self.cam_acceleration * look_vec.z, -self.max_speed, self.max_speed);
        }

        if inputs.is_down(Key::Space) {
            // No need to multiply the cam accel by anything because we only ever travel
            // straight up and down the Y axis.
            self.speed_y = ::util::clamp(self.speed_y - self.cam_acceleration, -self.max_speed, self.max_speed);
        }

        if inputs.is_down(Key::LeftShift) {
            self.speed_y = ::util::clamp(self.speed_y + self.cam_acceleration, -self.max_speed, self.max_speed);
        }

        if inputs.is_down(Key::LeftControl) {
            self.cam_acceleration = 0.1;
        } else {
            self.cam_acceleration = 0.03;
        }
    }

    fn destroy_looking_at(&mut self) {
        if let Some(pos) = self.get_look_pos() {
            self.chunk_manager.set_voxel(pos, Block::Air);
        }
    }

    fn update(&mut self) {
        let view = self.camera.transform_matrix();
        self.pipeline.set_uniform("u_Time", &self.time);
        self.pipeline.set_uniform("u_CameraPosition", &-self.camera.position);
        self.pipeline.set_uniform("u_View", &view);
        self.debug_pipeline.set_uniform("view", &view);

        let translation = Vector3::new(self.speed_x, self.speed_y, self.speed_z);
        if translation.magnitude() != 0.0 {
            // normalize fails whenever the magnitude of the vector is 0
            let magnitude = (self.speed_x*self.speed_x + self.speed_y*self.speed_y + self.speed_z*self.speed_z).sqrt();
            self.camera.translate(magnitude * translation.normalize());
        }

        self.speed_x *= 0.95;
        self.speed_y *= 0.95;
        self.speed_z *= 0.95;

        let cam_pos = self.camera.position;
        let int_pos = -Vector3::new((cam_pos.x / 50.0).ceil() as i32, (cam_pos.y / 50.0).ceil() as i32, (cam_pos.z / 50.0).ceil() as i32);
        self.chunk_manager.update_player_position(int_pos);
        self.chunk_manager.tick();
        self.time += 0.007;
    }

    fn get_look_pos(&self) -> Option<Vector3<i32>> {
        use std::cmp::Ordering;
        let cam_pos = -self.camera.position;
        let cam_pos_int = Vector3::new(cam_pos.x as i32, cam_pos.y as i32, cam_pos.z as i32);
        let look_vec = -self.camera.get_horizontal_look_vec();
        let ray = Ray3::new(::util::to_point(-self.camera.position), look_vec);

        let colliders = self.chunk_manager.colliders_around_point(cam_pos_int, 9);
        let mut colliders: Vec<_> = colliders.iter()
            .filter(|aabb| ray.intersects(&aabb)).collect();
        
        colliders.sort_by(|a, b| a.min.distance2(::util::to_point(cam_pos))
            .partial_cmp(&b.min.distance2(::util::to_point(cam_pos)))
            .unwrap_or(Ordering::Equal));
        
        colliders.get(0).map(|aabb| {
            let fv = ::util::to_vector(aabb.min);
            Vector3::new(fv.x as i32, fv.y as i32, fv.z as i32)
        })
    }

    fn draw(&mut self) {
        if let Some(look) = self.get_look_pos() {
            self.frame_at_voxel(Vector3::new(look.x as f32, look.y as f32, look.z as f32), 0.02);
        }

        if self.debug_frames {
            ::util::draw_frame(Aabb3::new(
                ::util::to_point(-(self.camera.position - Vector3::new(9.0, 9.0, 9.0))),
                ::util::to_point(-(self.camera.position + Vector3::new(9.0, 9.0, 9.0))),
            ), &mut self.debug_pipeline, 0.02);
        }

        self.chunk_manager.draw(&mut self.pipeline).expect("Drawing chunks failed");
        self.frames += 1;
    }

    fn frame_at_voxel(&mut self, pos: Vector3<f32>, thickness: f32) {
        if self.debug_frames {
            ::util::draw_frame(Aabb3::new(
                ::util::to_point(pos),
                ::util::to_point(pos + Vector3::new(1.0, 1.0, 1.0)),
            ), &mut self.debug_pipeline, thickness);
        }
    }
}

fn main() {
    let mut glfw = glfw::init(glfw::FAIL_ON_ERRORS).unwrap();
    println!("GLFW init");
    
    glfw.window_hint(WindowHint::ContextVersion(4, 5));
    glfw.window_hint(WindowHint::DepthBits(Some(24)));

    let (mut window, events) = glfw.create_window(600, 600, "Not Minecraft", glfw::WindowMode::Windowed)
        .expect("Failed to create GLFW window.");
    println!("Window created");

    window.set_all_polling(true);
    window.make_current();

    // Load OpenGL function pointers.
    // good *god* this function takes a long time fo compile
    gl::load_with(|symbol| window.get_proc_address(symbol) as *const _);

    let program = simple_pipeline("resources/terrain.vs", "resources/terrain.fs")
        .expect("Pipeline creation failure");
    let mut debug_program = simple_pipeline("resources/debug.vs", "resources/debug.fs")
        .expect("Pipeline creation failure");

    let mut application = Application::new(program, debug_program);
    let mut inputs = Inputs::new();

    unsafe {
        gl_call!(Enable(gl::DEPTH_TEST)).expect("glEnable failed");
        gl_call!(DepthFunc(gl::LESS)).expect("glDepthFunc failed");
        gl_call!(Enable(gl::CULL_FACE)).expect("glEnable failed");
        gl_call!(FrontFace(gl::CW)).expect("glFrontFace failed");
        gl_call!(CullFace(gl::BACK)).expect("glCullFace failed");
        gl_call!(Viewport(0, 0, 600, 600)).expect("glViewport failed");
    }

    application.set_viewport(600, 600);

    while !window.should_close() {
        misc::clear(misc::ClearMode::Color(0.529411765, 0.807843137, 0.921568627, 1.0));
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

        application.handle_inputs(&inputs);
        application.update();
        application.draw();

        window.swap_buffers();
    }
}
