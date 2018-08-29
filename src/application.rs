use engine::chunk::CHUNK_SIZE;
use glfw::Glfw;
use specs::shred::PanicHandler;
use std::sync::Mutex;
use std::sync::Arc;
use specs::prelude::*;
use smallvec::SmallVec;
use cgmath;
// use ::{NoiseGenerator, Inputs};
use engine::block::Block;
use std::cmp::Ordering;
use cgmath::{Matrix4, MetricSpace, Deg, Vector3, Point3};
use collision::algorithm::minkowski::GJK3;
use collision::primitive::Cuboid;
use collision::{Aabb3, CollisionStrategy, Discrete, Ray3, Union};
use glfw::{Action, Key, Window, MouseButton, WindowEvent, Context};
use glfw::CursorMode;
use engine::{Side, Voxel};
// use chunk_manager::ChunkManager;
use engine::camera::Rotation as CamRotation;
use engine::camera::Camera;
use engine::world::World as RustelWorld;
use gl_api::shader::program::LinkedProgram;
use gl_api::texture::Texture2D;
use gl_api::misc;

struct Position(Point3<f32>);
struct ChunkLoader(usize);

impl Component for Position {
    type Storage = DenseVecStorage<Self>;
}

impl Component for ChunkLoader {
    type Storage = FlaggedStorage<Self, HashMapStorage<Self>>;
}

struct ChunkManager {
    new_loaders: ReaderId<InsertedFlag>,
    old_loaders: ReaderId<RemovedFlag>,

    new_set: BitSet,
    old_set: BitSet,
}

impl<'a> System<'a> for ChunkManager {
    type SystemData = (Write<'a, RustelWorld<Block>, PanicHandler>, ReadStorage<'a, Position>, ReadStorage<'a, ChunkLoader>);

    fn run(&mut self, (mut world, positions, loaders): Self::SystemData) {
        loaders.populate_inserted(&mut self.new_loaders, &mut self.new_set);
        loaders.populate_removed(&mut self.old_loaders, &mut self.old_set);

        for (&Position(pos), _) in (&positions, &self.new_set).join() {
            let chunk_pos = Point3::new(pos.x.floor() as i32, pos.y.floor() as i32, pos.z.floor() as i32) / CHUNK_SIZE as i32;
            world.generator.request(chunk_pos);
        }

        world.flush_finished();
    }
}

struct TerrainRenderer;

impl<'a> System<'a> for TerrainRenderer {
    type SystemData = Read<'a, RustelWorld<Block>, PanicHandler>;
    fn run(&mut self, world: Self::SystemData) {

    }
}

// #[derive(Clone, Debug, PartialEq)]
// pub struct Transform(Matrix4<f32>);

// #[derive(Copy, Clone, Debug, PartialEq)]
// pub struct Player {
//     pub position: Point3<f64>,
//     pub velocity: Vector3<f64>,
//     pub acceleration: Vector3<f64>,
//     pub look_vec: Vector3<f64>,
// }

// impl Player {
//     fn apply_acceleration(&mut self, acceleration: Vector3<f64>) {
//         self.acceleration += acceleration;
//     }

//     fn integrate(&mut self, dt: f64) {
//         self.position += self.velocity * dt;
//         self.velocity += self.acceleration * dt;
//         // Reset acceleration each frame. A continually applied force will set
//         // the acceleration each frame, so resetting will clear forces that are
//         // no longer applied.
//         self.acceleration = Vector3::new(0.0, 0.0, 0.0);
//     }
// }



// #[derive(Debug)]
// pub struct Config {
//     acceleration: f64,
//     fast_acceleration: f64,
//     max_fall_speed: f32,
//     jump_velocity: f32,
//     gravity: f32,
// }

// pub struct Application {
//     jumping: bool,

//     player: Player,
//     time: f32,
//     frames: i32,
//     previous_cursor_x: f32,
//     previous_cursor_y: f32,
//     selection_start: Option<Point3<i32>>,
//     selected_block: Block,
//     select_queue: Vec<Vec<Point3<i32>>>,

//     _textures: Texture2D,
//     cfg: Config,
//     camera: Camera,
//     pipeline: LinkedProgram,
//     debug_pipeline: LinkedProgram,
//     chunk_manager: ChunkManager,
// }



// impl<'a> System<'a> for Application {
//     type SystemData = ();

//     fn run(&mut self, (): ()) {
//         // let now = glfw.get_time();
//         // let dt = now - prev_time;
//         // prev_time = now;

//         // self.handle_inputs(&inputs, 1.0);
//         self.update(1.0);
//         self.draw(1.0);

//         // window.swap_buffers();
//     }
// }

type InputReceiver = ::std::sync::mpsc::Receiver<(f64, ::glfw::WindowEvent)>;

// impl Application {
//     crate fn new(mut glfw: Glfw, window: Window, event_recv: InputReceiver, mut pipeline: LinkedProgram, mut debug_pipeline: LinkedProgram) {
//         let window = SharedWindow::new(window);
//         let events = SharedEvents::new(MultiReciever::from(event_recv));

//         // let mode_switcher = ModeSwitcher { window: window.clone(), events: events.clone() };
//         // let close_handler = CloseHandler { events: events.clone() };
//         let input_handler = InputHandler { window: window.clone(), events: events.clone() };

//         unsafe { gl_call!(Viewport(0, 0, 600, 600)).expect("glViewport failed"); }
//         let projection = ::cgmath::perspective(Deg(70.0), 600.0 / 600.0, 0.1, 1000.0f32);
//         pipeline.set_uniform("u_Projection", &projection);
//         debug_pipeline.set_uniform("projection", &projection);

//         // let mut poses = Vec::new();
//         // let mut colors = Vec::new();
//         // let mut attenuations = Vec::new();

//         // for i in 0..3 {
//         //     poses.push(Vector3::new(i as f32 * 10.0, 0.5, 0.5));
//         //     colors.push(Vector3::new(i as f32/3.0, 0.0, 0.0));
//         //     attenuations.push(0.5f32);
//         // }

//         // pipeline.set_uniform("u_Light", &poses.as_slice());
//         // pipeline.set_uniform("u_LightColor", &colors.as_slice());
//         // pipeline.set_uniform("u_LightAttenuation", &attenuations.as_slice());
//         pipeline.set_uniform("u_LightAmbient", &Vector3::<f32>::new(0.4, 0.5, 0.6));

//         let chunk_manager = ChunkManager::new(NoiseGenerator::new_default());

//         use gl_api::texture::*;

//         let textures = Texture2D::new();
//         textures.source_from_image("resources/textures.png").unwrap();
//         textures.mag_filter(MagnificationFilter::Nearest);
//         textures.min_filter(MinimizationFilter::Nearest);
//         textures.texture_wrap_behavior(TextureAxis::S, WrapMode::Repeat);
//         textures.texture_wrap_behavior(TextureAxis::T, WrapMode::Repeat);
//         pipeline.set_uniform("u_TextureMap", &textures);

//         let mut world = World::default();

//         world.add_resource(StopGameLoop(false));
//         world.add_resource(Modes::default());

//         let mut application = Application {
//             cfg: Config {
//                 acceleration: 0.5,
//                 fast_acceleration: 0.75,
//                 max_fall_speed: 2.0,
//                 jump_velocity: 6.5,
//                 gravity: 16.0,
//             },
//             player: Player {
//                 position: Point3::new(0.0, 0.0, 0.0),
//                 velocity: Vector3::new(0.0, 0.0, 0.0),
//                 acceleration: Vector3::new(0.0, 0.0, 0.0),
//                 look_vec: Vector3::new(0.0, 0.0, 0.0),
//                 // mass: 100.0,
//             },
//             jumping: false,
//             selection_start: None,
//             select_queue: Vec::new(),
//             previous_cursor_x: 0.0,
//             previous_cursor_y: 0.0,
//             selected_block: Block::Grass,
//             frames: 0,
//             time: 0.0,
//             _textures: textures,
//             camera: Camera::default(),
//             pipeline,
//             debug_pipeline,
//             chunk_manager,
//         };
        
//         // println!("Application built...");

//         let mut dispatcher = DispatcherBuilder::new()
//             // .with_thread_local(close_handler)
//             // .with_thread_local(mode_switcher)
//             .with_thread_local(input_handler)
//             .build();
        
//         dispatcher.setup(&mut world.res);
        
//         while !world.res.fetch::<StopGameLoop>().0 {
//             misc::clear(misc::ClearMode::Color(0.729411765, 0.907843137, 0.981568627, 1.0));
//             misc::clear(misc::ClearMode::Depth(1.0));

//             // Poll ofr new events, and fill the shared event buffer.
//             glfw.poll_events();
//             events.events().fill_queue();

//             // Update systems and the world.
//             world.maintain();
//             dispatcher.dispatch(&world.res);

//             application.update(0.001);
//             application.draw(0.001);

//             // Swap the backbuffer
//             window.window().swap_buffers();

//             // Clear the event buffer so we don't keep piling events on!
//             events.events().clear();
//         }

//     }

//     fn update_camera_rotation(&mut self, x: f32, y: f32) {
//         let dx = self.previous_cursor_x - x;
//         let dy = self.previous_cursor_y - y;
//         self.previous_cursor_x = x;
//         self.previous_cursor_y = y;

//         self.camera.rotate(CamRotation::AboutY(Deg(-dx as f64/3.0)));
//         self.camera.rotate(CamRotation::AboutX(Deg(-dy as f64/3.0)));
//     }

//     crate fn set_viewport(&mut self, width: i32, height: i32) {
//         unsafe { gl_call!(Viewport(0, 0, width, height)).expect("glViewport failed"); }

//         let projection = cgmath::perspective(Deg(70.0), width as f32 / height as f32, 0.1, 1000.0);
//         self.pipeline.set_uniform("u_Projection", &projection);
//         self.debug_pipeline.set_uniform("projection", &projection);
//     }

//     crate fn handle_event(&mut self, window: &mut Window, event: WindowEvent) -> bool {
//         match event {
//             WindowEvent::CursorPos(x, y) => self.update_camera_rotation(x as f32, y as f32),
//             WindowEvent::MouseButton(MouseButton::Button1, Action::Press, _) => self.start_selection(),
//             WindowEvent::MouseButton(MouseButton::Button1, Action::Release, _) => self.end_selection(),
//             WindowEvent::MouseButton(MouseButton::Button2, Action::Press, _) => self.place_block(),

//             WindowEvent::Key(Key::Escape, _, Action::Press, _) => return true,
//             WindowEvent::Scroll(_, dy) => {
//                 if dy > 0.0 {
//                     println!("Scroll up: {}", dy);
//                 }
//                 if dy < 0.0 {
//                     println!("Scroll down: {}", dy);
//                 }
//             }
            
//             WindowEvent::Size(width, height) => self.set_viewport(width, height),
//             _ => {}
//         }
//         false
//     }

//     fn selection_bounds(&self) -> Option<Aabb3<i32>> {
//         self.get_look_pos().and_then(|look| self.selection_start.map(|start| {
//             Aabb3::new(start, look)
//                 .union(&Aabb3::new(start, start + Vector3::new(1, 1, 1)))
//                 .union(&Aabb3::new(look, look + Vector3::new(1, 1, 1)))
//         }))
//     }

//     fn start_selection(&mut self) {
//         self.selection_start = self.get_look_pos();
//     }

//     fn end_selection(&mut self) {
//         let end = self.get_look_pos();
//         if let (Some(start), Some(end)) = (self.selection_start, end) {
//             if start == end {
//                 self.chunk_manager.set_voxel(start, Block::Air);
//             } else {
//                 let bounds = self.selection_bounds().unwrap();
//                 let mut vec = Vec::new();
//                 for x in bounds.min.x..bounds.max.x {
//                     for y in bounds.min.y..bounds.max.y {
//                         for z in bounds.min.z..bounds.max.z {
//                             let pos = Point3::new(x, y, z);
//                             if let Some(&voxel) = self.chunk_manager.world().get_voxel(pos) {
//                                 if voxel != Block::Air {
//                                     vec.push(pos);
//                                 }
//                             }
//                         }
//                     }
//                 }
//                 self.select_queue.push(vec);
//             }
//         }
//         self.selection_start = None;
//     }

//     // crate fn handle_inputs(&mut self, inputs: &Inputs, _dt: f64) {
//     //     if inputs.is_down(Key::Right) { self.camera.rotate(CamRotation::AboutY(Deg(1.0))); }
//     //     if inputs.is_down(Key::Left) { self.camera.rotate(CamRotation::AboutY(-Deg(1.0))); }
//     //     if inputs.is_down(Key::Up) { self.camera.rotate(CamRotation::AboutX(-Deg(1.0))); }
//     //     if inputs.is_down(Key::Down) { self.camera.rotate(CamRotation::AboutX(Deg(1.0))); }
        
//     //     let accel = if inputs.is_down(Key::LeftControl) {
//     //         self.cfg.fast_acceleration
//     //     } else {
//     //         self.cfg.acceleration
//     //     };

//     //     const INPUT_ACCEL_FACTOR: f64 = 100.0;
//     //     let (forward, right) = self.camera.get_spin_vecs();

//     //     if inputs.is_down(Key::W) {
//     //         self.player.apply_acceleration(-forward * INPUT_ACCEL_FACTOR);
//     //     }

//     //     if inputs.is_down(Key::S) {
//     //         self.player.apply_acceleration(forward * INPUT_ACCEL_FACTOR);
//     //     }

//     //     if inputs.is_down(Key::A) {
//     //         self.player.apply_acceleration(-right * INPUT_ACCEL_FACTOR);
//     //     }

//     //     if inputs.is_down(Key::D) {
//     //         self.player.apply_acceleration(right * INPUT_ACCEL_FACTOR);
//     //     }

//     //     if inputs.is_down(Key::Space) {
//     //         if !self.jumping && !self.noclip {
//     //             self.jumping = true;
//     //             self.player.velocity.y = 8.0;
//     //         }

//     //         if self.noclip {
//     //             self.player.apply_acceleration(Vector3::new(0.0, INPUT_ACCEL_FACTOR, 0.0));
//     //         }
//     //     }

//     //     if inputs.is_down(Key::LeftShift) {
//     //         if self.noclip {
//     //             self.player.apply_acceleration(Vector3::new(0.0, -INPUT_ACCEL_FACTOR, 0.0));
//     //         }
//     //     }
//     // }

//     fn collision_check(&mut self) {
//         let substeps = 3;
//         for _ in 0..substeps {
//             // let world = ;
//             let feet = Point3::new(
//                 self.player.position.x.floor() as i32,
//                 self.player.position.y.floor() as i32,
//                 self.player.position.z.floor() as i32);
//             let eyes = feet + Vector3::unit_y();

//             let around = [
//                 feet, eyes,

//                 feet - Vector3::unit_y(),
//                 feet + Vector3::unit_x(),
//                 feet - Vector3::unit_x(),
//                 feet + Vector3::unit_z(),
//                 feet - Vector3::unit_z(),

//                 eyes + Vector3::unit_y(),
//                 eyes + Vector3::unit_x(),
//                 eyes - Vector3::unit_x(),
//                 eyes + Vector3::unit_z(),
//                 eyes - Vector3::unit_z(),
//             ];
//             let around = around
//                 .into_iter()
//                 .filter(|&&pos| !self.chunk_manager.world().get_voxel(pos).map(|block| !block.properties().opaque).unwrap_or(false))
//                 .collect::<SmallVec<[_; 10]>>();

//             let gjk = GJK3::new();
//             const PLAYER_WIDTH: f64 = 0.45;
//             const PLAYER_HEIGHT: f64 = 1.8;

//             for block_pos in around {
//                 self.frame_at_voxel(block_pos.cast().unwrap(), Vector3::new(0.0, 1.0, 1.0), 0.003, false);
//                 let block_tfm = Matrix4::from_translation(
//                     ::util::to_vector(block_pos.cast().unwrap() + Vector3::new(0.5, 0.5, 0.5)),
//                 );

//                 let player_tfm = Matrix4::from_translation(
//                     ::util::to_vector(self.player.position + Vector3::new(0.0, PLAYER_HEIGHT / 2.0, 0.0)),
//                 );

//                 // NOTE: non-transparent blocks were filtered out
//                 if let Some(contact) = gjk.intersection(
//                     &CollisionStrategy::FullResolution,
//                     &Cuboid::new(PLAYER_WIDTH, PLAYER_HEIGHT, PLAYER_WIDTH),
//                     &player_tfm,
//                     &Cuboid::new(1.0, 1.0, 1.0),
//                     &block_tfm
//                 ) {
//                     let resolution = -contact.normal * contact.penetration_depth;

//                     // We check two cuboids here, so normals should be
//                     // axis-aligned. If any of the components are not zero, that
//                     // means we've had a collision on that face and should
//                     // cancel velocity in that direction. Alternatively, you
//                     // could multiply the component by something like -0.8 and
//                     // have a lot of fun!
//                     if resolution.x.abs() > 0.0 { self.player.velocity.x = 0.0; }
//                     if resolution.y.abs() > 0.0 { self.player.velocity.y = 0.0; }
//                     if resolution.y > 0.0 { self.jumping = false; }
//                     if resolution.z.abs() > 0.0 { self.player.velocity.z = 0.0; }
//                     // Let's say being inside a wall exerts some force
//                     // proportional to the depth inside the wall
//                     self.player.position += resolution;
//                 }
//             }
//         }
//     }
    
//     fn place_block(&mut self) {
//         let side = self.get_look_face();
//         let pos = self.get_look_pos();

//         if let (Some(side), Some(pos)) = (side, pos) {
//             self.chunk_manager.set_voxel(pos + side.offset(), self.selected_block);
//         }
//     }

//     crate fn update(&mut self, dt: f64) {
//         let view: Matrix4<f32> = self.camera.transform_matrix().cast().unwrap();
//         let cam_pos: Vector3<f32> = ::util::to_vector(self.camera.position).cast().unwrap();
//         self.pipeline.set_uniform("u_Time", &self.time);
//         self.pipeline.set_uniform("u_CameraPosition", &cam_pos);
//         self.pipeline.set_uniform("u_View", &view);
//         self.debug_pipeline.set_uniform("view", &view);

//         for queue in &mut self.select_queue {
//             if let Some(pos) = queue.pop() {
//                 self.chunk_manager.set_voxel(pos, Block::Air);
//             }
//         }

//         self.select_queue.retain(|queue| queue.len() > 0);

//         // What a mess...
//         // if self.noclip {
//         if false {
//             self.player.apply_acceleration(-self.player.velocity / 2.0);
//         } else {
//             let horizontal = Vector3::new(self.player.velocity.x, 0.0, self.player.velocity.z);
//             self.player.apply_acceleration(-horizontal * 20.0);
//             self.player.apply_acceleration(Vector3::new(0.0, -19.6, 0.0));
//         }
//         self.player.integrate(dt);
//         // if !self.noclip {
//         //     self.collision_check();
//         // }
//         self.camera.position = self.player.position + Vector3::new(0.0, 1.8 - 0.45, 0.0);

//         self.chunk_manager.update_player_position(self.player.position.cast().unwrap());
//         self.chunk_manager.tick();
//         self.time += 0.007;
//     }

//     fn get_look_face(&self) -> Option<Side> {
//         use cgmath::Point3;

//         // Thickness of the collision boxes on each face
//         let t = 0.1;
//         let look_vec = -self.camera.get_look_vec();

//         self.get_look_pos().and_then(|look_pos| {
//             let ray = Ray3::new(self.camera.position, look_vec);
//             let look_pos = look_pos.cast().unwrap();
//             let cam_pos = ::util::to_vector(self.camera.position);
//             let (l, h) = (look_pos, look_pos + Vector3::new(1.0, 1.0, 1.0));

//             // These are the bounding boxes of each face. They are each face,
//             // stretched out `t` units in their corresponding direction
//             let bb_left   = Aabb3::new(Point3::new(l.x, l.y, l.z), Point3::new(l.x - t, h.y, h.z));
//             let bb_right  = Aabb3::new(Point3::new(h.x, l.y, l.z), Point3::new(h.x + t, h.y, h.z));
//             let bb_bottom = Aabb3::new(Point3::new(l.x, l.y, l.z), Point3::new(h.x, l.y - t, h.z));
//             let bb_top    = Aabb3::new(Point3::new(l.x, h.y, l.z), Point3::new(h.x, h.y + t, h.z));
//             let bb_back   = Aabb3::new(Point3::new(l.x, l.y, l.z), Point3::new(h.x, h.y, l.z - t));
//             let bb_front  = Aabb3::new(Point3::new(l.x, l.y, h.z), Point3::new(h.x, h.y, h.z + t));

//             // Center positions of each face. We use these for sorting which
//             // faces are closest to the camera so we don't accidentally select
//             // backfaces
//             let pos_left   = Vector3::new(l.x, (l.y + h.y) * 0.5, (l.z + h.z) * 0.5);
//             let pos_right  = Vector3::new(h.x, (l.y + h.y) * 0.5, (l.z + h.z) * 0.5);
//             let pos_bottom = Vector3::new((l.x + h.x) * 0.5, l.y, (l.z + h.z) * 0.5);
//             let pos_top    = Vector3::new((l.x + h.x) * 0.5, h.y, (l.z + h.z) * 0.5);
//             let pos_back   = Vector3::new((l.x + h.x) * 0.5, (l.y + h.y) * 0.5, l.z);
//             let pos_front  = Vector3::new((l.x + h.x) * 0.5, (l.y + h.y) * 0.5, h.z);

//             let items = &mut [
//                 (Side::Left, pos_left, bb_left),
//                 (Side::Right, pos_right, bb_right),
//                 (Side::Bottom, pos_bottom, bb_bottom),
//                 (Side::Top, pos_top, bb_top),
//                 (Side::Back, pos_back, bb_back),
//                 (Side::Front, pos_front, bb_front),
//             ];

//             // Sort the list by distance to the camera
//             items.sort_by(|&(_, a, _), &(_, b, _)| a.distance2(cam_pos)
//                 .partial_cmp(&b.distance2(cam_pos))
//                 .unwrap_or(Ordering::Equal));
            
//             // Now get the side closest to the camera that intersects the ray
//             // extending from the player's eyes
//             items.iter().filter(|&&(_, _, aabb)| ray.intersects(&aabb)).map(|&(side, _, _)| side).next()
//         })
//     }

//     fn get_look_pos(&self) -> Option<Point3<i32>> {
//         use cgmath::MetricSpace;
//         const LOOK_REACH: usize = 50;

//         let look_vec = -self.camera.get_look_vec();
//         // Ray extending from the player's eye in their look direction forever.
//         let ray = Ray3::new(self.camera.position, look_vec);
//         let samples = 2 * LOOK_REACH;

//         for k in 0..samples {
//             // the length of the current reach vector. `k/samples` ranges from 0
//             // to 1, so `length` ranges from 0 to LOOK_REACH
//             let length = LOOK_REACH as f64 * k as f64 / samples as f64;
//             // the coordinates of the current space we're checking. it's the
//             // player's eye position, offset by the current portion of the reach
//             // vector
//             let base_pos = self.camera.position + look_vec * length;

//             // Look at all the voxels around the current center. If the voxel is
//             // solid and intersects the look ray, then we add it to the list
//             let mut to_sort = SmallVec::<[Point3<f64>; 32]>::new();
//             self.chunk_manager.world().around_voxel(base_pos.cast().unwrap(), 2, |pos, voxel| {
//                 let pos = pos.cast().unwrap();
//                 // Get the bounding box of the entire cube
//                 let bbox = Aabb3::new(pos, pos + Vector3::new(1.0, 1.0, 1.0));
//                 if voxel.properties().opaque && ray.intersects(&bbox) {
//                     to_sort.push(pos);
//                 }
//             });

//             // It could happen that a we select a voxel behind the current one
//             // if it happened to be added to the list before the one that's
//             // actually closer to the camera, so we sort the list of possible
//             // collisions by distance to the camera and pick the first element
//             to_sort.sort_by(|a, b| a.distance2(self.camera.position)
//                 .partial_cmp(&b.distance2(self.camera.position))
//                 .unwrap_or(Ordering::Equal));
            
//             if let Some(pos) = to_sort.first() {
//                 return Some(pos.cast().unwrap());
//             }
//         }

//         None
//     }

//     crate fn draw(&mut self, _dt: f64) {
//         let look_pos = self.get_look_pos();
//         let side = self.get_look_face();
//         // Draw frame around the block we're looking at
//         if let Some(look) = look_pos {
//             self.frame_at_voxel(Point3::new(look.x as f64, look.y as f64, look.z as f64), Vector3::new(1.0, 0.3, 0.0), 0.01, true);
//             if let Some(side) = side {
//                 self.frame_at_voxel(Point3::new(look.x as f64, look.y as f64, look.z as f64) + side.offset(), Vector3::new(0.0, 0.8, 0.6), 0.008, false);
//             }
//         }

//         if let Some(aabb) = self.selection_bounds() {
//             self.draw_frame(Aabb3::new(aabb.min.cast().unwrap(), aabb.max.cast().unwrap()), Vector3::new(1.0, 0.5, 0.0), 0.02, true);
//         }

//         self.draw_frame(Aabb3::new(
//             ::util::to_point(::util::to_vector(self.camera.position) - Vector3::new(9.0, 9.0, 9.0)),
//             ::util::to_point(::util::to_vector(self.camera.position) + Vector3::new(9.0, 9.0, 9.0)),
//         ), Vector3::new(0.0, 1.0, 0.0), 0.02, false);

//         self.chunk_manager.draw(&mut self.pipeline).expect("Drawing chunks failed");
//         self.frames += 1;
//     }

//     fn draw_frame(&mut self, aabb: Aabb3<f64>, color: Vector3<f64>, thickness: f64, force: bool) {
//         // if self.debug_frames || force {
//             let aabb = Aabb3::new(aabb.min.cast().unwrap(), aabb.max.cast().unwrap());
//             ::debug::draw_frame(&mut self.debug_pipeline, aabb, color.cast().unwrap(), thickness as f32);
//         // }
//     }

//     fn frame_at_voxel(&mut self, pos: Point3<f64>, color: Vector3<f64>, thickness: f64, force: bool) {
//         self.draw_frame(Aabb3::new(
//             pos, pos + Vector3::new(1.0, 1.0, 1.0),
//         ), color, thickness, force);
//     }
// }
