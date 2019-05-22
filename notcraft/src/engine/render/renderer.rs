use crate::engine::{
    components::GlobalTransform,
    loader,
    prelude::*,
    render::{
        camera::{ActiveCamera, Camera},
        mesher::TerrainMesh,
        Ao, Norm, Pos, Tang, Tex, TexId,
    },
    world::VoxelWorld,
};
use glium::{
    backend::Facade,
    framebuffer::SimpleFrameBuffer,
    glutin::EventsLoop,
    index::{IndexBuffer, PrimitiveType},
    texture::{
        DepthTexture2d, MipmapsOption, RawImage2d, SrgbTexture2dArray, Texture2d, Texture2dArray,
        TextureCreationError, UncompressedFloatFormat,
    },
    uniform,
    uniforms::UniformBuffer,
    vertex::VertexBuffer,
    Display, Program, Surface,
};
use nalgebra::{self as na, Perspective3};
use specs::{prelude::*, world::Index};
use std::{collections::HashMap, rc::Rc};

/// Map error to a string
macro_rules! err2s {
    ($e:expr) => {
        $e.map_err(|err| format!("{:?}", err))
    };
}

struct PipelineBuffers {
    gdepth: DepthTexture2d,
    // RGB -> XYZ
    gposition: Texture2d,
    // RGB
    gcolor: Texture2d,
    // RGB -> XYZ
    gnormal: Texture2d,
    // RGB
    gemissive: Texture2d,
    // R -> Roughness, G -> Metallic
    gextra: Texture2d,

    // Shadow map
    shadow: DepthTexture2d,
    // Ao map
    ao: Texture2d,

    // TODO: post-processing n stuff
    composite: Texture2d,
}

impl PipelineBuffers {
    fn new<F: Facade>(ctx: &F, width: u32, height: u32) -> Result<Self, TextureCreationError> {
        Ok(PipelineBuffers {
            gdepth: DepthTexture2d::empty(ctx, width, height)?,
            gposition: Texture2d::empty_with_format(
                ctx,
                UncompressedFloatFormat::F32F32F32,
                MipmapsOption::NoMipmap,
                width,
                height,
            )?,
            gcolor: Texture2d::empty_with_format(
                ctx,
                UncompressedFloatFormat::F32F32F32,
                MipmapsOption::NoMipmap,
                width,
                height,
            )?,
            gnormal: Texture2d::empty_with_format(
                ctx,
                UncompressedFloatFormat::F32F32F32,
                MipmapsOption::NoMipmap,
                width,
                height,
            )?,
            gemissive: Texture2d::empty_with_format(
                ctx,
                UncompressedFloatFormat::F32F32F32,
                MipmapsOption::NoMipmap,
                width,
                height,
            )?,
            gextra: Texture2d::empty_with_format(
                ctx,
                UncompressedFloatFormat::F32F32F32,
                MipmapsOption::NoMipmap,
                width,
                height,
            )?,

            shadow: DepthTexture2d::empty(ctx, width, height)?,
            ao: Texture2d::empty_with_format(
                ctx,
                UncompressedFloatFormat::F32,
                MipmapsOption::NoMipmap,
                width,
                height,
            )?,

            composite: Texture2d::empty_with_format(
                ctx,
                UncompressedFloatFormat::F32F32F32,
                MipmapsOption::NoMipmap,
                width,
                height,
            )?,
        })
    }
}

use glium::framebuffer::{MultiOutputFrameBuffer, ValidationError};

fn get_deferred_render_target<'a, F: Facade>(
    ctx: &F,
    buffers: &'a PipelineBuffers,
) -> Result<MultiOutputFrameBuffer<'a>, ValidationError> {
    MultiOutputFrameBuffer::with_depth_buffer(
        ctx,
        [
            ("gposition", &buffers.gposition),
            ("gcolor", &buffers.gcolor),
            ("gnormal", &buffers.gnormal),
            ("gemissive", &buffers.gemissive),
            ("gextra", &buffers.gextra),
        ]
        .iter()
        .cloned(),
        &buffers.gdepth,
    )
}

fn get_ao_render_target<'a, F: Facade>(
    ctx: &F,
    buffers: &'a PipelineBuffers,
) -> Result<SimpleFrameBuffer<'a>, ValidationError> {
    SimpleFrameBuffer::new(ctx, &buffers.ao)
}

const AO_KERNEL_SIZE: usize = 64;
const NUM_LIGHTS: usize = 64;

pub struct Renderer {
    display: Rc<Display>,
    terrain_render: TerrainRenderContext,
    buffers: PipelineBuffers,

    compose_program: Program,
    ao_program: Program,
    fullscreen_quad: VertexBuffer<Tex>,
    ao_kernel: UniformBuffer<[[f32; 4]; AO_KERNEL_SIZE]>,

    light_positions: UniformBuffer<[[f32; 4]; NUM_LIGHTS]>,
    light_colors: UniformBuffer<[[f32; 4]; NUM_LIGHTS]>,
}

impl Renderer {
    pub fn new(event_loop: &EventsLoop, world: &mut World) -> Result<Self, String> {
        let window = glium::glutin::WindowBuilder::new()
            .with_title("Notcraft™")
            .with_dimensions(glium::glutin::dpi::LogicalSize::new(1024.0, 768.0));
        let ctx = glium::glutin::ContextBuilder::new().with_depth_buffer(24);
        let display = Rc::new(
            Display::new(window, ctx, event_loop)
                .map_err(|err| format!("could not create Display: {:?}", err))?,
        );

        let (width, height) = display.get_framebuffer_dimensions();
        let buffers =
            PipelineBuffers::new(&*display, width, height).map_err(|err| format!("{:?}", err))?;
        let terrain_render = TerrainRenderContext::new(display.clone(), world)?;
        let compose_program = err2s!(loader::load_shader(&*display, "resources/shaders/compose"))?;
        let ao_program = err2s!(loader::load_shader(&*display, "resources/shaders/ssao"))?;
        let fullscreen_quad = err2s!(VertexBuffer::immutable(&*display, &[
            Tex { uv: [-1.0, 1.0] },
            Tex { uv: [1.0, 1.0] },
            Tex { uv: [-1.0, -1.0] },
            Tex { uv: [1.0, 1.0] },
            Tex { uv: [-1.0, -1.0] },
            Tex { uv: [1.0, -1.0] },
        ]))?;

        use rand::Rng;

        let mut ao_samples = [[0.0f32; 4]; AO_KERNEL_SIZE];
        let mut rng = rand::thread_rng();
        let mut rand = |low| rng.gen_range(low, 1.0f32);
        for i in 0..AO_KERNEL_SIZE {
            let sample =
                rand(0.0) * Vector4::new(rand(-1.0), rand(-1.0), rand(0.0), 0.0).normalize();
            ao_samples[i] = sample.into();
        }

        let ao_kernel = err2s!(UniformBuffer::new(&*display, ao_samples))?;

        let mut light_positions = [[0.0; 4]; NUM_LIGHTS];
        let mut light_colors = [[0.0; 4]; NUM_LIGHTS];

        let mut rng = rand::thread_rng();
        for i in 0..NUM_LIGHTS {
            light_positions[i] = [
                rng.gen_range(-64.0, 64.0),
                rng.gen_range(10.0, 20.0),
                rng.gen_range(-64.0, 64.0),
                1.0,
            ];
            light_colors[i] = [
                rng.gen_range(0.0, 1.0),
                rng.gen_range(0.0, 1.0),
                rng.gen_range(0.0, 1.0),
                1.0,
            ];
        }

        let light_positions = err2s!(UniformBuffer::dynamic(&*display, light_positions))?;
        let light_colors = err2s!(UniformBuffer::dynamic(&*display, light_colors))?;
        // ao_kernel.write(&ao_samples);

        Ok(Renderer {
            display,
            terrain_render,
            buffers,
            compose_program,
            fullscreen_quad,
            ao_kernel,
            ao_program,
            light_positions,
            light_colors,
        })
    }

    pub fn draw(&mut self, world: &mut World) -> Result<(), String> {
        // Upload/delete etc meshes to keep in sync with the world.
        self.terrain_render
            .update_meshes(&*self.display, world)
            .unwrap();

        // Render terrain to gbuffers
        {
            let mut target = err2s!(get_deferred_render_target(&*self.display, &self.buffers))?;
            target.clear_color_and_depth((0.0, 0.0, 0.0, 1.0), 1.0);
            self.terrain_render.draw(&mut target, world)?;
        }

        let cameras = world.read_storage::<Camera>();
        let transforms = world.read_storage::<comp::GlobalTransform>();
        let cam_transform = get_camera(
            world.read_resource::<ActiveCamera>().0,
            &cameras,
            &transforms,
        );
        let (width, height) = self.display.get_framebuffer_dimensions();
        let (view, proj) = get_proj_view(width, height, cam_transform);
        let cam_pos = get_cam_pos(cam_transform);

        // AO render pass
        {
            let mut target = err2s!(get_ao_render_target(&*self.display, &self.buffers))?;
            target.clear_color(1.0, 1.0, 1.0, 1.0);
            err2s!(target.draw(
                &self.fullscreen_quad,
                glium::index::NoIndices(glium::index::PrimitiveType::TrianglesList),
                &self.ao_program,
                &uniform! {
                    gdepth: self.buffers.gdepth.sampled(),
                    gnormal: self.buffers.gnormal.sampled(),
                    gposition: self.buffers.gposition.sampled(),

                    samples: &self.ao_kernel,
                    projection_matrix: array4x4(proj.to_homogeneous()),
                    view_matrix: array4x4(view),
                },
                &Default::default(),
            ))?;
        }

        // let slice = unsafe {
        //     self.light_positions
        //         .slice_custom_mut(|contents| &contents[..])
        // };

        // slice
        //     .map_write()
        //     .set(0, [cam_pos.x, cam_pos.y, cam_pos.z, 1.0]);

        // Compose
        let mut frame = self.display.draw();
        frame.clear_color(0.1, 0.3, 0.3, 1.0);
        err2s!(frame.draw(
            &self.fullscreen_quad,
            glium::index::NoIndices(glium::index::PrimitiveType::TrianglesList),
            &self.compose_program,
            &uniform! {
                gcolor: self.buffers.gcolor.sampled(),
                gposition: self.buffers.gposition.sampled(),
                gnormal: self.buffers.gnormal.sampled(),
                gemissive: self.buffers.gemissive.sampled(),
                gdepth: self.buffers.gdepth.sampled(),
                gextra: self.buffers.gextra.sampled(),
                ao: self.buffers.ao.sampled(),

                light_position_buffer: &self.light_positions,
                light_color_buffer: &self.light_colors,

                camera_pos: array3(cam_pos),
                projection_matrix: array4x4(proj.to_homogeneous()),
                view_matrix: array4x4(view),
            },
            &Default::default(),
        ))?;
        err2s!(frame.finish())?;
        Ok(())
    }
}

#[derive(Debug)]
struct TerrainBuffers {
    // TODO: use u16 when we can
    index: IndexBuffer<u32>,
    pos: VertexBuffer<Pos>,
    tex: VertexBuffer<Tex>,
    norm: VertexBuffer<Norm>,
    tang: VertexBuffer<Tang>,
    ao: VertexBuffer<Ao>,
    tex_id: VertexBuffer<TexId>,
}

impl TerrainBuffers {
    fn immutable<F: Facade>(ctx: &F, mesh: &TerrainMesh) -> Result<Self, String> {
        Ok(TerrainBuffers {
            index: err2s!(IndexBuffer::immutable(
                ctx,
                PrimitiveType::TrianglesList,
                &mesh.index
            ))?,
            pos: err2s!(VertexBuffer::immutable(ctx, &mesh.pos))?,
            tex: err2s!(VertexBuffer::immutable(ctx, &mesh.tex))?,
            norm: err2s!(VertexBuffer::immutable(ctx, &mesh.norm))?,
            tang: err2s!(VertexBuffer::immutable(ctx, &mesh.tang))?,
            ao: err2s!(VertexBuffer::immutable(ctx, &mesh.ao))?,
            tex_id: err2s!(VertexBuffer::immutable(ctx, &mesh.id))?,
        })
    }

    fn glium_verts(&self) -> impl glium::vertex::MultiVerticesSource<'_> {
        (
            &self.pos,
            &self.tex,
            &self.norm,
            &self.tang,
            &self.ao,
            &self.tex_id,
        )
    }
}

fn get_camera<'a>(
    active: Option<Entity>,
    cameras: &'a ReadStorage<'_, Camera>,
    transforms: &'a ReadStorage<'_, GlobalTransform>,
) -> Option<(&'a Camera, &'a GlobalTransform)> {
    active
        .and_then(move |entity| {
            let camera = cameras.get(entity);
            let global = transforms.get(entity);
            camera.and_then(|cam| global.map(|comp| (cam, comp)))
        })
        .or_else(move || (cameras, transforms).join().next())
}

fn get_proj_view(
    width: u32,
    height: u32,
    cam_transform: Option<(&Camera, &GlobalTransform)>,
) -> (Matrix4<f32>, Perspective3<f32>) {
    let (view, mut proj) = cam_transform
        .map(|(cam, transform)| (transform.0.view_matrix(), cam.projection))
        .unwrap_or_else(|| (na::Matrix4::identity(), Camera::default().projection));
    proj.set_aspect(width as f32 / height as f32);
    (view, proj)
}

fn get_cam_pos(cam_transform: Option<(&Camera, &GlobalTransform)>) -> Vector3<f32> {
    match cam_transform {
        Some((_, transform)) => transform.0.translation.vector,
        None => Vector3::new(0.0, 0.0, 0.0),
    }
}

struct TerrainRenderContext {
    // Meshes that were added this frame
    new_meshes: BitSet,
    // Mask for all current meshes
    current_meshes: BitSet,
    // storage of actual vertex/index data
    meshes: HashMap<Index, TerrainBuffers>,
    mesh_reader: ReaderId<ComponentEvent>,

    albedo: SrgbTexture2dArray,
    normal: Texture2dArray,
    extra: Texture2dArray,

    program: Program,
    display: Rc<Display>,
}

impl TerrainRenderContext {
    fn new(display: Rc<Display>, world: &mut World) -> Result<Self, String> {
        let mesh_reader = world.write_storage::<TerrainMesh>().register_reader();

        let program = err2s!(loader::load_shader(&*display, "resources/shaders/terrain"))?;

        let voxel_world = world.read_resource::<VoxelWorld>();
        let paths = voxel_world.registry.texture_paths();
        let (width, height, maps) = err2s!(loader::load_block_textures(
            "resources/textures/blocks",
            paths.iter().map(|s| &**s)
        ))?;
        let dims = (width, height);

        log::debug!("Texture dimensions: {:?}", dims);

        let mut albedo = vec![];
        let mut normal = vec![];
        let mut extra = vec![];

        for map in maps {
            assert!(map.albedo.dimensions() == dims);
            albedo.push(RawImage2d::from_raw_rgba_reversed(
                &map.albedo.into_raw(),
                dims,
            ));

            let normal_map = if let Some(norm) = map.normal {
                norm
            } else {
                image::ImageBuffer::from_pixel(width, height, image::Rgb {
                    data: [127u8, 127, 255],
                })
            };
            assert!(normal_map.dimensions() == dims);
            normal.push(RawImage2d::from_raw_rgb_reversed(&normal_map, dims));

            let extra_map = if let Some(extra) = map.extra {
                extra
            } else {
                image::ImageBuffer::from_pixel(width, height, image::Rgb { data: [0u8, 0, 0] })
            };
            assert!(extra_map.dimensions() == dims);
            extra.push(RawImage2d::from_raw_rgb_reversed(&extra_map, dims));
        }

        let albedo = err2s!(SrgbTexture2dArray::with_mipmaps(
            &*display,
            albedo,
            MipmapsOption::NoMipmap
        ))?;
        let normal = err2s!(Texture2dArray::with_mipmaps(
            &*display,
            normal,
            MipmapsOption::NoMipmap
        ))?;
        let extra = err2s!(Texture2dArray::with_mipmaps(
            &*display,
            extra,
            MipmapsOption::NoMipmap
        ))?;

        Ok(TerrainRenderContext {
            new_meshes: Default::default(),
            current_meshes: Default::default(),
            meshes: Default::default(),
            mesh_reader,
            program,
            albedo,
            normal,
            extra,
            display,
        })
    }

    fn draw<S: Surface>(&mut self, target: &mut S, world: &mut World) -> Result<(), String> {
        let cameras = world.read_storage::<Camera>();
        let global = world.read_storage::<comp::GlobalTransform>();

        // Either get the active camera, or the first camera we can find.
        let camera_entity = world.read_resource::<ActiveCamera>().0;
        let camera = get_camera(camera_entity, &cameras, &global);

        // If we don't have any cameras anywhere, then just put it at the origin.
        let (width, height) = self.display.get_framebuffer_dimensions();
        let (view, proj) = get_proj_view(width, height, camera);

        for (transform, idx) in (&global, &self.current_meshes).join() {
            let buffers = &self.meshes[&idx];
            target
                .draw(
                    buffers.glium_verts(),
                    &buffers.index,
                    &self.program,
                    &uniform! {
                        // tfms: &self.transform_buffer,
                        model: array4x4(transform.0.to_matrix()),
                        albedo_maps: self.albedo.sampled(), //.magnify_filter(MagnifySamplerFilter::Nearest),
                        normal_maps: self.normal.sampled(), //.magnify_filter(MagnifySamplerFilter::Nearest),
                        extra_maps: self.extra.sampled(), //.magnify_filter(MagnifySamplerFilter::Nearest),
                        view: array4x4(view),
                        projection: array4x4(proj.to_homogeneous()),
                    },
                    &glium::DrawParameters {
                        depth: glium::Depth {
                            test: glium::DepthTest::IfLess,
                            write: true,
                            ..Default::default()
                        },
                        // backface_culling: glium::BackfaceCullingMode::CullCounterClockwise,
                        ..Default::default()
                    },
                )
                .map_err(|err| format!("{:?}", err))?;
        }
        Ok(())
    }

    /// Synchronize the World state and the GPU meshes
    fn update_meshes<F: Facade>(&mut self, ctx: &F, world: &mut World) -> Result<(), String> {
        self.new_meshes.clear();
        for &event in world
            .read_storage::<TerrainMesh>()
            .channel()
            .read(&mut self.mesh_reader)
        {
            match event {
                ComponentEvent::Inserted(id) => {
                    self.new_meshes.add(id);
                    self.current_meshes.add(id);
                }
                ComponentEvent::Modified(id) => {
                    self.meshes.remove(&id);
                    self.new_meshes.add(id);
                }
                ComponentEvent::Removed(id) => {
                    self.meshes.remove(&id);
                    self.current_meshes.remove(id);
                }
            }
        }

        for (mesh, idx) in (&world.read_storage::<TerrainMesh>(), &self.new_meshes).join() {
            self.meshes
                .insert(idx, TerrainBuffers::immutable(ctx, &mesh)?);
        }
        Ok(())
    }
}

// #[derive(Clone, Debug, PartialEq)]
// pub struct MeshBuilder {
//     // Indices are special
//     indices: Option<IndexBuffer>,
//     attributes: HashMap<Cow<'static, str>, AttributeBuffer>,
//     primitive: Primitive,
// }

// impl MeshBuilder {
//     pub fn new(primitive: Primitive) -> Self {
//         Mesh {
//             indices: None,
//             attributes: HashMap::new(),
//             primitive,
//         }
//     }

//     pub fn with_indices<T>(mut self, indices: T) -> Self
//     where
//         T: Into<IndexBuffer>,
//     {
//         self.indices = indices.into();
//         self
//     }

//     pub fn with<T, K>(mut self, name: K, buf: T) -> Self
//     where
//         T: Into<AttributeBuffer>,
//         K: Into<Cow<'static, str>>,
//     {
//         self.attributes.insert(name.into(), buf.into());
//         self
//     }

//     pub fn build(self) -> Result<Mesh, (Self, MeshBuildError)> {
//         // TODO: validate that `indices` doesn't point to anything OOB
//     }
// }

// impl StaticMesh {}

// // fn foo() {
// //     // StaticMesh is meant for geometry that will stay in GPU memory for
// long //     // periods of time, and will have very fast GPU access.
// //     let mesh1 = MeshBuilder::new(Primitive::TriangleList)
// //         .with("position", (3, pos_buffer))
// //         .with("uv", (uv_buffer))
// //         .with_norm_tang();

// //     chunk_meshes.insert(chunk, mesh1);
// //     chunk_materials.insert(chunk, self.chunk_material); // Handle to
// terrain // shader n stuff }

// //     // DynamicMesh will be in CPU-visible memory, so it's better suited
// for // geometry     // that will be updated frequently.
// //     let mesh2 = DynamicMesh::empty(&ctx, Primitive::TriangleList)
// //         .with::<Position>(pos_buffer)
// //         .with::<Uv>(uv_buffer);

// //     let shader = Shader::new("resources/shaders/lava");
// // }

pub fn array4x4<T: Into<[[U; 4]; 4]>, U>(mat: T) -> [[U; 4]; 4] {
    mat.into()
}

pub fn array3<T: Into<[U; 3]>, U>(vec: T) -> [U; 3] {
    vec.into()
}
