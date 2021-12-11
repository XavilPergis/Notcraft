use crate::engine::{
    loader,
    math::*,
    render::{
        camera::{ActiveCamera, Camera},
        mesher::TerrainMesh,
    },
    transform::GlobalTransform,
    world::registry::BlockRegistry,
};
use anyhow::Result;
use crossbeam_channel::Receiver;
use glium::{
    backend::Facade,
    framebuffer::{MultiOutputFrameBuffer, ValidationError},
    index::{IndexBuffer, PrimitiveType},
    texture::{
        DepthTexture2d, MipmapsOption, RawImage2d, SrgbTexture2dArray, Texture2d,
        UncompressedFloatFormat,
    },
    uniform,
    uniforms::MagnifySamplerFilter,
    vertex::VertexBuffer,
    Display, Program, Surface,
};
use legion::{world::Event, Entity, IntoQuery, Read, Resources, World};
use nalgebra::{self as na, Perspective3};
use std::{
    collections::{HashMap, HashSet},
    rc::Rc,
    sync::Arc,
};

use super::{mesher::TerrainVertex, Tex};

struct TerrainFramebuffers {
    depth_buffer: DepthTexture2d,
    color_buffer: Texture2d,
}

impl TerrainFramebuffers {
    fn new<F: Facade>(ctx: &F, width: u32, height: u32) -> Result<Self> {
        Ok(TerrainFramebuffers {
            depth_buffer: DepthTexture2d::empty(ctx, width, height)?,
            color_buffer: Texture2d::empty_with_format(
                ctx,
                UncompressedFloatFormat::F32F32F32,
                MipmapsOption::NoMipmap,
                width,
                height,
            )?,
        })
    }
}

struct SharedState {
    display: Rc<Display>,
    fullscreen_quad: VertexBuffer<Tex>,
}

impl SharedState {
    pub fn new(display: Rc<Display>) -> Result<Rc<Self>> {
        let fullscreen_quad = VertexBuffer::immutable(&*display, &[
            Tex { uv: [-1.0, 1.0] },
            Tex { uv: [1.0, 1.0] },
            Tex { uv: [-1.0, -1.0] },
            Tex { uv: [1.0, 1.0] },
            Tex { uv: [-1.0, -1.0] },
            Tex { uv: [1.0, -1.0] },
        ])?;

        Ok(Rc::new(Self {
            display,
            fullscreen_quad,
        }))
    }

    pub fn display(&self) -> &Display {
        &*self.display
    }
}

pub struct Renderer {
    shared: Rc<SharedState>,

    terrain_renderer: TerrainRenderer,
    post_renderer: PostProcessRenderer,
}

fn render_all<S: Surface>(
    ctx: &mut Renderer,
    target: &mut S,
    world: &mut World,
    resources: &mut Resources,
) -> anyhow::Result<()> {
    // upload/delete etc meshes to keep in sync with the world.
    ctx.terrain_renderer.update_meshes(world).unwrap();

    // render terrain to terrain buffer
    render_terrain(
        &mut ctx.terrain_renderer,
        &mut ctx.post_renderer.render_target()?,
        world,
        resources,
    )?;

    render_post(&mut ctx.post_renderer, target, world, resources)?;

    Ok(())
}

impl Renderer {
    pub fn new(
        display: Rc<Display>,
        registry: Arc<BlockRegistry>,
        world: &mut World,
    ) -> Result<Self> {
        let shared = SharedState::new(display)?;

        let terrain_render = TerrainRenderer::new(Rc::clone(&shared), registry, world)?;
        let post_renderer = PostProcessRenderer::new(Rc::clone(&shared))?;

        Ok(Renderer {
            shared,
            terrain_renderer: terrain_render,
            post_renderer,
        })
    }

    pub fn draw<S: Surface>(
        &mut self,
        target: &mut S,
        world: &mut World,
        resources: &mut Resources,
    ) -> Result<()> {
        render_all(self, target, world, resources)
    }
}

struct PostProcessRenderer {
    shared: Rc<SharedState>,
    post_program: Program,
    post_process_source: TerrainFramebuffers,
}

impl PostProcessRenderer {
    pub fn new(shared: Rc<SharedState>) -> Result<Self> {
        let post_program = loader::load_shader(shared.display(), "resources/shaders/post")?;
        let (width, height) = shared.display().get_framebuffer_dimensions();
        let post_process_source = TerrainFramebuffers::new(shared.display(), width, height)?;

        Ok(Self {
            shared,
            post_program,
            post_process_source,
        })
    }

    fn render_target(&self) -> Result<MultiOutputFrameBuffer, ValidationError> {
        MultiOutputFrameBuffer::with_depth_buffer(
            self.shared.display(),
            [("b_color", &self.post_process_source.color_buffer)]
                .iter()
                .cloned(),
            &self.post_process_source.depth_buffer,
        )
    }
}

fn render_post<S: Surface>(
    ctx: &mut PostProcessRenderer,
    target: &mut S,
    world: &mut World,
    resources: &mut Resources,
) -> anyhow::Result<()> {
    let cam_transform = get_camera(world, resources);
    let (width, height) = ctx.shared.display().get_framebuffer_dimensions();
    let (view, proj) = get_proj_view(width, height, cam_transform);
    let cam_pos = get_cam_pos(cam_transform);

    // post
    target.clear_color(0.1, 0.3, 0.3, 1.0);
    target.draw(
        &ctx.shared.fullscreen_quad,
        glium::index::NoIndices(glium::index::PrimitiveType::TrianglesList),
        &ctx.post_program,
        &uniform! {
            b_color: ctx.post_process_source.color_buffer.sampled(),
            b_depth: ctx.post_process_source.depth_buffer.sampled(),

            camera_pos: array3(cam_pos),
            projection_matrix: array4x4(proj.to_homogeneous()),
            view_matrix: array4x4(view),
        },
        &Default::default(),
    )?;

    Ok(())
}

#[derive(Debug)]
struct TerrainBuffers {
    // TODO: use u16 when we can
    indices: IndexBuffer<u32>,
    vertices: VertexBuffer<TerrainVertex>,
}

impl TerrainBuffers {
    fn immutable<F: Facade>(ctx: &F, mesh: &TerrainMesh) -> Result<Self> {
        Ok(TerrainBuffers {
            indices: IndexBuffer::immutable(ctx, PrimitiveType::TrianglesList, &mesh.indices)?,
            vertices: VertexBuffer::immutable(ctx, &mesh.vertices)?,
        })
    }

    fn glium_verts(&self) -> impl glium::vertex::MultiVerticesSource<'_> {
        &self.vertices
    }
}

fn get_camera<'a>(
    world: &'a mut World,
    resources: &mut Resources,
) -> Option<(&'a Camera, &'a GlobalTransform)> {
    let active = resources.get::<ActiveCamera>().and_then(|id| id.0)?;

    let camera = Read::<Camera>::query().get(world, active).ok()?;
    let global = Read::<GlobalTransform>::query().get(world, active).ok()?;

    Some((camera, global))
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
        None => na::vector!(0.0, 0.0, 0.0),
    }
}

struct TerrainRenderer {
    shared: Rc<SharedState>,

    // entities with `TerrainMesh` that need to be rebuilt
    needs_rebuild: HashSet<Entity>,

    // since `TerrainBuffers` is not `Send + Sync`, we need to store it here
    // TODO: make this not suck
    built_meshes: HashMap<Entity, TerrainBuffers>,

    mesh_events: Receiver<Event>,

    block_textures: SrgbTexture2dArray,
    terrain_program: Program,
}

impl TerrainRenderer {
    fn new(
        shared: Rc<SharedState>,
        registry: Arc<BlockRegistry>,
        world: &mut World,
    ) -> Result<Self> {
        let program = loader::load_shader(shared.display(), "resources/shaders/simple")?;
        let (width, height, maps) =
            loader::load_block_textures("resources/textures/blocks", registry.texture_paths())?;
        let dims = (width, height);

        let mut albedo = vec![];

        for map in maps {
            assert!(map.albedo.dimensions() == dims);
            albedo.push(RawImage2d::from_raw_rgba_reversed(
                &map.albedo.into_raw(),
                dims,
            ));
        }

        let albedo =
            SrgbTexture2dArray::with_mipmaps(shared.display(), albedo, MipmapsOption::NoMipmap)?;

        let (mesh_events_tx, mesh_events_rx) = crossbeam_channel::unbounded();
        world.subscribe(mesh_events_tx, legion::component::<TerrainMesh>());

        Ok(TerrainRenderer {
            needs_rebuild: Default::default(),
            built_meshes: Default::default(),
            mesh_events: mesh_events_rx,
            terrain_program: program,
            block_textures: albedo,
            shared,
        })
    }

    /// Synchronize the World state and the GPU meshes
    fn update_meshes(&mut self, world: &mut World) -> Result<()> {
        self.needs_rebuild.clear();

        for event in self.mesh_events.try_iter() {
            match event {
                Event::ArchetypeCreated(_) => {}

                Event::EntityInserted(entity, _) => {
                    self.needs_rebuild.insert(entity);
                }

                Event::EntityRemoved(entity, _) => {
                    self.built_meshes.remove(&entity);
                }
            }
        }

        let mut mesh_query = Read::<TerrainMesh>::query();
        let mut changed_query = Entity::query().filter(legion::maybe_changed::<TerrainMesh>());

        for &entity in changed_query.iter(world) {
            if let Ok(_mesh) = mesh_query.get(world, entity) {
                self.built_meshes.remove(&entity);
                self.needs_rebuild.insert(entity);
            }
        }

        for &entity in self.needs_rebuild.iter() {
            if let Ok(mesh) = mesh_query.get(world, entity) {
                if !mesh.indices.is_empty() {
                    self.built_meshes.insert(
                        entity,
                        TerrainBuffers::immutable(self.shared.display(), &mesh)?,
                    );
                } else {
                    self.built_meshes.remove(&entity);
                }
            }
        }
        Ok(())
    }
}

fn render_terrain<S: Surface>(
    ctx: &mut TerrainRenderer,
    target: &mut S,
    world: &mut World,
    resources: &mut Resources,
) -> anyhow::Result<()> {
    let camera = get_camera(world, resources);

    target.clear_color_and_depth((0.0, 0.0, 0.0, 0.0), 1.0);

    // If we don't have any cameras anywhere, then just put it at the origin.
    let (width, height) = ctx.shared.display().get_framebuffer_dimensions();
    let (view, proj) = get_proj_view(width, height, camera);

    let mut transforms = Read::<GlobalTransform>::query();
    for (&entity, buffers) in ctx.built_meshes.iter() {
        if let Ok(transform) = transforms.get(world, entity) {
            target.draw(
                buffers.glium_verts(),
                &buffers.indices,
                &ctx.terrain_program,
                &uniform! {
                    model: array4x4(transform.0.to_matrix()),
                    albedo_maps: ctx.block_textures.sampled().magnify_filter(MagnifySamplerFilter::Nearest),
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
            )?;
        }
    }

    Ok(())
}

pub fn array4x4<T: Into<[[U; 4]; 4]>, U>(mat: T) -> [[U; 4]; 4] {
    mat.into()
}

pub fn array3<T: Into<[U; 3]>, U>(vec: T) -> [U; 3] {
    vec.into()
}
