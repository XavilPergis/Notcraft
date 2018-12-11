#![feature(duration_float, transpose_result)]

#[macro_use]
extern crate log;
#[macro_use]
extern crate specs_derive;
#[macro_use]
extern crate serde_derive;

pub mod engine;
pub mod handle;
pub mod loader;
pub mod util;

use crate::engine::{
    audio::AudioManager,
    camera::Camera,
    components as comp,
    job::Worker,
    render::{
        debug::*,
        mesher::{ChunkMesher, Mesher},
        terrain::*,
        DeferredRenderPass, DeferredRenderPassContext, GraphicsData,
    },
    resources as res,
    systems::*,
    world::{
        block::{BlockRegistry, Faces},
        gen::*,
        ChunkPos, VoxelWorld,
    },
};
use cgmath::{Deg, Point3, Vector3};
use collision::Aabb3;
use glium::{
    backend::Facade,
    index::NoIndices,
    texture::{Cubemap, RawImage2d, Texture2dArray, TextureCreationError},
    PolygonMode, Surface,
};
use image::{DynamicImage, ImageError, RgbaImage};
use shrev::EventChannel;
use specs::prelude::*;
pub use specs::shred;
use std::{
    cell::RefCell,
    collections::HashMap,
    error::Error,
    path::{Path, PathBuf},
    rc::Rc,
    time::Duration,
};
// mod benches {
//     use super::*;
//     use test::Bencher;

//     #[bench]
//     fn bench_mesher(bencher: &mut Bencher) {
//         let (registry, _) =
// BlockRegistry::load_from_file("resources/blocks.json").unwrap();         let
// mut world = VoxelWorld::new(registry);         let mut gen =
// NoiseGenerator::new_default();

//         for x in -1..=1 {
//             for y in -1..=1 {
//                 for z in -1..=1 {
//                     let pos = ChunkPos(Point3::new(x, y, z));
//                     world.set_chunk(pos, gen.compute(&pos));
//                 }
//             }
//         }

//         bencher.iter(|| {
//             let mut mesher = Mesher::new(ChunkPos(Point3::new(0, 0, 0)),
// &world);             mesher.mesh();
//         });
//     }

// }

use glium::{
    framebuffer::SimpleFrameBuffer,
    texture::{CubeLayer, Texture2d},
    uniforms::MagnifySamplerFilter,
};
use std::fs::File;

// oh sweet jesus...
fn load_cubemap<F: Facade>(ctx: &F, name: &str) -> Cubemap {
    static LAYER_NAMES: &[(&str, CubeLayer)] = &[
        ("right", CubeLayer::PositiveX),
        ("left", CubeLayer::NegativeX),
        ("top", CubeLayer::PositiveY),
        ("bottom", CubeLayer::NegativeY),
        ("front", CubeLayer::PositiveZ),
        ("back", CubeLayer::NegativeZ),
    ];

    let blit_target = glium::BlitTarget {
        left: 0,
        bottom: 0,
        width: 1024,
        height: 1024,
    };

    // TODO: don't hardcode p l s
    let cubemap = Cubemap::empty(ctx, 1024).unwrap();

    for &(name, layer) in LAYER_NAMES {
        let path = format!("resources/textures/skybox/{}.tga", name);
        let image = image::open(path).unwrap().to_rgba();
        let dims = image.dimensions();

        let raw = RawImage2d::from_raw_rgba_reversed(&image.into_raw(), dims);
        let texture = Texture2d::new(ctx, raw).unwrap();

        let fbo = SimpleFrameBuffer::new(ctx, cubemap.main_level().image(layer)).unwrap();

        texture
            .as_surface()
            .blit_whole_color_to(&fbo, &blit_target, MagnifySamplerFilter::Nearest);
    }

    cubemap
}

fn main() {
    simple_logger::init_with_level(log::Level::Debug).unwrap();
    let mut events_loop = glium::glutin::EventsLoop::new();
    let window = glium::glutin::WindowBuilder::new()
        .with_title("Hello, world!")
        .with_dimensions(glium::glutin::dpi::LogicalSize::new(1024.0, 768.0));
    let context = glium::glutin::ContextBuilder::new().with_vsync(true);
    let display = glium::Display::new(window, context, &events_loop).unwrap();

    // display.gl_window().grab_cursor(true).unwrap();
    println!("Context created!");

    let mut window_events = shrev::EventChannel::new();

    // let projection = ::cgmath::perspective(Deg(70.0), 600.0 / 600.0, 0.1,
    // 1000.0f32); debug_program.set_uniform(&mut ctx, "projection",
    // &projection);

    let mut world = World::default();

    // world.register::<Handle<::engine::render::terrain::GpuChunkMesh>>();
    world.register::<comp::Transform>();
    world.register::<comp::ClientControlled>();
    world.register::<comp::Player>();
    world.register::<comp::RigidBody>();
    world.register::<comp::ChunkId>();
    world.register::<comp::DirtyMesh>();
    world.register::<comp::Collidable>();

    let (registry, tex_names) = BlockRegistry::load_from_file("resources/blocks.json").unwrap();
    let voxel_world = VoxelWorld::new(registry);

    let player_tfm = comp::Transform::default();
    world
        .create_entity()
        .with(comp::ClientControlled)
        .with(comp::Player)
        .with(player_tfm)
        .with(comp::Collidable {
            aabb: Aabb3::new(Point3::new(-0.4, -1.6, -0.4), Point3::new(0.4, 0.2, 0.4)),
        })
        .with(comp::RigidBody {
            mass: 100.0,
            drag: Vector3::new(3.0, 6.0, 3.0),
            velocity: Vector3::new(0.0, 0.0, 0.0),
        })
        .build();

    let graphics = Rc::new(RefCell::new(GraphicsData::new(&display, tex_names)));

    // let mut mesh_channel = EventChannel::<(ChunkPos, CpuChunkMesh)>::new();

    // let terrain_renderer = TerrainRenderSystem {
    //     ctx: ctx.clone(),
    //     pool,
    //     program,
    //     mesh_recv: mesh_channel.register_reader(),
    // };

    fn duration_as_ms(duration: Duration) -> f64 {
        (duration.as_secs() as f64 + duration.subsec_nanos() as f64 * 1e-9) * 1000.0
    }

    struct TraceSystem<S> {
        inner: S,
        name: &'static str,
        samples: Vec<Duration>,
    }

    impl<S> TraceSystem<S> {
        fn new(inner: S, name: &'static str) -> Self {
            TraceSystem {
                inner,
                name,
                samples: Vec::new(),
            }
        }
    }

    impl<'a, S> System<'a> for TraceSystem<S>
    where
        S: System<'a>,
    {
        type SystemData = S::SystemData;

        fn run(&mut self, data: Self::SystemData) {
            if self.samples.len() >= 100 {
                let len = self.samples.len() as f64;
                let sum: f64 = self.samples.drain(..).map(duration_as_ms).sum();

                debug!(
                    "Timing: Took {} ms on average for system \"{}\" ",
                    sum / len,
                    self.name,
                );
            }

            let before = Instant::now();
            self.inner.run(data);
            let after = Instant::now();

            self.samples.push(after - before);
        }
    }

    fn attach_system<'a, 'b, T>(
        builder: DispatcherBuilder<'a, 'b>,
        sys: T,
        name: &'static str,
        deps: &[&str],
    ) -> DispatcherBuilder<'a, 'b>
    where
        T: for<'c> System<'c> + Send + 'a,
    {
        builder.with(TraceSystem::new(sys, name), name, deps)
    }

    fn attach_system_sync<'a, 'b, T>(
        builder: DispatcherBuilder<'a, 'b>,
        sys: T,
        name: &'static str,
    ) -> DispatcherBuilder<'a, 'b>
    where
        T: for<'c> System<'c> + 'b,
    {
        builder.with_thread_local(TraceSystem::new(sys, name))
    }

    let mut builder = DispatcherBuilder::new();
    builder = attach_system(builder, ChunkUnloader::default(), "chunk unloader", &[]);
    builder = attach_system(builder, AudioManager::new(), "audio manager", &[]);
    builder = attach_system(
        builder,
        CameraRotationUpdater::new(&mut window_events),
        "cursor input handler",
        &[],
    );
    builder = attach_system(builder, PlayerController, "player controller", &[]);
    builder = attach_system(builder, Physics::new(), "physics", &[]);
    builder = attach_system(
        builder,
        BlockInteraction::new(&mut window_events),
        "block interactions",
        &["physics"],
    );
    builder = attach_system(builder, TerrainGenerator::new(), "terrain generator", &[]);

    builder = attach_system_sync(builder, ChunkMesher::new(&graphics), "chunk mesher");
    builder = attach_system_sync(
        builder,
        InputHandler::new(&mut window_events),
        "input handler",
    );

    let mut dispatcher = builder.build();

    dispatcher.setup(&mut world.res);

    world.add_resource(res::ActiveDirections::default());
    world.add_resource(res::StopGameLoop(false));
    world.add_resource(window_events);
    world.add_resource(res::Dt(Duration::from_secs(1)));
    world.add_resource(Camera::default());

    world.add_resource(voxel_world);
    world.add_resource(PolygonMode::Fill);
    world.add_resource(DebugAccumulator::default());

    println!("World set up");

    use crate::engine::world::block::BlockRegistry;
    use std::time::Instant;

    let mut samples = vec![];

    let mut debug_renderer = DebugRenderer::new(&display).unwrap();
    let mut terrain_renderer = TerrainRenderer::new(&display, &graphics).unwrap();

    use glium::framebuffer::*;

    let albedo_map = glium::texture::Texture2d::empty_with_format(
        &display,
        glium::texture::UncompressedFloatFormat::F32F32F32F32,
        glium::texture::MipmapsOption::NoMipmap,
        1024,
        768,
    )
    .unwrap();
    let position_map = glium::texture::Texture2d::empty_with_format(
        &display,
        glium::texture::UncompressedFloatFormat::F32F32F32F32,
        glium::texture::MipmapsOption::NoMipmap,
        1024,
        768,
    )
    .unwrap();
    let normal_map = glium::texture::Texture2d::empty_with_format(
        &display,
        glium::texture::UncompressedFloatFormat::F32F32F32F32,
        glium::texture::MipmapsOption::NoMipmap,
        1024,
        768,
    )
    .unwrap();
    let extra_buffer = glium::texture::Texture2d::empty_with_format(
        &display,
        glium::texture::UncompressedFloatFormat::F32F32F32F32,
        glium::texture::MipmapsOption::NoMipmap,
        1024,
        768,
    )
    .unwrap();

    let depth = glium::texture::DepthTexture2d::empty_with_format(
        &display,
        glium::texture::DepthFormat::F32,
        glium::texture::MipmapsOption::NoMipmap,
        1024,
        768,
    )
    .unwrap();

    let output = &[
        ("positions", &position_map),
        ("normals", &normal_map),
        ("colors", &albedo_map),
        ("extra", &extra_buffer),
    ];

    let mut framebuffer = glium::framebuffer::MultiOutputFrameBuffer::with_depth_buffer(
        &display,
        output.iter().cloned(),
        &depth,
    )
    .unwrap();

    use glium::uniform;

    let quad_buffer = {
        #[derive(Copy, Clone, Debug)]
        struct Vertex {
            pos: [f32; 2],
        }
        glium::implement_vertex!(Vertex, pos);

        let m1 = -1.0;
        let p1 = 1.0;

        glium::VertexBuffer::new(&display, &[
            Vertex { pos: [m1, m1] },
            Vertex { pos: [p1, m1] },
            Vertex { pos: [m1, p1] },
            Vertex { pos: [p1, p1] },
            Vertex { pos: [m1, p1] },
            Vertex { pos: [p1, m1] },
        ])
        .unwrap()
    };

    let composition_program = glium::Program::from_source(
        &display,
        &util::read_file("resources/shaders/compose.vs").unwrap(),
        &util::read_file("resources/shaders/compose.fs").unwrap(),
        None,
    )
    .unwrap();

    let mut time = 0.0f32;

    while !world.res.fetch::<res::StopGameLoop>().0 {
        let frame_start = Instant::now();

        world.exec(
            |mut channel: Write<'_, EventChannel<glium::glutin::Event>>| {
                events_loop.poll_events(|event| channel.single_write(event))
            },
        );

        // Update camera position and aspect ratio
        world.exec(
            |(mut camera, player, client, transforms): (
                Write<'_, Camera>,
                ReadStorage<'_, comp::Player>,
                ReadStorage<'_, comp::ClientControlled>,
                ReadStorage<'_, comp::Transform>,
            )| {
                for (tfm, _, _) in (&transforms, &player, &client).join() {
                    let pos = tfm.position;
                    let aspect = util::aspect_ratio(&display.gl_window()).unwrap();

                    camera.projection.aspect = aspect;
                    camera.position = pos;
                }
            },
        );

        // Update systems and the world.
        world.maintain();
        world.res.insert(res::StopGameLoop(false));
        dispatcher.dispatch(&world.res);

        graphics
            .borrow_mut()
            .update(&display, ChunkPos(Point3::new(0, 0, 0)));

        let mut frame = display.draw();

        frame.clear_color_and_depth((0.729411765, 0.907843137, 0.981568627, 1.0), 1.0);
        framebuffer.clear_color_and_depth((0.0, 0.0, 0.0, 1.0), 1.0);

        // world.exec(
        //     |(mut accum, camera): (Write<'_, DebugAccumulator>, Read<'_, Camera>)| {
        //         debug_renderer.draw(&mut frame, &mut *accum, *camera);
        //     },
        // );

        world.exec(
            |(camera, mode): (Read<'_, Camera>, ReadExpect<'_, PolygonMode>)| {
                let mut context = DeferredRenderPassContext {
                    facade: &display,
                    target: glium::framebuffer::MultiOutputFrameBuffer::with_depth_buffer(
                        &display,
                        output.iter().cloned(),
                        &depth,
                    )
                    .unwrap(),
                    data: &*graphics.borrow(),

                    camera: *camera,
                    polygon_mode: *mode,
                };

                terrain_renderer.draw(&mut context).unwrap();
            },
        );

        world.exec(|camera: Read<'_, Camera>| {
            let eye_pos: [f32; 3] = camera.position.into();
            let sun_dir = [0.5, 1.0, 0.2f32];
            frame
                .draw(
                    &quad_buffer,
                    NoIndices(glium::index::PrimitiveType::TrianglesList),
                    &composition_program,
                    &uniform! {
                        positions: &position_map,
                        normals: &normal_map,
                        colors: &albedo_map,
                        extra: &extra_buffer,
                        eye: eye_pos,
                        sun_dir: sun_dir,
                    },
                    &Default::default(),
                )
                .unwrap();
        });

        let processing_end = Instant::now();

        // Swap the backbuffer
        frame.finish().unwrap();

        let frame_end = Instant::now();
        let dt = frame_end - frame_start;
        *world.write_resource::<res::Dt>() = res::Dt(dt);

        samples.push(processing_end - frame_start);

        if samples.len() >= 100 {
            let len = samples.len() as f64;
            let sum: f64 = samples.drain(..).map(duration_as_ms).sum();

            debug!(
                "Frame took {} ms on average ({} fps)",
                sum / len,
                1000.0 * len / sum
            );
        }

        time += 0.01;
    }
}
