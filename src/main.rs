#![feature(const_fn, trace_macros, nll, optin_builtin_traits, crate_visibility_modifier)]

extern crate gl;
extern crate glfw;
extern crate image;
extern crate cgmath;
extern crate noise;
extern crate smallvec;
extern crate collision;
extern crate rayon;
#[macro_use] extern crate smallbitvec;
#[macro_use] extern crate lazy_static;

#[macro_use] mod gl_api;
pub mod engine;
pub mod util;
pub mod debug;

use engine::ChunkPos;
use engine::WorldPos;
use engine::chunk::Chunk;
use noise::NoiseFn;
use engine::terrain::ChunkGenerator;
use engine::terrain::OctaveNoise;
use cgmath::Rotation;
use std::collections::HashSet;
use std::cmp::Ordering;
use cgmath::{MetricSpace, Matrix4, Deg, Vector3, Vector2, Point3, Quaternion};
use collision::Union;
use collision::algorithm::minkowski::GJK3;
use collision::primitive::Cuboid;
use collision::Discrete;
use collision::{Aabb3, Ray3, CollisionStrategy};
use glfw::{Action, Context, Key, Window, MouseButton, WindowEvent, WindowHint};
use glfw::CursorMode;
use engine::Voxel;
use engine::chunk_manager::ChunkManager;
use engine::camera::Rotation as CamRotation;
use engine::camera::Camera;
use gl_api::shader::program::LinkedProgram;
use gl_api::texture::Texture;
use gl_api::shader::*;
use gl_api::misc;

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
enum Block {
    Air,
    Stone,
    Dirt,
    Grass,
    Water,
}

use engine::Precomputed;
use engine::Side;

vertex! {
    vertex BlockFace {
        pos: Vector3<f32>,
        norm: Vector3<f32>,
        face: i32,
        uv: Vector2<f32>,
    }
}

impl Voxel for Block {
    type PerVertex = BlockFace;
    fn has_transparency(&self) -> bool { *self == Block::Air }
    fn vertex_data(&self, pre: Precomputed) -> BlockFace {
        BlockFace {
            pos: pre.pos,
            norm: pre.norm,
            face: pre.face,
            uv: (match *self {
                Block::Air => Vector2::new(0.0, 0.0),
                Block::Stone => Vector2::new(1.0, 0.0),
                Block::Dirt => Vector2::new(2.0, 0.0),
                Block::Grass => match pre.side {
                    Side::Top => Vector2::new(0.0, 0.0),
                    Side::Bottom => Vector2::new(2.0, 0.0),
                    _ => Vector2::new(0.0, 1.0),
                },
                Block::Water => Vector2::new(1.0, 0.0),
            } + pre.face_offset) / 4.0
        }
    }
}

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

pub struct NoiseGenerator<N> {
    noise: OctaveNoise<N>,
}

impl<N> NoiseGenerator<N> {
    pub fn new_default(noise: N) -> Self {
        NoiseGenerator {
            noise: OctaveNoise {
                lacunarity: 2.0,
                persistance: 0.4,
                height: 60.0,
                octaves: 4,
                noise,
            },
        }
    }

    fn block_at(&self, pos: Point3<f64>) -> bool where N: NoiseFn<[f64; 3]> {
        self.noise.get([pos.x, pos.y, pos.z]) - pos.y >= pos.y
    }

    fn pos_at_block(pos: ChunkPos, offset: Vector3<i32>) -> Point3<f64> {
        const SIZE: i32 = engine::chunk::CHUNK_SIZE as i32;
        let x = ((SIZE*pos.x) as f64 + offset.x as f64) / SIZE as f64;
        let y = (SIZE*pos.y) as f64 + offset.y as f64;
        let z = ((SIZE*pos.z) as f64 + offset.z as f64) / SIZE as f64;
        Point3::new(x, y, z)
    }
}

impl<N> ChunkGenerator<Block> for NoiseGenerator<N> where N: NoiseFn<[f64; 3]> {
    fn generate(&self, pos: Point3<i32>) -> Chunk<Block> {
        const SIZE: i32 = engine::chunk::CHUNK_SIZE as i32;
        const SIZE_USIZE: usize = engine::chunk::CHUNK_SIZE;
        let mut buffer = Vec::with_capacity(SIZE_USIZE*SIZE_USIZE*SIZE_USIZE);
        for by in 0..SIZE {
            for bz in 0..SIZE {
                for bx in 0..SIZE {
                    let mut pos = Self::pos_at_block(pos, Vector3::new(bx, by, bz));
                    pos.y /= SIZE as f64;
                    buffer.push(if pos.y <= 0.0 { Block::Stone } else { Block::Air });
                }
            }
        }

        let mut chunk = Chunk::new(buffer);
        // for by in 0..SIZE_USIZE {
        //     for bz in 0..SIZE_USIZE {
        //         for bx in 0..SIZE_USIZE {
        //             if Block::Air == chunk[(bx, by, bz)] {
        //                 for oy in 0..4 {
        //                     if oy <= by {
        //                         if chunk[(bx, by - oy, bz)] != Block::Air {
        //                             chunk[(bx, by - oy, bz)] = Block::Dirt;
        //                         } 
        //                     }
        //                 }
        //             }
        //         }
        //     }
        // }
        chunk
    }
}

#[derive(Debug)]
struct Config {
    acceleration: f32,
    fast_acceleration: f32,
    max_fall_speed: f32,
    jump_velocity: f32,
    gravity: f32,
}

struct Application {
    wireframe: bool,
    mouse_capture: bool,
    debug_frames: bool,
    noclip: bool,
    jumping: bool,

    player_pos: Point3<f32>,
    velocity: Vector3<f32>,
    time: f32,
    frames: i32,
    previous_cursor_x: f32,
    previous_cursor_y: f32,
    selection_start: Option<Point3<i32>>,
    selected_block: Block,

    textures: Texture,
    cfg: Config,
    camera: Camera,
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
        pipeline.set_uniform("u_LightAmbient", &Vector3::<f32>::new(0.4, 0.5, 0.6));

        // use engine::terrain::NoiseGenerator;

        let chunk_manager = ChunkManager::new(NoiseGenerator::new_default(
            noise::OpenSimplex::default(),
        ));

        use gl_api::texture::*;

        let textures = Texture::new();
        textures.source_from_image("resources/textures.png").unwrap();
        textures.mag_filter(MagnificationFilter::Nearest);
        textures.min_filter(MinificationFilter::Nearest);
        textures.texture_wrap_behavior(TextureAxis::S, WrapMode::Repeat);
        textures.texture_wrap_behavior(TextureAxis::T, WrapMode::Repeat);
        pipeline.set_uniform("u_TextureMap", &textures);

        Application {
            cfg: Config {
                acceleration: 0.15,
                fast_acceleration: 0.2,
                max_fall_speed: 2.0,
                jump_velocity: 6.5,
                gravity: 16.0,
            },
            wireframe: false,
            debug_frames: false,
            mouse_capture: false,
            noclip: false,
            jumping: false,
            velocity: Vector3::new(0.0, 0.0, 0.0),
            player_pos: Point3::new(0.0, 0.0, 0.0),
            selection_start: None,
            previous_cursor_x: 0.0,
            previous_cursor_y: 0.0,
            selected_block: Block::Grass,
            frames: 0,
            time: 0.0,
            textures,
            camera: Camera::default(),
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

        self.camera.rotate(CamRotation::AboutY(Deg(-dx as f32/3.0)));
        self.camera.rotate(CamRotation::AboutX(Deg(-dy as f32/3.0)));
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
            WindowEvent::MouseButton(MouseButton::Button1, Action::Press, _) => self.start_selection(),
            WindowEvent::MouseButton(MouseButton::Button1, Action::Release, _) => self.end_selection(),
            WindowEvent::MouseButton(MouseButton::Button2, Action::Press, _) => self.place_block(),

            WindowEvent::Key(Key::Escape, _, Action::Press, _) => return true,
            WindowEvent::Key(Key::F1, _, Action::Press, _) => self.toggle_debug_frames(),
            WindowEvent::Key(Key::F2, _, Action::Press, _) => self.toggle_wireframe(),
            WindowEvent::Key(Key::F3, _, Action::Press, _) => self.toggle_mouse_capture(window),
            WindowEvent::Key(Key::F4, _, Action::Press, _) => self.noclip = !self.noclip,
            
            WindowEvent::Size(width, height) => self.set_viewport(width, height),
            _ => {}
        }
        false
    }

    fn selection_bounds(&self) -> Option<Aabb3<i32>> {
        self.get_look_pos().and_then(|look| self.selection_start.map(|start| {
            Aabb3::new(start, look)
                .union(&Aabb3::new(start, start + Vector3::new(1, 1, 1)))
                .union(&Aabb3::new(look, look + Vector3::new(1, 1, 1)))
        }))
    }

    fn start_selection(&mut self) {
        self.selection_start = self.get_look_pos();
    }

    fn end_selection(&mut self) {
        let end = self.get_look_pos();
        if let (Some(start), Some(end)) = (self.selection_start, end) {
            if start == end {
                self.chunk_manager.set_voxel(start, Block::Air);
            } else {
                self.chunk_manager.set_voxel_range(self.selection_bounds().unwrap(), Block::Air);
            }
        }
        self.selection_start = None;
    }

    fn handle_inputs(&mut self, inputs: &Inputs, _dt: f64) {
        if inputs.is_down(Key::Right) { self.camera.rotate(CamRotation::AboutY(Deg(1.0))); }
        if inputs.is_down(Key::Left) { self.camera.rotate(CamRotation::AboutY(-Deg(1.0))); }
        if inputs.is_down(Key::Up) { self.camera.rotate(CamRotation::AboutX(-Deg(1.0))); }
        if inputs.is_down(Key::Down) { self.camera.rotate(CamRotation::AboutX(Deg(1.0))); }
        
        let accel = if inputs.is_down(Key::LeftControl) {
            self.cfg.fast_acceleration
        } else {
            self.cfg.acceleration
        };

        if inputs.is_down(Key::W) {
            let look_vec = accel * self.camera.get_spin_vecs().0;
            self.velocity.x = self.velocity.x - look_vec.x;
            self.velocity.z = self.velocity.z - look_vec.z;
        }

        if inputs.is_down(Key::S) {
            let look_vec = accel * self.camera.get_spin_vecs().0;
            self.velocity.x = self.velocity.x + look_vec.x;
            self.velocity.z = self.velocity.z + look_vec.z;
        }

        if inputs.is_down(Key::A) {
            let look_vec = accel * self.camera.get_spin_vecs().1;
            self.velocity.x = self.velocity.x - look_vec.x;
            self.velocity.z = self.velocity.z - look_vec.z;
        }

        if inputs.is_down(Key::D) {
            let look_vec = accel * self.camera.get_spin_vecs().1;
            self.velocity.x = self.velocity.x + look_vec.x;
            self.velocity.z = self.velocity.z + look_vec.z;
        }

        if inputs.is_down(Key::Space) {
            if !self.jumping && !self.noclip {
                self.jumping = true;
                self.velocity.y = self.cfg.jump_velocity;
            }

            if self.noclip {
                self.velocity.y += accel;
            }
        }

        if inputs.is_down(Key::LeftShift) {
            if self.noclip {
                self.velocity.y -= accel;
            }
        }
    }

    fn apply_motion(&mut self, dt: f64) {
        let substeps = 3;
        let timestep = dt as f32 / (substeps as f32);
        for _ in 0..substeps {
            let world = self.chunk_manager.world();
            let feet = Point3::new(
                self.player_pos.x.floor() as i32,
                self.player_pos.y.floor() as i32,
                self.player_pos.z.floor() as i32);
            
            // Don't apply any motion if the player is unloaded chunks.
            if world.get_voxel(feet).is_none() { return; }

            self.player_pos += self.velocity * timestep;
            let gjk = GJK3::new();
            let around = world.around_voxel(feet, 3, |pos, voxel| if voxel.has_transparency() { None } else { Some(pos) });

            const PLAYER_WIDTH: f32 = 0.45;
            const PLAYER_HEIGHT: f32 = 1.8;

            for block_pos in around {
                self.frame_at_voxel(block_pos.cast().unwrap(), Vector3::new(0.0, 1.0, 1.0), 0.003, false);
                let block_tfm = Matrix4::from_translation(
                    ::util::to_vector(block_pos.cast().unwrap() + Vector3::new(0.5, 0.5, 0.5)),
                );

                let player_tfm = Matrix4::from_translation(
                    ::util::to_vector(self.player_pos + Vector3::new(0.0, PLAYER_HEIGHT / 2.0, 0.0)),
                );

                // NOTE: non-transparent blocks were filtered out
                if let Some(contact) = gjk.intersection(
                    &CollisionStrategy::FullResolution,
                    &Cuboid::new(PLAYER_WIDTH, PLAYER_HEIGHT, PLAYER_WIDTH),
                    &player_tfm,
                    &Cuboid::new(1.0, 1.0, 1.0),
                    &block_tfm
                ) {
                    let resolution = -1.0 * contact.penetration_depth * contact.normal;

                    // We check two cuboids here, so normals should be axis-aligned. If any of
                    // the components are not zero, that means we've had a collision on that
                    // face and should cancel velocity in that direction. Alternatively, you
                    // could multiply the component by something like -0.8 and have a lot of fun!
                    if resolution.x.abs() > 0.0 { self.velocity.x = 0.0; }
                    if resolution.y.abs() > 0.0 { self.velocity.y = 0.0; self.jumping = false; }
                    if resolution.z.abs() > 0.0 { self.velocity.z = 0.0; }
                    self.player_pos += resolution;
                }
            }
        }
    }
    
    fn place_block(&mut self) {
        let side = self.get_look_face();
        let pos = self.get_look_pos();

        if let (Some(side), Some(pos)) = (side, pos) {
            self.chunk_manager.set_voxel(pos + side.offset(), self.selected_block);
        }
    }

    fn update(&mut self, dt: f64) {
        let view = self.camera.transform_matrix();
        self.pipeline.set_uniform("u_Time", &self.time);
        self.pipeline.set_uniform("u_CameraPosition", &::util::to_vector(self.camera.position));
        self.pipeline.set_uniform("u_View", &view);
        self.debug_pipeline.set_uniform("view", &view);

        const FRICTION: f32 = 0.02;
        const GLIDE_FRICTION: f32 = 0.1;

        if self.noclip {
            self.velocity *= GLIDE_FRICTION.powf(dt as f32);
            self.player_pos += self.velocity * 2.0 * dt as f32;
        } else {
            self.velocity.x *= FRICTION.powf(dt as f32);
            self.velocity.z *= FRICTION.powf(dt as f32);
            self.velocity.y -= ::util::clamp(self.cfg.gravity * dt as f32, -self.cfg.max_fall_speed, ::std::f32::INFINITY);
            self.apply_motion(dt);
        }
        self.camera.position = self.player_pos + Vector3::new(0.0, 1.8 - 0.45, 0.0);

        self.chunk_manager.update_player_position(self.player_pos);
        self.chunk_manager.tick();
        self.time += 0.007;
    }

    fn get_look_face(&self) -> Option<Side> {
        use cgmath::Point3;

        // Thickness of the collision boxes on each face
        let t = 0.1;
        let look_vec = -self.camera.get_look_vec();

        self.get_look_pos().and_then(|look_pos| {
            let ray = Ray3::new(self.camera.position, look_vec);
            let look_pos = look_pos.cast().unwrap();
            let cam_pos = ::util::to_vector(self.camera.position);
            let (l, h) = (look_pos, look_pos + Vector3::new(1.0, 1.0, 1.0));

            // These are the bounding boxes of each face. They are each face, stretched out `t` units
            // in their corresponding direction
            let bb_left   = Aabb3::new(Point3::new(l.x, l.y, l.z), Point3::new(l.x - t, h.y, h.z));
            let bb_right  = Aabb3::new(Point3::new(h.x, l.y, l.z), Point3::new(h.x + t, h.y, h.z));
            let bb_bottom = Aabb3::new(Point3::new(l.x, l.y, l.z), Point3::new(h.x, l.y - t, h.z));
            let bb_top    = Aabb3::new(Point3::new(l.x, h.y, l.z), Point3::new(h.x, h.y + t, h.z));
            let bb_back   = Aabb3::new(Point3::new(l.x, l.y, l.z), Point3::new(h.x, h.y, l.z - t));
            let bb_front  = Aabb3::new(Point3::new(l.x, l.y, h.z), Point3::new(h.x, h.y, h.z + t));

            // Center positions of each face. We use these for sorting which faces are closest to the
            // camera so we don't accidentally select backfaces
            let pos_left   = Vector3::new(l.x, (l.y + h.y) * 0.5, (l.z + h.z) * 0.5);
            let pos_right  = Vector3::new(h.x, (l.y + h.y) * 0.5, (l.z + h.z) * 0.5);
            let pos_bottom = Vector3::new((l.x + h.x) * 0.5, l.y, (l.z + h.z) * 0.5);
            let pos_top    = Vector3::new((l.x + h.x) * 0.5, h.y, (l.z + h.z) * 0.5);
            let pos_back   = Vector3::new((l.x + h.x) * 0.5, (l.y + h.y) * 0.5, l.z);
            let pos_front  = Vector3::new((l.x + h.x) * 0.5, (l.y + h.y) * 0.5, h.z);

            let items = &mut [
                (Side::Left, pos_left, bb_left),
                (Side::Right, pos_right, bb_right),
                (Side::Bottom, pos_bottom, bb_bottom),
                (Side::Top, pos_top, bb_top),
                (Side::Back, pos_back, bb_back),
                (Side::Front, pos_front, bb_front),
            ];

            // Sort the list by distance to the camera
            items.sort_by(|&(_, a, _), &(_, b, _)| a.distance2(cam_pos)
                .partial_cmp(&b.distance2(cam_pos))
                .unwrap_or(Ordering::Equal));
            
            // Now get the side closest to the camera that intersects the ray extending from the
            // player's eyes
            items.iter().filter(|&&(_, _, aabb)| ray.intersects(&aabb)).map(|&(side, _, _)| side).next()
        })
    }

    fn get_look_pos(&self) -> Option<Point3<i32>> {
        use cgmath::Rotation3;
        const LOOK_REACH: usize = 50;

        fn check_collision(ray: Ray3<f32>, pos: Point3<f32>) -> bool {
            // convert the floating position to a voxel-aligned position
            let voxel_pos = Point3::new(pos.x.floor(), pos.y.floor(), pos.z.floor());
            // Get the bounding box of the entire cube
            let bbox = Aabb3::new(voxel_pos, voxel_pos + Vector3::new(1.0, 1.0, 1.0));
            ray.intersects(&bbox)
        }

        fn point_floor(point: Point3<f32>) -> Point3<f32> {
            Point3::new(point.x.floor(), point.y.floor(), point.z.floor())
        }

        let look_vec = -self.camera.get_look_vec();
        // Ray extending from the player's eye in their look direction forever.
        let ray = Ray3::new(self.camera.position, look_vec);

        let samples = 2 * LOOK_REACH;
        let rotate_samples = 8;
        let rotate_sample_len = 0.5f32;
        let up = rotate_sample_len * Vector3::unit_y();

        let is_solid = |pos: Point3<f32>| 
            self.chunk_manager.world()
                .get_voxel(point_floor(pos).cast().unwrap())
                .map(|voxel| !voxel.has_transparency())
                .unwrap_or(false);

        for k in 0..samples {
            // the length of the current reach vector. `k/samples` ranges from 0 to 1, so `length`
            // ranges from 0 to LOOK_REACH
            let length = LOOK_REACH as f32 * k as f32 / samples as f32;
            // the coordinates of the current space we're checking. it's the player's eye position,
            // offset by the current portion of the reach vector
            let pos = self.camera.position + look_vec * length;
            if is_solid(pos) && check_collision(ray, pos) {
                return Some(point_floor(pos).cast().unwrap());
            }

            for l in 0..rotate_samples {
                let rot_frac = l as f32 / rotate_samples as f32;
                let rotation = Quaternion::from_axis_angle(look_vec, Deg(rot_frac * 360.0));
                let pos = pos + rotation.rotate_vector(up);
                if is_solid(pos) && check_collision(ray, pos) {
                    return Some(point_floor(pos).cast().unwrap());
                }
            }
        }

        None
    }

    fn draw(&mut self, _dt: f64) {
        let look_pos = self.get_look_pos();
        let side = self.get_look_face();
        // Draw frame around the block we're looking at
        if let Some(look) = look_pos {
            self.frame_at_voxel(Point3::new(look.x as f32, look.y as f32, look.z as f32), Vector3::new(0.0, 0.6, 0.8), 0.01, true);
            if let Some(side) = side {
                self.frame_at_voxel(Point3::new(look.x as f32, look.y as f32, look.z as f32) + side.offset(), Vector3::new(0.0, 0.8, 0.6), 0.008, false);
            }
        }

        if let Some(aabb) = self.selection_bounds() {
            self.draw_frame(Aabb3::new(aabb.min.cast().unwrap(), aabb.max.cast().unwrap()), Vector3::new(1.0, 0.5, 0.0), 0.02, true);
        }

        self.draw_frame(Aabb3::new(
            ::util::to_point(::util::to_vector(self.camera.position) - Vector3::new(9.0, 9.0, 9.0)),
            ::util::to_point(::util::to_vector(self.camera.position) + Vector3::new(9.0, 9.0, 9.0)),
        ), Vector3::new(0.0, 1.0, 0.0), 0.02, false);

        self.chunk_manager.draw(&mut self.pipeline).expect("Drawing chunks failed");
        self.frames += 1;
    }

    fn draw_frame(&mut self, aabb: Aabb3<f32>, color: Vector3<f32>, thickness: f32, force: bool) {
        if self.debug_frames || force {
            ::debug::draw_frame(&mut self.debug_pipeline, aabb, color, thickness);
        }
    }

    fn frame_at_voxel(&mut self, pos: Point3<f32>, color: Vector3<f32>, thickness: f32, force: bool) {
        self.draw_frame(Aabb3::new(
            pos, pos + Vector3::new(1.0, 1.0, 1.0),
        ), color, thickness, force);
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
    glfw.set_swap_interval(SwapInterval::Adaptive);

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

        let now = glfw.get_time();
        let dt = now - prev_time;
        prev_time = now;

        application.handle_inputs(&inputs, dt);
        application.update(dt);
        application.draw(dt);

        window.swap_buffers();
    }
}
