extern crate log;
#[macro_use]
extern crate serde_derive;

pub mod audio_pool;
pub mod client;
pub mod total_float;

use crate::client::{
    camera::{ActiveCamera, Camera},
    input::{keys, DigitalInput, InputPlugin, InputState, RawInputEvent},
    render::{
        mesher::{ChunkMesherPlugin, MesherMode},
        renderer::{add_debug_box, DebugBox, RenderPlugin},
    },
};
use audio_pool::{load_audio, RandomizedAudioPools};
use bevy_app::{AppExit, Events};
use bevy_core::CorePlugin;
use client::{
    audio::{
        ActiveAudioListener, AudioEvent, AudioListener, AudioPlugin, AudioState,
        ParameterizedSource,
    },
    render::renderer::{immediate_draw_box_edges, ImmediateLines, LineCanvas, RenderStage},
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
use notcraft_common::{
    aabb::Aabb,
    physics::{AabbCollider, CollisionPlugin, PhysicsPlugin, RigidBody},
    prelude::*,
    transform::Transform,
    try_system,
    world::{
        self,
        chunk::ChunkAccess,
        registry::{BlockId, AIR_BLOCK},
        trace_ray, BlockPos, DynamicChunkLoader, Ray3, RaycastHit, WorldPlugin,
    },
    Axis, Side,
};
use rand::{
    distributions::{Distribution, Uniform},
    Rng,
};
use std::{
    collections::{HashMap, HashSet},
    rc::Rc,
};
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
    // TODO: certainly not this!!
    block_name: &'static str,
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
            {
                let mut canvas = ctx.lines.start_default();
                draw_selection_box(&mut canvas, start_pos, hit.pos, [1.0, 0.2, 0.2, 0.8]);
            }

            if input.key(DigitalInput::Button(1)).is_falling() {
                iter_blocks_in(start_pos, hit.pos, |pos| {
                    ctx.set_block(pos, AIR_BLOCK);
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

            {
                let mut canvas = ctx.lines.start_default();
                draw_selection_box(&mut canvas, start_pos, end_pos, [0.2, 0.2, 1.0, 0.8]);
            }

            if input.key(DigitalInput::Button(3)).is_falling() {
                let id = ctx.access.registry().lookup(ctx.manip.block_name);
                iter_blocks_in(start_pos, end_pos, |pos| {
                    ctx.set_block(pos, id);
                });
                ctx.manip.start_pos = None;
                ctx.manip.start_button = None;
            }
        }
    } else {
        {
            let mut canvas = ctx.lines.start_default();
            draw_selection_box(&mut canvas, hit.pos, hit.pos, [1.0, 0.2, 0.2, 0.8]);
        }

        if let Some(side) = hit.side {
            let norm = side.normal::<i32>();
            let offset = BlockPos {
                x: hit.pos.x + norm.x,
                y: hit.pos.y + norm.y,
                z: hit.pos.z + norm.z,
            };

            {
                let mut canvas = ctx.lines.start_default();
                draw_selection_box(&mut canvas, offset, offset, [0.2, 0.2, 1.0, 0.8]);
            }
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
    ctx: &mut TerrainManipulationContext,
    input: &InputState,
    axis: Axis,
    from: BlockPos,
    to: BlockPos,
    id: BlockId,
) {
    if from[axis] > to[axis] {
        return;
    }

    let mut max_n = from[axis];
    for n in from[axis]..=to[axis] {
        let pos = replace_axis(from, axis, n);
        if ctx.access.block(pos).map_or(true, |id| {
            ctx.access.registry().get(id).collision_type().is_solid()
        }) {
            break;
        }
        max_n = n;
    }

    {
        let mut canvas = ctx.lines.start_default();
        draw_selection_box(&mut canvas, from, replace_axis(from, axis, max_n), [
            0.2, 0.2, 1.0, 0.8,
        ]);
    }

    if input.key(DigitalInput::Button(3)).is_rising() {
        for n in from[axis]..=max_n {
            ctx.set_block(replace_axis(from, axis, n), id);
        }
    }
}

fn build_to_me_negative(
    ctx: &mut TerrainManipulationContext,
    input: &InputState,
    axis: Axis,
    from: BlockPos,
    to: BlockPos,
    id: BlockId,
) {
    if from[axis] < to[axis] {
        return;
    }

    let mut min_n = from[axis];
    for n in (to[axis]..=from[axis]).rev() {
        let pos = replace_axis(from, axis, n);
        if ctx.access.block(pos).map_or(true, |id| {
            ctx.access.registry().get(id).collision_type().is_solid()
        }) {
            break;
        }
        min_n = n;
    }

    {
        let mut canvas = ctx.lines.start_default();
        draw_selection_box(&mut canvas, from, replace_axis(from, axis, min_n), [
            0.2, 0.2, 1.0, 0.8,
        ]);
    }

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
    let id = ctx.access.registry().lookup(ctx.manip.block_name);
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
    {
        let mut canvas = ctx.lines.start_default();
        draw_selection_box(&mut canvas, hit.pos, hit.pos, [1.0, 0.2, 0.2, 0.8]);
    }

    if input.key(DigitalInput::Button(1)).is_rising() {
        ctx.set_block(hit.pos, AIR_BLOCK);
    }

    if let Some(side) = hit.side {
        let norm = side.normal::<i32>();
        let offset = BlockPos {
            x: hit.pos.x + norm.x,
            y: hit.pos.y + norm.y,
            z: hit.pos.z + norm.z,
        };

        {
            let mut canvas = ctx.lines.start_default();
            draw_selection_box(&mut canvas, offset, offset, [0.2, 0.2, 1.0, 0.8]);
        }

        if input.key(DigitalInput::Button(3)).is_rising() {
            let id = ctx.access.registry().lookup(ctx.manip.block_name);
            ctx.set_block(offset, id);
        }
    }
}

fn draw_selection_box(canvas: &mut LineCanvas, start: BlockPos, end: BlockPos, color: [f32; 4]) {
    let aabb = box_enclosing(start, end);

    // draw outer border
    canvas.color(color).width(2.0);
    immediate_draw_box_edges(canvas, &aabb);

    canvas.color([color[0], color[1], color[2], color[3] / 3.0]);
    canvas.width(1.0);
    if start.x != end.x {
        let min_x = i32::min(start.x, end.x);
        let max_x = i32::max(start.x, end.x);

        let ny = aabb.min.y;
        let py = aabb.max.y;
        let nz = aabb.min.z;
        let pz = aabb.max.z;
        for x in min_x..max_x {
            let x = x + 1;
            let nn = vector![x as f32, ny, nz];
            let np = vector![x as f32, ny, pz];
            let pn = vector![x as f32, py, nz];
            let pp = vector![x as f32, py, pz];
            canvas.goto(nn).line(np).line(pp).line(pn).line(nn);
        }
    }

    if start.y != end.y {
        let min_y = i32::min(start.y, end.y);
        let max_y = i32::max(start.y, end.y);

        let nx = aabb.min.x;
        let px = aabb.max.x;
        let nz = aabb.min.z;
        let pz = aabb.max.z;
        for y in min_y..max_y {
            let y = y + 1;
            let nn = vector![nx, y as f32, nz];
            let np = vector![nx, y as f32, pz];
            let pn = vector![px, y as f32, nz];
            let pp = vector![px, y as f32, pz];
            canvas.goto(nn).line(np).line(pp).line(pn).line(nn);
        }
    }

    if start.z != end.z {
        let min_z = i32::min(start.z, end.z);
        let max_z = i32::max(start.z, end.z);

        let nx = aabb.min.x;
        let px = aabb.max.x;
        let ny = aabb.min.y;
        let py = aabb.max.y;
        for z in min_z..max_z {
            let z = z + 1;
            let nn = vector![nx, ny, z as f32];
            let np = vector![nx, py, z as f32];
            let pn = vector![px, ny, z as f32];
            let pp = vector![px, py, z as f32];
            canvas.goto(nn).line(np).line(pp).line(pn).line(nn);
        }
    }
}

struct TerrainManipulationContext<'a> {
    access: &'a mut ChunkAccess,
    manip: &'a mut TerrainManipulator,
    transform: &'a Transform,
    // collider: &'a AabbCollider,
    broken_blocks: &'a mut HashMap<BlockId, HashSet<BlockPos>>,
    lines: &'a mut ImmediateLines,
}

impl<'a> TerrainManipulationContext<'a> {
    fn set_block(&mut self, pos: BlockPos, id: BlockId) {
        if let Some(prev) = self.access.block(pos) {
            if id == AIR_BLOCK && id != prev {
                self.broken_blocks.entry(prev).or_default().insert(pos);
            }
            // TODO: prevent placing blocks that would collide with any entity colliders
            self.access.set_block(pos, id);
        }
    }
}

fn terrain_manipulation(
    input: Res<InputState>,
    mut access: ResMut<ChunkAccess>,
    mut query: Query<(
        &Transform,
        // &AabbCollider,
        &mut TerrainManipulator,
    )>,
    mut lines: ResMut<ImmediateLines>,
    mut audio_events: EventWriter<AudioEvent>,
    audio_pools: Res<RandomizedAudioPools>,
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

    let mut broken_blocks = HashMap::default();
    query.for_each_mut(|(transform, mut manip)| {
        if input.key(VirtualKeyCode::Q).is_rising() {
            manip.block_name = match manip.block_name {
                "debug_glow_block" => "stone",
                _ => "debug_glow_block",
            };

            log::info!("switched block to {}", manip.block_name);
        }

        if let Some(hit) = trace_ray(&mut access, make_ray(transform, &-Vector3::z()), 100.0) {
            let mut ctx = TerrainManipulationContext {
                access: &mut access,
                manip: &mut manip,
                transform,
                broken_blocks: &mut broken_blocks,
                lines: &mut lines,
            };

            if input.key(VirtualKeyCode::E).is_rising() {
                for x in hit.pos.x - 10..hit.pos.x + 10 {
                    for y in hit.pos.y - 10..hit.pos.y + 10 {
                        for z in hit.pos.z - 10..hit.pos.z + 10 {
                            let dx = hit.pos.x - x;
                            let dy = hit.pos.y - y;
                            let dz = hit.pos.z - z;
                            if dx * dx + dy * dy + dz * dz < 100 {
                                ctx.set_block(BlockPos { x, y, z }, AIR_BLOCK);
                            }
                        }
                    }
                }
            }

            if ctx.manip.start_pos.is_some() || (input.ctrl() && input.shift()) {
                terrain_manipulation_area(&input, &hit, &mut ctx);
            } else if ctx.manip.start_pos.is_none() && input.ctrl() {
                terrain_manipulation_build_to_me(&input, &hit, &mut ctx);
            } else if ctx.manip.start_pos.is_none() {
                terrain_manipulation_single(&input, &hit, &mut ctx);
            }
        }
    });

    if let Some(transform) = query.iter_mut().next().map(|(t, _)| t) {
        // how many sounds of the same type can be playing at once
        const SOUND_TYPE_LIMIT: usize = 3;
        const MAX_AMPLITUDE: f32 = 6.0;

        #[derive(Debug)]
        struct SoundEntry {
            amplitude: f32,
            count: usize,
        }

        let mut sounds = HashMap::new();
        let mut near = None;
        let mut far = None;
        let mut near_dist = f32::MAX;
        let mut far_dist = f32::MIN;
        for (&id, positions) in broken_blocks.iter() {
            let block_name = format!("blocks/break/{}", access.registry().name(id));
            if let Some(sound_id) = audio_pools.id(&block_name) {
                assert!(!positions.is_empty());
                let count = positions.len().min(SOUND_TYPE_LIMIT);
                let amplitude = (positions.len() as f32) / (count as f32);

                sounds.insert(sound_id, SoundEntry { amplitude, count });

                for pos in positions.iter() {
                    let dx = transform.pos().x - pos.x as f32 + 0.5;
                    let dy = transform.pos().y - pos.y as f32 + 0.5;
                    let dz = transform.pos().z - pos.z as f32 + 0.5;
                    let dist_sq = dx * dx + dy * dy + dz * dz;

                    if dist_sq < near_dist {
                        near_dist = dist_sq;
                        near = Some(pos);
                    }

                    if dist_sq > far_dist {
                        far_dist = dist_sq;
                        far = Some(pos);
                    }
                }
            }
        }

        let mut rng = rand::thread_rng();
        for (id, entry) in sounds.into_iter() {
            let start = Point3::from(near.unwrap().origin());
            let end = Point3::from(far.unwrap().origin());

            let step = match entry.count {
                0 => unreachable!(),
                1 => vector![0.0, 0.0, 0.0], // could be anything
                n => (end - start) / (n as f32 - 1.0),
            };

            let mut pos = start;
            for _ in 0..entry.count {
                pos += step;

                audio_pools.select(&mut rng, id, |id, mut params| {
                    // 1.0 / (1.0 + f32::exp(-));
                    let curved = entry.amplitude.sqrt().min(MAX_AMPLITUDE);
                    params.min_amplitude *= curved;
                    params.max_amplitude *= curved;
                    let source = ParameterizedSource::from_sample(id).with_parameters(params);
                    audio_events.send(AudioEvent::SpawnSpatial(pos, source));
                });
            }
        }
    }
}

fn player_look_first_person(
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
        transform.rotation.pitch = util::clamp(-PI / 2.0, PI / 2.0, transform.rotation.pitch);
    }
}

fn player_controller(
    time: Res<Time>,
    input: Res<InputState>,
    player_controller: ResMut<PlayerController>,
    camera_controller: Res<CameraController>,
    mut player_query: Query<(&mut Transform, &mut RigidBody, &AabbCollider)>,
) {
    if input
        .key(VirtualKeyCode::C)
        .require_modifiers(ModifiersState::CTRL)
        .is_rising()
    {
        let grabbed = input.is_cursor_grabbed();
        input.grab_cursor(!grabbed);
        input.hide_cursor(!grabbed);
    }

    if matches!(camera_controller.mode, CameraControllerMode::Static) {
        return;
    }

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
            0.7, 1.7, 0.7
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
        .insert(AudioListener::default())
        .insert(TerrainManipulator {
            start_pos: None,
            start_button: None,
            block_name: "debug_glow_block",
        })
        .id();

    cmd.insert_resource(ActiveCamera(Some(camera)));
    cmd.insert_resource(ActiveAudioListener(Some(camera)));
    cmd.insert_resource(CameraController {
        mode: CameraControllerMode::Follow(player),
        camera,
    });
    cmd.insert_resource(PlayerController { player });
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Default)]
pub struct DefaultPlugins;

impl PluginGroup for DefaultPlugins {
    fn build(&mut self, group: &mut bevy_app::PluginGroupBuilder) {
        group.add(CorePlugin);
        group.add(WindowingPlugin::default());
        group.add(InputPlugin::default());
        group.add(WorldPlugin::default());
        group.add(RenderPlugin::default());
        group.add(AudioPlugin::default());

        #[cfg(feature = "hot-reload")]
        group.add(client::loader::HotReloadPlugin::default());
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

    #[structopt(long, short = "D")]
    pub enable_debug_events: Option<Vec<String>>,
}

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
struct WeightedList<T> {
    items: Vec<(usize, T)>,
    total_weight: usize,
}

impl<T> Default for WeightedList<T> {
    fn default() -> Self {
        Self {
            items: Default::default(),
            total_weight: Default::default(),
        }
    }
}

impl<T> WeightedList<T> {
    pub fn push(&mut self, weight: usize, value: T) {
        self.items.push((self.total_weight, value));
        self.total_weight += weight;
    }

    pub fn select<'a, R>(&'a self, rng: &mut R) -> Option<&'a T>
    where
        R: Rng + ?Sized,
    {
        if self.items.is_empty() {
            return None;
        }

        let num = Uniform::new_inclusive(0, self.total_weight - 1).sample(rng);
        Some(match self.items.binary_search_by_key(&num, |&(w, _)| w) {
            // we use the straight index here because our lower bound as described in the comment
            // below is inclusive.
            Ok(idx) => &self.items[idx].1,

            // just using the straight index here would cause a "rounding up" sort of behavior, eg:
            // given a weighted list [(0, A), (10, B)] and selected number 3, B would be selected,
            // but we want A to be. you might think of this as each entry representing a start
            // number and the next entry as defining an end number, defining the range [start, end)
            // as mapping to the value in the start node.
            //
            // also note that unconditionally subtracting here is fine, since the first item of the
            // item list always has a number of 0, which is the lowest value the generated number
            // will be, meaning the `Ok` case will always be selected when the generated number is
            // 0.
            Err(idx) => &self.items[idx - 1].1,
        })
    }
}

fn load_sounds(mut cmd: Commands, mut state: ResMut<AudioState>) -> Result<()> {
    let pools = load_audio("resources/audio/manifest.ron", &mut *state)?;
    cmd.insert_resource(pools);

    Ok(())
}

fn main() {
    env_logger::init();

    let options = RunOptions::from_args();

    if let Some(enabled) = options
        .enable_debug_events
        .map(|names| names.into_iter().collect::<HashSet<_>>())
    {
        let enabled = match enabled.is_empty() {
            true => None,
            false => Some(enabled),
        };
        println!("enabled debug events: {:?}", enabled);

        world::debug::events::enumerate(enabled.as_ref());
        client::debug::events::enumerate(enabled.as_ref());
    }

    App::build()
        .add_plugins(DefaultPlugins)
        .add_plugin(ChunkMesherPlugin::default().with_mode(options.mesher_mode))
        .add_plugin(PhysicsPlugin::default())
        .add_plugin(CollisionPlugin::default())
        .add_startup_system(setup_player.system())
        .add_startup_system(try_system!(load_sounds))
        .add_system(
            player_look_first_person
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
        .add_system_to_stage(
            RenderStage::PreRender,
            client::debug::debug_event_handler.system(),
        )
        .add_system_to_stage(
            CoreStage::Last,
            notcraft_common::debug::clear_debug_events.exclusive_system(),
        )
        .set_runner(glutin_runner)
        .run();
}

#[derive(Debug, Hash, PartialEq, Eq, Clone, SystemLabel)]
pub struct PlayerControllerUpdate;

#[derive(Debug, Hash, PartialEq, Eq, Clone, SystemLabel)]
pub struct CameraControllerUpdate;
