#[macro_use]
extern crate log;
#[macro_use]
extern crate serde_derive;

pub mod engine;
pub mod util;

use crate::engine::{
    input::{keys, DigitalInput, InputState},
    prelude::*,
    render::{
        camera::{ActiveCamera, Camera},
        renderer::{add_debug_box, DebugBox, DebugBoxKind},
    },
    world::{registry::AIR, BlockPos, Ray3, VoxelWorld},
};
use app::{AppExit, Events};
use bevy_core::CorePlugin;
use engine::{
    input::{InputPlugin, RawInputEvent},
    physics::{AabbCollider, CollisionPlugin, PhysicsPlugin, RigidBody},
    render::{
        mesher::{ChunkMesherPlugin, MesherMode},
        renderer::{Aabb, RenderPlugin},
    },
    transform::Transform,
    world::{
        chunk::ChunkSnapshotCache, registry::BlockId, trace_ray, DynamicChunkLoader, RaycastHit,
        WorldPlugin,
    },
    Axis, Side,
};
use glium::{
    glutin::{
        event::{ButtonId, Event, ModifiersState, VirtualKeyCode, WindowEvent},
        event_loop::{ControlFlow, EventLoop},
        window::WindowBuilder,
        ContextBuilder,
    },
    Display,
};
use nalgebra::{point, Point3, UnitQuaternion, Vector2, Vector3};
use num_traits::Float;
use std::{rc::Rc, sync::Arc};
use structopt::StructOpt;

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

fn camera_controller(
    input: Res<InputState>,
    mut camera_controller: ResMut<CameraController>,
    player_controller: ResMut<PlayerController>,
    // world: &mut SubWorld,
    mut transform_query: Query<&mut Transform>,
) {
    let mut update_camera_transform =
        |camera_controller: &mut CameraController, entity| match transform_query
            .get_mut(entity)
            .ok()
            .as_deref()
            .copied()
        {
            None => camera_controller.mode = CameraControllerMode::Static,
            Some(player_transform) => {
                let mut camera_transform =
                    transform_query.get_mut(camera_controller.camera).unwrap();
                *camera_transform = player_transform.translated(&nalgebra::vector![0.0, 0.5, 0.0]);
            }
        };

    match camera_controller.mode {
        CameraControllerMode::Follow(entity) => {
            update_camera_transform(&mut camera_controller, entity)
        }
        CameraControllerMode::Static => {}
    }

    if input
        .key(VirtualKeyCode::S)
        .require_modifiers(ModifiersState::CTRL | ModifiersState::SHIFT)
        .is_rising()
    {
        update_camera_transform(&mut camera_controller, player_controller.player);
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
                rgba: [1.0, 0.2, 0.2, 0.8],
                kind: DebugBoxKind::Solid,
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
                kind: DebugBoxKind::Solid,
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
        add_debug_box(DebugBox {
            bounds: util::block_aabb(hit.pos),
            rgba: [1.0, 0.2, 0.2, 0.8],
            kind: DebugBoxKind::Solid,
        });
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
                kind: DebugBoxKind::Solid,
            });
        }
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
            .map_or(true, |id| ctx.world.registry.collision_type(id).is_solid())
        {
            break;
        }
        max_n = n;
    }

    add_debug_box(DebugBox {
        bounds: box_enclosing(from, replace_axis(from, axis, max_n)),
        rgba: [0.2, 0.2, 1.0, 0.8],
        kind: DebugBoxKind::Solid,
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
            .map_or(true, |id| ctx.world.registry.collision_type(id).is_solid())
        {
            break;
        }
        min_n = n;
    }

    add_debug_box(DebugBox {
        bounds: box_enclosing(from, replace_axis(from, axis, min_n)),
        rgba: [0.2, 0.2, 1.0, 0.8],
        kind: DebugBoxKind::Solid,
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
        rgba: [1.0, 0.2, 0.2, 0.8],
        kind: DebugBoxKind::Solid,
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
            kind: DebugBoxKind::Solid,
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

fn terrain_manipulation(
    input: Res<InputState>,
    voxel_world: Res<Arc<VoxelWorld>>,
    query: Query<(
        &Transform,
        // &AabbCollider,
        &mut TerrainManipulator,
    )>,
) {
    // transform: &Transform,
    // // collider: &AabbCollider,
    // manip: &mut TerrainManipulator,

    // mode: single, build to me, area
    // single - no modifiers
    // build to me - ctrl
    // area - ctrl + shift

    // button 1 - left click
    // button 2 - middle click
    // button 3 - right click
    query.for_each_mut(|(transform, mut manip)| {
        let mut cache = ChunkSnapshotCache::new(&voxel_world);
        if let Some(hit) = trace_ray(&mut cache, make_ray(transform, &-Vector3::z()), 20.0) {
            let mut ctx = TerrainManipulationContext {
                world: &voxel_world,
                manip: &mut manip,
                transform,
                // collider,
            };

            if ctx.manip.start_pos.is_some() || (input.ctrl() && input.shift()) {
                terrain_manipulation_area(&input, &hit, &mut ctx);
            } else if ctx.manip.start_pos.is_none() && input.ctrl() {
                terrain_manipulation_build_to_me(&input, &hit, &mut ctx);
            } else if ctx.manip.start_pos.is_none() {
                terrain_manipulation_single(&input, &hit, &mut ctx);
            }
        }
    });
}

fn player_look_controller(
    input: Res<InputState>,
    player_controller: ResMut<PlayerController>,
    mut query: Query<&mut Transform>,
) {
    use std::f32::consts::PI;

    let pitch_delta = input.cursor_delta().y.to_radians();
    let yaw_delta = input.cursor_delta().x.to_radians();

    if let Some(mut transform) = query.get_mut(player_controller.player).ok() {
        transform.rotation.yaw -= yaw_delta;
        transform.rotation.pitch -= pitch_delta;
        transform.rotation.pitch = util::clamp(transform.rotation.pitch, -PI / 2.0, PI / 2.0);
    }
}

fn player_controller(
    time: Res<Time>,
    input: Res<InputState>,
    player_controller: ResMut<PlayerController>,
    mut player_query: Query<(&mut Transform, &mut RigidBody, &AabbCollider)>,
) {
    if let Some((mut transform, mut rigidbody, collider)) =
        player_query.get_mut(player_controller.player).ok()
    {
        let mut vert_acceleration = 9.0;
        let mut horiz_acceleration = 70.0;

        if collider.on_ground {
            horiz_acceleration *= 0.85;
        }

        if input.key(VirtualKeyCode::LControl).is_pressed() {
            horiz_acceleration *= 5.5;
            vert_acceleration *= 3.5;
        }

        if input.key(keys::FORWARD).is_pressed() {
            rigidbody.acceleration +=
                transform_project_xz(&mut transform, nalgebra::vector![0.0, -horiz_acceleration]);
        }
        if input.key(keys::BACKWARD).is_pressed() {
            rigidbody.acceleration +=
                transform_project_xz(&mut transform, nalgebra::vector![0.0, horiz_acceleration]);
        }
        if input.key(keys::RIGHT).is_pressed() {
            rigidbody.acceleration +=
                transform_project_xz(&mut transform, nalgebra::vector![horiz_acceleration, 0.0]);
        }
        if input.key(keys::LEFT).is_pressed() {
            rigidbody.acceleration +=
                transform_project_xz(&mut transform, nalgebra::vector![-horiz_acceleration, 0.0]);
        }
        if input.key(keys::UP).is_pressed() {
            if collider.in_liquid {
                rigidbody.acceleration.y += 60.0;
            } else if collider.on_ground {
                rigidbody.velocity.y = vert_acceleration;
            }
        }

        // 0.96 with horiz_acceleration=30.0 is good for flight or slippery surfaces or
        // such rigidbody.velocity.x *= 0.96;
        // rigidbody.velocity.z *= 0.96;

        let horiz_drag = 0.1;
        rigidbody.velocity.x *= util::lerp(1.0 - horiz_drag, 0.0, time.delta_seconds());
        rigidbody.velocity.z *= util::lerp(1.0 - horiz_drag, 0.0, time.delta_seconds());

        if collider.in_liquid {
            rigidbody.velocity.y *= util::lerp(0.96, 0.0, time.delta_seconds());
        }

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

fn setup_player(mut cmd: Commands) {
    let player = cmd
        .spawn()
        .insert(Transform::default().translated(&nalgebra::vector![0.0, 20.0, 0.0]))
        .insert(AabbCollider::new(Aabb::with_dimensions(nalgebra::vector![
            0.8, 2.0, 0.8
        ])))
        .insert(RigidBody::default())
        .insert(DynamicChunkLoader {
            load_radius: 7,
            unload_radius: 8,
        })
        .id();

    let camera = cmd
        .spawn()
        .insert(Camera::default())
        .insert(Transform::default())
        .insert(TerrainManipulator {
            start_pos: None,
            start_button: None,
        })
        .id();

    cmd.insert_resource(ActiveCamera(Some(camera)));
    cmd.insert_resource(CameraController {
        mode: CameraControllerMode::Follow(player),
        camera,
    });
    cmd.insert_resource(PlayerController { player });
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Default)]
pub struct DefaultPlugins;

impl PluginGroup for DefaultPlugins {
    fn build(&mut self, group: &mut app::PluginGroupBuilder) {
        group.add(CorePlugin);
        group.add(WindowingPlugin::default());
        group.add(InputPlugin::default());
        group.add(WorldPlugin::default());
        group.add(RenderPlugin::default());

        #[cfg(feature = "hot-reload")]
        group.add(engine::loader::HotReloadPlugin::default());
    }
}

#[derive(Debug, Default)]
pub struct WindowingPlugin {}

impl Plugin for WindowingPlugin {
    fn build(&self, app: &mut AppBuilder) {
        let event_loop = EventLoop::new();
        let window = WindowBuilder::new().with_title("Notcraft™");
        let graphics_context = ContextBuilder::new().with_depth_buffer(24).with_vsync(true);
        let display = Rc::new(Display::new(window, graphics_context, &event_loop).unwrap());

        app.insert_non_send_resource(event_loop);
        app.insert_non_send_resource(display);
    }
}

fn glutin_runner(mut app: App) {
    // the runner isn't `FnOnce`, or even `FnMut`, so we can't move the display and
    // event loop into here.
    let event_loop = app.world.remove_non_send::<EventLoop<()>>().unwrap();
    let display = Rc::clone(app.world.get_non_send_resource::<Rc<Display>>().unwrap());

    event_loop.run(move |event, _target, cf| match event {
        Event::WindowEvent {
            event: WindowEvent::CloseRequested,
            ..
        } => {
            // TODO: move close handling code somewhere else mayhaps
            *cf = ControlFlow::Exit;
        }

        // TODO: i should probably set up dedicated event channels for each of these
        Event::DeviceEvent { device_id, event } => {
            if let Some(mut events) = app.world.get_resource_mut::<Events<RawInputEvent>>() {
                events.send(RawInputEvent::Device(device_id, event));
            }
        }

        Event::WindowEvent { window_id, event } => {
            if let Some(mut events) = app.world.get_resource_mut::<Events<RawInputEvent>>() {
                if let Some(event) = event.to_static() {
                    events.send(RawInputEvent::Window(window_id, event));
                }
            }
        }

        Event::MainEventsCleared => display.gl_window().window().request_redraw(),
        Event::RedrawRequested(id) if id == display.gl_window().window().id() => {
            app.update();
            let mut app_exit_events = app.world.get_resource_mut::<Events<AppExit>>().unwrap();
            if app_exit_events.drain().last().is_some() {
                *cf = ControlFlow::Exit;
            }
        }

        _ => {}
    });
}

#[derive(Clone, Debug, StructOpt)]
pub struct RunOptions {
    #[structopt(default_value = "simple", long)]
    pub mesher_mode: MesherMode,
}

fn main() {
    simple_logger::init_with_level(log::Level::Debug).unwrap();

    let options = RunOptions::from_args();

    App::build()
        .add_plugins(DefaultPlugins)
        .add_plugin(ChunkMesherPlugin::default().with_mode(options.mesher_mode))
        .add_plugin(PhysicsPlugin::default())
        .add_plugin(CollisionPlugin::default())
        .add_startup_system(setup_player.system())
        // .add_system(intermittent_music.system())
        .add_system(
            player_look_controller
                .system()
                .label(PlayerControllerUpdate),
        )
        .add_system(player_controller.system().label(PlayerControllerUpdate))
        .add_system(
            camera_controller
                .system()
                .label(CameraControllerUpdate)
                .after(PlayerControllerUpdate),
        )
        .add_system(terrain_manipulation.system().after(CameraControllerUpdate))
        .set_runner(glutin_runner)
        .run();
}

#[derive(Debug, Hash, PartialEq, Eq, Clone, SystemLabel)]
pub struct PlayerControllerUpdate;

#[derive(Debug, Hash, PartialEq, Eq, Clone, SystemLabel)]
pub struct CameraControllerUpdate;
