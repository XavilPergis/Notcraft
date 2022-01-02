#[macro_use]
extern crate log;
#[macro_use]
extern crate serde_derive;

pub mod engine;
pub mod util;

#[derive(Copy, Clone, Debug, PartialEq)]
pub struct PlayerController {
    player: Entity,
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub enum CameraControllerMode {
    Follow(Entity),
    Static,
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub struct CameraController {
    camera: Entity,
    mode: CameraControllerMode,
}

use crate::engine::{
    input::{keys, DigitalInput, InputState},
    render::{
        camera::{ActiveCamera, Camera},
        renderer::{add_debug_box, DebugBox, DebugBoxKind, Renderer},
    },
    world::{registry::AIR, BlockPos, Ray3, VoxelWorld},
};
use engine::{
    audio::{intermittent_music_system, MusicState},
    input::input_compiler_system,
    physics::{
        apply_gravity_system, apply_rigidbody_motion_system, terrain_collision_system,
        update_previous_colliders_system, AabbCollider, RigidBody,
    },
    render::{
        mesher::{chunk_mesher_system, MesherContext},
        renderer::Aabb,
    },
    transform::Transform,
    world::{
        chunk::ChunkSnapshotCache,
        load_chunks_system,
        registry::{BlockId, BlockRegistry},
        trace_ray, update_world_system, ChunkLoaderContext, DynamicChunkLoader, RaycastHit,
    },
    Axis, Dt, Side, StopGameLoop,
};
use glium::{
    glutin::{
        event::{
            ButtonId, ElementState, Event, KeyboardInput, ModifiersState, VirtualKeyCode,
            WindowEvent,
        },
        event_loop::{ControlFlow, EventLoop},
        window::WindowBuilder,
        ContextBuilder,
    },
    Display,
};
use legion::{systems::CommandBuffer, world::SubWorld, *};
use nalgebra::{point, vector, Point3, UnitQuaternion, Vector2, Vector3};
use std::{
    rc::Rc,
    sync::Arc,
    time::{Duration, Instant},
};

#[legion::system]
fn camera_controller(
    #[resource] input: &InputState,
    #[resource] camera_controller: &mut CameraController,
    #[resource] player_controller: &mut PlayerController,
    world: &mut SubWorld,
    transform_query: &mut Query<&mut Transform>,
) {
    let mut update_camera_transform =
        |camera_controller: &mut CameraController, entity| match transform_query
            .get_mut(world, entity)
            .ok()
            .copied()
        {
            None => camera_controller.mode = CameraControllerMode::Static,
            Some(player_transform) => {
                let camera_transform = transform_query
                    .get_mut(world, camera_controller.camera)
                    .unwrap();
                *camera_transform = player_transform.translated(&nalgebra::vector![0.0, 0.5, 0.0]);
            }
        };

    match camera_controller.mode {
        CameraControllerMode::Follow(entity) => update_camera_transform(camera_controller, entity),
        CameraControllerMode::Static => {}
    }

    if input
        .key(VirtualKeyCode::S)
        .require_modifiers(ModifiersState::CTRL | ModifiersState::SHIFT)
        .is_rising()
    {
        update_camera_transform(camera_controller, player_controller.player);
        camera_controller.mode = CameraControllerMode::Static;
    }

    if input
        .key(VirtualKeyCode::F)
        .require_modifiers(ModifiersState::CTRL | ModifiersState::SHIFT)
        .is_rising()
    {
        camera_controller.mode = CameraControllerMode::Follow(player_controller.player);
    }
}

#[derive(Copy, Clone, Debug)]
pub struct TerrainManipulator {
    start_pos: Option<BlockPos>,
    start_button: Option<ButtonId>,
}

fn make_ray(transform: &Transform, reference: &Vector3<f32>) -> Ray3<f32> {
    Ray3 {
        direction: transform
            .rotation
            .to_quaternion()
            .transform_vector(reference),
        origin: Point3::from(transform.translation.vector),
    }
}

fn iter_blocks_in(a: BlockPos, b: BlockPos, mut func: impl FnMut(BlockPos)) {
    let xmin = i32::min(a.x, b.x);
    let ymin = i32::min(a.y, b.y);
    let zmin = i32::min(a.z, b.z);
    let xmax = i32::max(a.x, b.x);
    let ymax = i32::max(a.y, b.y);
    let zmax = i32::max(a.z, b.z);
    for x in xmin..=xmax {
        for z in zmin..=zmax {
            for y in ymin..=ymax {
                func(BlockPos { x, y, z });
            }
        }
    }
}

fn box_enclosing(a: BlockPos, b: BlockPos) -> Aabb {
    let xmin = i32::min(a.x, b.x);
    let ymin = i32::min(a.y, b.y);
    let zmin = i32::min(a.z, b.z);
    let xmax = i32::max(a.x, b.x) + 1;
    let ymax = i32::max(a.y, b.y) + 1;
    let zmax = i32::max(a.z, b.z) + 1;
    let min = point![xmin as f32, ymin as f32, zmin as f32];
    let max = point![xmax as f32, ymax as f32, zmax as f32];
    Aabb { min, max }
}

fn terrain_manipulation_area(
    input: &InputState,
    hit: &RaycastHit,
    ctx: &mut TerrainManipulationContext,
) {
    if let Some(start_pos) = ctx.manip.start_pos {
        let start_button = ctx.manip.start_button.unwrap();

        if start_button == 1 {
            add_debug_box(DebugBox {
                bounds: box_enclosing(start_pos, hit.pos),
                rgba: [1.0, 0.7, 0.2, 0.8],
                kind: DebugBoxKind::Dotted,
            });
            if input.key(DigitalInput::Button(1)).is_falling() {
                iter_blocks_in(start_pos, hit.pos, |pos| {
                    ctx.set_block(pos, AIR);
                });
                ctx.manip.start_pos = None;
                ctx.manip.start_button = None;
            }
        }

        if start_button == 3 {
            let offset = hit
                .side
                .map(|side| side.normal::<i32>())
                .unwrap_or_default();
            let end_pos = BlockPos {
                x: hit.pos.x + offset.x,
                y: hit.pos.y + offset.y,
                z: hit.pos.z + offset.z,
            };
            add_debug_box(DebugBox {
                bounds: box_enclosing(start_pos, end_pos),
                rgba: [0.2, 0.2, 1.0, 0.8],
                kind: DebugBoxKind::Dotted,
            });
            if input.key(DigitalInput::Button(3)).is_falling() {
                let id = ctx.world.registry.get_id("stone");
                iter_blocks_in(start_pos, end_pos, |pos| {
                    ctx.set_block(pos, id);
                });
                ctx.manip.start_pos = None;
                ctx.manip.start_button = None;
            }
        }
    } else {
        if input.key(DigitalInput::Button(1)).is_rising() {
            ctx.manip.start_pos = Some(hit.pos);
            ctx.manip.start_button = Some(1);
        } else if input.key(DigitalInput::Button(3)).is_rising() {
            let offset = hit
                .side
                .map(|side| side.normal::<i32>())
                .unwrap_or_default();
            let start_pos = BlockPos {
                x: hit.pos.x + offset.x,
                y: hit.pos.y + offset.y,
                z: hit.pos.z + offset.z,
            };
            ctx.manip.start_pos = Some(start_pos);
            ctx.manip.start_button = Some(3);
        }
    }
}

fn replace_axis(mut pos: BlockPos, axis: Axis, value: i32) -> BlockPos {
    pos[axis] = value;
    pos
}

fn build_to_me_positive(
    ctx: &TerrainManipulationContext,
    input: &InputState,
    axis: Axis,
    from: BlockPos,
    to: BlockPos,
    id: BlockId,
) {
    if from[axis] > to[axis] {
        return;
    }

    let mut cache = ChunkSnapshotCache::new(ctx.world);

    let mut max_n = from[axis];
    for n in from[axis]..=to[axis] {
        let pos = replace_axis(from, axis, n);
        if cache
            .block(pos)
            .map_or(true, |id| ctx.world.registry.collidable(id))
        {
            break;
        }
        max_n = n;
    }

    add_debug_box(DebugBox {
        bounds: box_enclosing(from, replace_axis(from, axis, max_n)),
        rgba: [0.2, 0.2, 1.0, 0.8],
        kind: DebugBoxKind::Dotted,
    });

    if input.key(DigitalInput::Button(3)).is_rising() {
        for n in from[axis]..=max_n {
            ctx.set_block(replace_axis(from, axis, n), id);
        }
    }
}

fn build_to_me_negative(
    ctx: &TerrainManipulationContext,
    input: &InputState,
    axis: Axis,
    from: BlockPos,
    to: BlockPos,
    id: BlockId,
) {
    if from[axis] < to[axis] {
        return;
    }

    let mut cache = ChunkSnapshotCache::new(ctx.world);

    let mut min_n = from[axis];
    for n in (to[axis]..=from[axis]).rev() {
        let pos = replace_axis(from, axis, n);
        if cache
            .block(pos)
            .map_or(true, |id| ctx.world.registry.collidable(id))
        {
            break;
        }
        min_n = n;
    }

    add_debug_box(DebugBox {
        bounds: box_enclosing(from, replace_axis(from, axis, min_n)),
        rgba: [0.2, 0.2, 1.0, 0.8],
        kind: DebugBoxKind::Dotted,
    });

    if input.key(DigitalInput::Button(3)).is_rising() {
        for n in min_n..=from[axis] {
            ctx.set_block(replace_axis(from, axis, n), id);
        }
    }
}

fn terrain_manipulation_build_to_me(
    input: &InputState,
    hit: &RaycastHit,
    ctx: &mut TerrainManipulationContext,
) {
    let id = ctx.world.registry.get_id("stone");
    if let Some(side) = hit.side {
        let offset = side.normal::<i32>();
        let start_pos = BlockPos {
            x: hit.pos.x + offset.x,
            y: hit.pos.y + offset.y,
            z: hit.pos.z + offset.z,
        };
        let player_pos = BlockPos {
            x: ctx.transform.translation.x.floor() as i32,
            y: ctx.transform.translation.y.floor() as i32,
            z: ctx.transform.translation.z.floor() as i32,
        };

        match side {
            Side::Top => {
                build_to_me_positive(ctx, input, Axis::Y, start_pos, player_pos, id);
            }
            Side::Bottom => {
                build_to_me_negative(ctx, input, Axis::Y, start_pos, player_pos, id);
            }
            Side::Right => {
                build_to_me_positive(ctx, input, Axis::X, start_pos, player_pos, id);
            }
            Side::Left => {
                build_to_me_negative(ctx, input, Axis::X, start_pos, player_pos, id);
            }
            Side::Front => {
                build_to_me_positive(ctx, input, Axis::Z, start_pos, player_pos, id);
            }
            Side::Back => {
                build_to_me_negative(ctx, input, Axis::Z, start_pos, player_pos, id);
            }

            _ => {}
        }
    }
}

fn terrain_manipulation_single(
    input: &InputState,
    hit: &RaycastHit,
    ctx: &mut TerrainManipulationContext,
) {
    add_debug_box(DebugBox {
        bounds: util::block_aabb(hit.pos),
        rgba: [1.0, 0.7, 0.2, 0.8],
        kind: DebugBoxKind::Dotted,
    });
    if input.key(DigitalInput::Button(1)).is_rising() {
        ctx.set_block(hit.pos, AIR);
    }

    if let Some(side) = hit.side {
        let norm = side.normal::<i32>();
        let offset = BlockPos {
            x: hit.pos.x + norm.x,
            y: hit.pos.y + norm.y,
            z: hit.pos.z + norm.z,
        };
        add_debug_box(DebugBox {
            bounds: util::block_aabb(offset),
            rgba: [0.2, 0.2, 1.0, 0.8],
            kind: DebugBoxKind::Dotted,
        });
        if input.key(DigitalInput::Button(3)).is_rising() {
            let id = ctx.world.registry.get_id("stone");
            ctx.set_block(offset, id);
        }
    }
}

struct TerrainManipulationContext<'a> {
    world: &'a Arc<VoxelWorld>,
    manip: &'a mut TerrainManipulator,
    transform: &'a Transform,
    // collider: &'a AabbCollider,
}

impl<'a> TerrainManipulationContext<'a> {
    fn set_block(&self, pos: BlockPos, id: BlockId) {
        // if !self
        //     .collider
        //     .aabb
        //     .transformed(self.transform)
        //     .intersects(&util::block_aabb(pos))
        // {
        // }
        self.world.set_block(pos, id);
    }
}

#[legion::system(for_each)]
fn terrain_manipulation(
    #[resource] input: &InputState,
    #[resource] voxel_world: &Arc<VoxelWorld>,
    transform: &Transform,
    // collider: &AabbCollider,
    manip: &mut TerrainManipulator,
) {
    // mode: single, build to me, area
    // single - no modifiers
    // build to me - ctrl
    // area - ctrl + shift

    // button 1 - left click
    // button 2 - middle click
    // button 3 - right click
    let mut cache = ChunkSnapshotCache::new(voxel_world);
    if let Some(hit) = trace_ray(&mut cache, make_ray(transform, &-Vector3::z()), 20.0) {
        add_debug_box(DebugBox {
            bounds: util::block_aabb(hit.pos).inflate(0.005),
            rgba: [0.0, 0.0, 0.0, 0.8],
            kind: DebugBoxKind::Solid,
        });

        let mut ctx = TerrainManipulationContext {
            world: voxel_world,
            manip,
            transform,
            // collider,
        };

        if ctx.manip.start_pos.is_some() || (input.ctrl() && input.shift()) {
            terrain_manipulation_area(input, &hit, &mut ctx);
        } else if ctx.manip.start_pos.is_none() && input.ctrl() {
            terrain_manipulation_build_to_me(input, &hit, &mut ctx);
        } else if ctx.manip.start_pos.is_none() {
            terrain_manipulation_single(input, &hit, &mut ctx);
        }
    }
}

#[legion::system]
fn player_controller(
    #[resource] input: &InputState,
    #[resource] player_controller: &mut PlayerController,
    world: &mut SubWorld,
    player_query: &mut Query<(&mut Transform, &mut RigidBody, &AabbCollider)>,
) {
    use std::f32::consts::PI;

    let pitch_delta = input.cursor_delta().y * (PI / 180.0);
    let yaw_delta = input.cursor_delta().x * (PI / 180.0);

    if let Some((transform, rigidbody, collider)) =
        player_query.get_mut(world, player_controller.player).ok()
    {
        transform.rotation.yaw -= yaw_delta;
        transform.rotation.pitch -= pitch_delta;
        transform.rotation.pitch = util::clamp(transform.rotation.pitch, -PI / 2.0, PI / 2.0);

        let mut vert_acceleration = 10.5;
        let mut horiz_acceleration = 45.0;

        // let mut speed = 5.0 * dt.as_secs_f32();

        if input.key(VirtualKeyCode::LControl).is_pressed() {
            // speed *= 10.0;
            horiz_acceleration *= 4.0;
            vert_acceleration *= 5.0;
        }

        if input.key(keys::FORWARD).is_pressed() {
            rigidbody.acceleration +=
                transform_project_xz(transform, nalgebra::vector![0.0, -horiz_acceleration]);
            // transform.translation.vector +=
            //     transform_project_xz(transform, nalgebra::vector![0.0,
            // -speed]);
        }
        if input.key(keys::BACKWARD).is_pressed() {
            rigidbody.acceleration +=
                transform_project_xz(transform, nalgebra::vector![0.0, horiz_acceleration]);
            // transform.translation.vector +=
            //     transform_project_xz(transform, nalgebra::vector![0.0,
            // speed]);
        }
        if input.key(keys::RIGHT).is_pressed() {
            rigidbody.acceleration +=
                transform_project_xz(transform, nalgebra::vector![horiz_acceleration, 0.0]);
            // transform.translation.vector +=
            //     transform_project_xz(transform, nalgebra::vector![speed,
            // 0.0]);
        }
        if input.key(keys::LEFT).is_pressed() {
            rigidbody.acceleration +=
                transform_project_xz(transform, nalgebra::vector![-horiz_acceleration, 0.0]);
            // transform.translation.vector +=
            //     transform_project_xz(transform, nalgebra::vector![-speed,
            // 0.0]);
        }
        if input.key(keys::UP).is_pressed() {
            if collider.on_ground {
                rigidbody.velocity.y = vert_acceleration;
            }
            // transform.translation.vector.y += speed;
        }
        if input.key(keys::DOWN).is_pressed() {
            // rigidbody.acceleration += nalgebra::vector![0.0, -acceleration,
            // 0.0];
            // transform.translation.vector.y -= speed;
        }

        // 0.96 with horiz_acceleration=30.0 is good for flight or slippery surfaces or
        // such rigidbody.velocity.x *= 0.96;
        // rigidbody.velocity.z *= 0.96;

        rigidbody.velocity.x *= 0.88;
        rigidbody.velocity.z *= 0.88;

        if input
            .key(VirtualKeyCode::C)
            .require_modifiers(ModifiersState::CTRL)
            .is_rising()
        {
            let grabbed = input.is_cursor_grabbed();

            input.grab_cursor(!grabbed);
            input.hide_cursor(!grabbed);
        }
    }
}

fn transform_project_xz(transform: &Transform, translation: Vector2<f32>) -> Vector3<f32> {
    // remove all components of the rotation except for the rotation in the XZ plane
    let lateral_rotation = UnitQuaternion::from_euler_angles(0.0, transform.rotation.yaw, 0.0);
    let local_translation = nalgebra::vector![translation.x, 0.0, translation.y];
    lateral_rotation * local_translation
}

fn setup_world(cmd: &mut CommandBuffer) {
    let player = cmd.push((
        Transform::default().translated(&nalgebra::vector![0.0, 20.0, 0.0]),
        AabbCollider::new(Aabb::with_dimensions(nalgebra::vector![0.8, 2.0, 0.8])),
        RigidBody::default(),
        DynamicChunkLoader {
            load_radius: 7,
            unload_radius: 8,
        },
    ));
    let camera = cmd.push((
        Camera::default(),
        Transform::default(),
        TerrainManipulator {
            start_pos: None,
            start_button: None,
        },
    ));

    cmd.exec_mut(move |_, res| {
        res.insert(ActiveCamera(Some(camera)));
        res.insert(CameraController {
            mode: CameraControllerMode::Follow(player),
            camera,
        });
        res.insert(PlayerController { player });
    });
}

struct MainContext {
    duration_samples: Option<Vec<Duration>>,
    start_instant: Option<Instant>,

    schedule: Schedule,
    world: World,
    resources: Resources,

    display: Rc<Display>,
    renderer: Renderer,
}

fn start_frame(ctx: &mut MainContext) {
    ctx.start_instant = Some(Instant::now());
}

fn end_frame_processing(ctx: &mut MainContext) {
    assert!(ctx.start_instant.is_some(), "sample was not started!");

    let elapsed = ctx.start_instant.unwrap().elapsed();
    if let Some(samples) = ctx.duration_samples.as_mut() {
        samples.push(elapsed);
    }
}

fn end_frame(ctx: &mut MainContext) {
    assert!(ctx.start_instant.is_some(), "sample was not started!");

    let dt = ctx.start_instant.unwrap().elapsed();
    *ctx.resources.get_mut::<Dt>().unwrap() = Dt(dt);
}

fn report_frame_samples(ctx: &mut MainContext) {
    if let Some(samples) = ctx.duration_samples.as_mut() {
        if samples.len() >= 1000 {
            let len = samples.len() as u32;
            let average_duration = samples.drain(..).sum::<Duration>() / len;

            log::debug!(
                "Frame took {} ms on average ({} fps)",
                average_duration.as_secs_f64() * 1000.0,
                1.0 / average_duration.as_secs_f64()
            );
        }
    }
}

fn run_frame(ctx: &mut MainContext) {
    start_frame(ctx);

    // update
    ctx.schedule.execute(&mut ctx.world, &mut ctx.resources);

    // draw
    // TODO: might want to integrate this into ECS, might not
    let mut frame = ctx.display.draw();
    ctx.renderer
        .draw(&mut frame, &mut ctx.world, &mut ctx.resources)
        .unwrap();

    end_frame_processing(ctx);

    frame.finish().unwrap();

    end_frame(ctx);
    report_frame_samples(ctx);
}

fn main() {
    simple_logger::init_with_level(log::Level::Debug).unwrap();

    let mut world = World::default();
    let mut resources = Resources::default();

    let registry = BlockRegistry::load_from_file("resources/blocks.json").unwrap();
    let voxel_world = VoxelWorld::new(Arc::clone(&registry));

    let mesher_ctx = MesherContext::new(&voxel_world);

    {
        let mut setup_buf = CommandBuffer::new(&world);
        setup_world(&mut setup_buf);
        setup_buf.flush(&mut world, &mut resources);

        resources.insert(voxel_world);
        resources.insert(engine::input::InputState::default());
        resources.insert(StopGameLoop(false));
        resources.insert(Dt(Duration::from_secs(1)));
    }

    let event_loop = EventLoop::new();
    let (window_events_tx, window_events_rx) = crossbeam_channel::unbounded();

    let window = WindowBuilder::new().with_title("Notcraft™");
    let ctx = ContextBuilder::new().with_depth_buffer(24).with_vsync(true);

    let display = Rc::new(Display::new(window, ctx, &event_loop).unwrap());

    let renderer = Renderer::new(
        Rc::clone(&display),
        Arc::clone(&registry),
        &mut world,
        &mut resources,
    )
    .unwrap();

    let schedule = Schedule::builder()
        .add_thread_local(input_compiler_system(window_events_rx, Rc::clone(&display)))
        .add_thread_local(intermittent_music_system(MusicState::new()))
        .add_system(chunk_mesher_system(mesher_ctx))
        // all modifications to entities with `Transform`s + `AabbCollider`s should be made after
        // this system has been flushed.
        .add_system(update_previous_colliders_system())
        // maintain the world (primarily flush queued chunk writes). systems that read the current
        // state of the world should happen after this sytem has been flushed.
        .add_system(update_world_system())
        .flush()
        .add_system(player_controller_system())
        .add_system(apply_gravity_system())
        .flush()
        .add_system(apply_rigidbody_motion_system())
        .flush()
        // all modifications to entities with `Transform`s + `AabbCollider`s should be flushed by
        // this point.
        .add_system(terrain_collision_system())
        .flush()
        // modifications to the entity being followed by the camera controller should be flushed by
        // this point.
        .add_system(camera_controller_system())
        .add_system(load_chunks_system(ChunkLoaderContext::new(&mut world)))
        .flush()
        .add_system(terrain_manipulation_system())
        .build();

    let mut main_ctx = MainContext {
        duration_samples: Some(vec![]),
        start_instant: None,
        schedule,
        world,
        resources,
        display: Rc::clone(&display),
        renderer,
    };

    let mut quit_start_instant = None;

    event_loop.run(move |event, _target, cf| match event {
        Event::WindowEvent { event, .. } => match event {
            // TODO: move close handling code somewhere else mayhaps
            WindowEvent::CloseRequested => {
                *cf = ControlFlow::Exit;
            }

            WindowEvent::KeyboardInput {
                input:
                    KeyboardInput {
                        virtual_keycode: Some(VirtualKeyCode::Escape),
                        state,
                        ..
                    },
                ..
            } => match state {
                ElementState::Pressed => {
                    if quit_start_instant.is_none() {
                        quit_start_instant = Some(Instant::now());
                    }
                }
                ElementState::Released => quit_start_instant = None,
            },

            _ => (),
        },

        Event::DeviceEvent { device_id, event } => {
            window_events_tx.send((device_id, event)).unwrap()
        }

        Event::MainEventsCleared => display.gl_window().window().request_redraw(),
        Event::RedrawRequested(id) if id == display.gl_window().window().id() => {
            run_frame(&mut main_ctx);
            if quit_start_instant.map_or(false, |inst| inst.elapsed() >= Duration::from_secs(1)) {
                *cf = ControlFlow::Exit;
            }
        }
        _ => {}
    });
}
