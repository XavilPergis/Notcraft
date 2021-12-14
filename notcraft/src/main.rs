#[macro_use]
extern crate log;
#[macro_use]
extern crate serde_derive;

pub mod engine;
pub mod util;

#[derive(Copy, Clone, Debug, PartialEq, Default)]
pub struct Player;

use crate::engine::{
    input::{keys, InputState},
    render::{
        camera::{ActiveCamera, Camera},
        renderer::Renderer,
    },
    world::VoxelWorld,
};
use engine::{
    render::mesher::MesherContext,
    transform::{Parent, Transform},
    world::{registry::BlockRegistry, ChunkLoader},
    Dt, StopGameLoop,
};
use glium::{
    glutin::{
        event::{Event, KeyboardInput, VirtualKeyCode, WindowEvent},
        event_loop::{ControlFlow, EventLoop},
        window::WindowBuilder,
        ContextBuilder,
    },
    Display,
};
use legion::{systems::CommandBuffer, *};
use std::{
    rc::Rc,
    sync::Arc,
    time::{Duration, Instant},
};

#[legion::system(for_each)]
fn player_movement(
    #[resource] input: &InputState,
    #[resource] Dt(dt): &Dt,
    transform: &mut Transform,
    _player: &Player,
) {
    use std::f32::consts::PI;
    let pitch = input.cursor_delta().y * (PI / 180.0);
    let yaw = input.cursor_delta().x * (PI / 180.0);

    transform.rotation.x -= pitch;
    transform.rotation.x = f32::min(transform.rotation.x, PI / 2.0);
    transform.rotation.x = f32::max(transform.rotation.x, -PI / 2.0);

    transform.rotation.y -= yaw;

    let speed = 100.0 * dt.as_secs_f32();

    if input.key(keys::FORWARD).is_pressed() {
        engine::transform::creative_flight(transform, nalgebra::vector!(0.0, -speed));
    }
    if input.key(keys::BACKWARD).is_pressed() {
        engine::transform::creative_flight(transform, nalgebra::vector!(0.0, speed));
    }
    if input.key(keys::RIGHT).is_pressed() {
        engine::transform::creative_flight(transform, nalgebra::vector!(speed, 0.0));
    }
    if input.key(keys::LEFT).is_pressed() {
        engine::transform::creative_flight(transform, nalgebra::vector!(-speed, 0.0));
    }
    if input.key(keys::UP).is_pressed() {
        transform.translate_global(&nalgebra::vector!(0.0, speed, 0.0).into());
    }
    if input.key(keys::DOWN).is_pressed() {
        transform.translate_global(&nalgebra::vector!(0.0, -speed, 0.0).into());
    }

    if input.key(VirtualKeyCode::Tab).is_rising() {
        let grabbed = input.is_cursor_grabbed();

        input.grab_cursor(!grabbed);
        input.hide_cursor(!grabbed);
    }
}

fn setup_world(cmd: &mut CommandBuffer) {
    let player = cmd.push((Transform::default(), Player, ChunkLoader { radius: 7 }));

    let camera_entity = cmd.push((Parent(player), Camera::default(), Transform::default()));

    cmd.exec_mut(move |_, res| {
        res.insert(ActiveCamera(Some(camera_entity)));

        res.insert(engine::input::InputState::default());
        res.insert(StopGameLoop(false));
        res.insert(Dt(Duration::from_secs(1)));
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
    }

    let event_loop = EventLoop::new();
    let (window_events_tx, window_events_rx) = crossbeam_channel::unbounded();

    let window = WindowBuilder::new().with_title("Notcraft™");
    let ctx = ContextBuilder::new().with_depth_buffer(24);

    let display = Rc::new(Display::new(window, ctx, &event_loop).unwrap());

    let renderer = Renderer::new(
        Rc::clone(&display),
        Arc::clone(&registry),
        &mut world,
        &mut resources,
    )
    .unwrap();

    let schedule = Schedule::builder()
        .add_thread_local(engine::input::input_compiler_system(
            window_events_rx,
            Rc::clone(&display),
        ))
        .add_thread_local(engine::audio::intermittent_music_system(
            engine::audio::MusicState::new(),
        ))
        .add_system(engine::render::mesher::chunk_mesher_system(mesher_ctx))
        .add_system(engine::world::update_world_system())
        .add_system(engine::world::gen::update_surface_cache_system())
        .flush()
        .add_system(player_movement_system())
        .add_system(engine::world::load_chunks_system(
            engine::world::ChunkLoaderContext::new(&mut world),
        ))
        .flush()
        .add_system(engine::transform::transform_hierarchy_system())
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
                        ..
                    },
                ..
            } => {
                *cf = ControlFlow::Exit;
            }

            _ => (),
        },

        Event::DeviceEvent { device_id, event } => {
            window_events_tx.send((device_id, event)).unwrap()
        }

        Event::MainEventsCleared => display.gl_window().window().request_redraw(),
        Event::RedrawRequested(id) if id == display.gl_window().window().id() => {
            run_frame(&mut main_ctx);
        }
        _ => {}
    });

    // while !resources
    //     .get::<res::StopGameLoop>()
    //     .map_or(true, |val| val.0)
    // {

    // }
}
