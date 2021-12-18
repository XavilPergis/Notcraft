use super::Tex;
use crate::{
    engine::{
        loader,
        math::*,
        render::{
            camera::{ActiveCamera, Camera},
            mesher::TerrainMesh,
        },
        transform::GlobalTransform,
        world::registry::BlockRegistry,
    },
    util,
};
use anyhow::Result;
use crossbeam_channel::{Receiver, Sender};
use glium::{
    backend::Facade,
    framebuffer::{SimpleFrameBuffer, ValidationError},
    index::IndexBuffer,
    texture::{
        DepthTexture2d, DepthTexture2dMultisample, MipmapsOption, RawImage2d, SrgbTexture2dArray,
        Texture2d, Texture2dMultisample, UncompressedFloatFormat,
    },
    uniform,
    uniforms::MagnifySamplerFilter,
    vertex::VertexBuffer,
    BlitTarget, Display, Program, Rect, Surface,
};
use legion::{world::Event, Entity, IntoQuery, Read, Resources, World};
use nalgebra::{self as na, Perspective3};
use std::{
    collections::{HashMap, HashSet},
    marker::PhantomData,
    rc::Rc,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
};

struct CommonState {
    display: Rc<Display>,
    fullscreen_quad: VertexBuffer<Tex>,
}

impl CommonState {
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
    _shared: Rc<CommonState>,

    terrain_renderer: TerrainRenderer,
    post_renderer: PostProcessRenderer,
    sky_renderer: SkyRenderer,
}

fn render_all<S: Surface>(
    ctx: &mut Renderer,
    target: &mut S,
    world: &mut World,
    resources: &mut Resources,
) -> anyhow::Result<()> {
    render_sky(
        &mut ctx.sky_renderer,
        &mut ctx.post_renderer.render_target()?,
        world,
        resources,
    )?;

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
        resources: &mut Resources,
    ) -> Result<Self> {
        let shared = CommonState::new(display)?;

        let terrain_renderer =
            TerrainRenderer::new(Rc::clone(&shared), registry, world, resources)?;
        let post_renderer = PostProcessRenderer::new(Rc::clone(&shared))?;
        let sky_renderer = SkyRenderer::new(Rc::clone(&shared))?;

        Ok(Renderer {
            _shared: shared,
            terrain_renderer,
            post_renderer,
            sky_renderer,
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

pub const MSAA_SAMPLES: u32 = 4;

struct PostProcessRenderer {
    shared: Rc<CommonState>,
    post_program: Program,
    post_process_color_target: Texture2dMultisample,
    post_process_depth_target: DepthTexture2dMultisample,
    post_process_color_resolved: Texture2d,
    post_process_depth_resolved: DepthTexture2d,
}

impl PostProcessRenderer {
    pub fn new(shared: Rc<CommonState>) -> Result<Self> {
        let post_program = loader::load_shader(shared.display(), "resources/shaders/post")?;
        let (width, height) = shared.display().get_framebuffer_dimensions();

        let post_process_color_target = Texture2dMultisample::empty_with_format(
            shared.display(),
            UncompressedFloatFormat::F32F32F32,
            MipmapsOption::NoMipmap,
            width,
            height,
            MSAA_SAMPLES,
        )?;
        let post_process_depth_target =
            DepthTexture2dMultisample::empty(shared.display(), width, height, MSAA_SAMPLES)?;

        let post_process_color_resolved = Texture2d::empty_with_format(
            shared.display(),
            UncompressedFloatFormat::F32F32F32,
            MipmapsOption::NoMipmap,
            width,
            height,
        )?;
        let post_process_depth_resolved = DepthTexture2d::empty(shared.display(), width, height)?;

        Ok(Self {
            shared,
            post_program,
            post_process_color_target,
            post_process_depth_target,
            post_process_color_resolved,
            post_process_depth_resolved,
        })
    }

    fn render_target(&self) -> Result<SimpleFrameBuffer, ValidationError> {
        SimpleFrameBuffer::with_depth_buffer(
            self.shared.display(),
            &self.post_process_color_target,
            &self.post_process_depth_target,
        )
    }

    fn resolve_target(&self) -> Result<SimpleFrameBuffer, ValidationError> {
        SimpleFrameBuffer::with_depth_buffer(
            self.shared.display(),
            &self.post_process_color_resolved,
            &self.post_process_depth_resolved,
        )
    }
}

fn recreate_post_textures(ctx: &mut PostProcessRenderer, width: u32, height: u32) -> Result<()> {
    ctx.post_process_color_target = Texture2dMultisample::empty_with_format(
        ctx.shared.display(),
        UncompressedFloatFormat::F32F32F32,
        MipmapsOption::NoMipmap,
        width,
        height,
        MSAA_SAMPLES,
    )?;
    ctx.post_process_depth_target =
        DepthTexture2dMultisample::empty(ctx.shared.display(), width, height, MSAA_SAMPLES)?;

    ctx.post_process_color_resolved = Texture2d::empty_with_format(
        ctx.shared.display(),
        UncompressedFloatFormat::F32F32F32,
        MipmapsOption::NoMipmap,
        width,
        height,
    )?;
    ctx.post_process_depth_resolved = DepthTexture2d::empty(ctx.shared.display(), width, height)?;

    Ok(())
}

fn render_post<S: Surface>(
    ctx: &mut PostProcessRenderer,
    target: &mut S,
    world: &mut World,
    resources: &mut Resources,
) -> anyhow::Result<()> {
    let (width, height) = target.get_dimensions();
    let (buf_width, buf_height) = ctx.post_process_depth_resolved.dimensions();
    if buf_width != width || buf_height != height {
        recreate_post_textures(ctx, width, height)?;
    }

    let cam_transform = get_camera(world, resources);
    let (view, proj) = get_view_projection(width, height, cam_transform);
    let cam_pos = get_cam_pos(cam_transform);

    ctx.resolve_target()?.blit_from_simple_framebuffer(
        &ctx.render_target()?,
        &Rect {
            left: 0,
            bottom: 0,
            width,
            height,
        },
        &BlitTarget {
            left: 0,
            bottom: 0,
            width: width as i32,
            height: height as i32,
        },
        MagnifySamplerFilter::Linear,
    );

    // post
    target.clear_color(0.0, 0.0, 0.0, 0.0);
    target.draw(
        &ctx.shared.fullscreen_quad,
        glium::index::NoIndices(glium::index::PrimitiveType::TrianglesList),
        &ctx.post_program,
        &uniform! {
            b_color: ctx.post_process_color_resolved.sampled(),
            b_depth: ctx.post_process_depth_resolved.sampled(),

            camera_pos: array3(cam_pos),
            projection_matrix: array4x4(proj.to_homogeneous()),
            view_matrix: array4x4(view),
        },
        &Default::default(),
    )?;

    Ok(())
}

struct SkyRenderer {
    shared: Rc<CommonState>,
    sky_program: Program,
}

impl SkyRenderer {
    fn new(shared: Rc<CommonState>) -> Result<Self> {
        let sky_program = loader::load_shader(shared.display(), "resources/shaders/sky")?;

        Ok(Self {
            shared,
            sky_program,
        })
    }
}

fn render_sky<S: Surface>(
    ctx: &mut SkyRenderer,
    target: &mut S,
    world: &mut World,
    resources: &mut Resources,
) -> anyhow::Result<()> {
    target.clear_color_and_depth((0.9, 0.95, 1.0, 1.0), 1.0);

    let cam_transform = get_camera(world, resources);
    let (width, height) = ctx.shared.display().get_framebuffer_dimensions();
    let (view, proj) = get_view_projection(width, height, cam_transform);
    let cam_pos = get_cam_pos(cam_transform);

    target.draw(
        &ctx.shared.fullscreen_quad,
        glium::index::NoIndices(glium::index::PrimitiveType::TrianglesList),
        &ctx.sky_program,
        &uniform! {
            camera_pos: array3(cam_pos),
            projection_matrix: array4x4(proj.to_homogeneous()),
            view_matrix: array4x4(view),
        },
        &Default::default(),
    )?;

    Ok(())
}

#[derive(Debug)]
pub struct MeshBuffers<V: Copy> {
    pub vertices: VertexBuffer<V>,
    pub indices: IndexBuffer<u32>,
    // mesh bounds, in model space
    pub aabb: Aabb,
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

fn get_view_projection(
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

#[derive(Debug)]
pub struct MeshHandle<M>(Arc<MeshHandleInner<M>>);

// unsafe impl<M> Send for MeshHandle<M> {}
// unsafe impl<M> Sync for MeshHandle<M> {}

impl<M> Clone for MeshHandle<M> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl<M> MeshHandle<M> {
    pub fn reupload(&self, mesh: M) {
        self.0.shared.mesh_sender.send((self.0.id, mesh)).unwrap();
    }
}

#[derive(Debug)]
pub struct MeshHandleInner<M> {
    id: usize,
    shared: Arc<SharedMeshContext<M>>,
    _phantom: PhantomData<M>,
}

impl<M> Drop for MeshHandleInner<M> {
    fn drop(&mut self) {
        // should be ok to ignore the result here, if the render thread shut down, then
        // that means the meshes were all already dropped.
        let _ = self.shared.mesh_dropped_sender.send(self.id);
    }
}

pub trait UploadableMesh {
    type Vertex: Copy;

    fn upload<F: Facade>(&self, ctx: &F) -> Result<MeshBuffers<Self::Vertex>>;
}

struct LocalMeshContext<M: UploadableMesh> {
    shared: Arc<SharedMeshContext<M>>,
    meshes: HashMap<usize, MeshBuffers<M::Vertex>>,
    render_component_events_receiver: Receiver<Event>,
    render_component_entities: HashSet<Entity>,
}

impl<M: UploadableMesh + Send + Sync + 'static> LocalMeshContext<M> {
    pub fn new(world: &mut World) -> Self {
        let (sender, render_component_events_receiver) = crossbeam_channel::unbounded();
        world.subscribe(sender, legion::component::<RenderMeshComponent<M>>());
        Self {
            shared: SharedMeshContext::new(),
            meshes: Default::default(),
            render_component_events_receiver,
            render_component_entities: Default::default(),
        }
    }

    fn update<F: Facade>(&mut self, ctx: &F) -> Result<()> {
        for event in self.render_component_events_receiver.try_iter() {
            match event {
                Event::EntityInserted(entity, _) => {
                    self.render_component_entities.insert(entity);
                }
                Event::EntityRemoved(entity, _) => {
                    self.render_component_entities.remove(&entity);
                }

                _ => {}
            }
        }

        for (id, data) in self.shared.mesh_receiver.try_iter() {
            self.meshes.insert(id, data.upload(ctx)?);
        }

        for id in self.shared.mesh_dropped_receiver.try_iter() {
            self.meshes.remove(&id);
        }

        Ok(())
    }
}

#[derive(Debug)]
pub struct SharedMeshContext<M> {
    next_id: AtomicUsize,
    mesh_receiver: Receiver<(usize, M)>,
    mesh_sender: Sender<(usize, M)>,
    mesh_dropped_receiver: Receiver<usize>,
    mesh_dropped_sender: Sender<usize>,
}

impl<M> SharedMeshContext<M> {
    pub fn new() -> Arc<SharedMeshContext<M>> {
        let (mesh_sender, mesh_receiver) = crossbeam_channel::unbounded();
        let (mesh_dropped_sender, mesh_dropped_receiver) = crossbeam_channel::unbounded();

        Arc::new(Self {
            next_id: AtomicUsize::new(0),
            mesh_receiver,
            mesh_sender,
            mesh_dropped_receiver,
            mesh_dropped_sender,
        })
    }

    pub fn upload(self: &Arc<Self>, mesh: M) -> MeshHandle<M> {
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        self.mesh_sender.send((id, mesh)).unwrap();
        MeshHandle(Arc::new(MeshHandleInner {
            id,
            shared: Arc::clone(&self),
            _phantom: PhantomData,
        }))
    }
}

struct TerrainRenderer {
    shared: Rc<CommonState>,

    terrain_meshes: LocalMeshContext<TerrainMesh>,

    block_textures: SrgbTexture2dArray,
    terrain_program: Program,
}

impl TerrainRenderer {
    fn new(
        shared: Rc<CommonState>,
        registry: Arc<BlockRegistry>,
        world: &mut World,
        resources: &mut Resources,
    ) -> Result<Self> {
        let terrain_program = loader::load_shader(shared.display(), "resources/shaders/simple")?;
        let textures =
            loader::load_block_textures("resources/textures/blocks", registry.texture_paths())?;

        let textures = registry
            .texture_paths()
            .map(|name| {
                let map = &textures.block_textures[name];
                RawImage2d::from_raw_rgba_reversed(map, map.dimensions())
            })
            .collect();

        let block_textures =
            SrgbTexture2dArray::with_mipmaps(shared.display(), textures, MipmapsOption::NoMipmap)?;

        let terrain_meshes = LocalMeshContext::new(world);
        resources.insert(Arc::clone(&terrain_meshes.shared));

        Ok(TerrainRenderer {
            terrain_program,
            block_textures,
            terrain_meshes,
            shared,
        })
    }
}

#[derive(Debug)]
pub struct RenderMeshComponent<M>(MeshHandle<M>);

impl<M> RenderMeshComponent<M> {
    pub fn new(handle: MeshHandle<M>) -> Self {
        Self(handle)
    }
}

fn render_terrain<S: Surface>(
    ctx: &mut TerrainRenderer,
    target: &mut S,
    world: &mut World,
    resources: &mut Resources,
) -> anyhow::Result<()> {
    ctx.terrain_meshes.update(ctx.shared.display())?;

    let camera = get_camera(world, resources);

    // If we don't have any cameras anywhere, then just put it at the origin.
    let (width, height) = target.get_dimensions();
    let (view, proj) = get_view_projection(width, height, camera);
    let viewproj = proj.as_matrix() * view;

    let mut query = <(&GlobalTransform, &RenderMeshComponent<TerrainMesh>)>::query();
    for &entity in ctx.terrain_meshes.render_component_entities.iter() {
        if let Ok((transform, RenderMeshComponent(handle))) = query.get(world, entity) {
            let buffers =
                ctx.terrain_meshes.meshes.get(&handle.0.id).expect(
                    "RenderMeshComponent existed for entity that was not in terrain_entities",
                );

            let model = transform.0.to_matrix();
            let mvp = viewproj * model;

            if !should_draw_aabb(&mvp, &buffers.aabb) {
                continue;
            }

            target.draw(
                &buffers.vertices,
                &buffers.indices,
                &ctx.terrain_program,
                &uniform! {
                    model: array4x4(model),
                    view: array4x4(view),
                    projection: array4x4(proj.to_homogeneous()),
                    albedo_maps: ctx.block_textures.sampled().magnify_filter(MagnifySamplerFilter::Nearest),
                },
                &glium::DrawParameters {
                    depth: glium::Depth {
                        test: glium::DepthTest::IfLess,
                        write: true,
                        ..Default::default()
                    },
                    backface_culling: glium::BackfaceCullingMode::CullCounterClockwise,
                    ..Default::default()
                },
            )?;
        }
    }

    Ok(())
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Aabb {
    pub min: Point3<f32>,
    pub max: Point3<f32>,
}

impl Aabb {
    #[rustfmt::skip]
    fn contains(&self, point: &Point3<f32>) -> bool {
        util::is_within(point.x, self.min.x, self.max.x) &&
        util::is_within(point.y, self.min.y, self.max.y) &&
        util::is_within(point.z, self.min.z, self.max.z)
    }

    #[rustfmt::skip]
    pub fn intersects(&self, other: &Aabb) -> bool {
        (self.contains(&other.min) || self.contains(&other.max)) ||
        (other.contains(&self.min) || other.contains(&self.max))
    }
}

fn should_draw_aabb(mvp: &Matrix4<f32>, aabb: &Aabb) -> bool {
    // an AABB is excluded from the test if all its 8 corners lay outside any single
    // frustum plane. we transform into clip space because the camera frustum planes
    // have some very nice properties. each plane is 1 unit from the origin along
    // its respective axis, and points inwards directly towards the origin. because
    // of this, the test for e.x. the bottom plane is simply `point.y / point.w >
    // -1.0`. we can just test `point.y > -point.w` though, by multiplying both
    // sides of the inequality by `point.w`

    // my first attempt at this only tested if each corner was inside the camera
    // frustum, instead of outside any frustum plane, which led to some false
    // negatives where the corners would straddle the corner of the frustum, so the
    // line connecting them would cross through the frustum. this means that the
    // object might potentially influence the resulting image, but was excluded
    // because those points weren't actually inside the frustum.

    let corners_clip = [
        mvp * na::point![aabb.min.x, aabb.min.y, aabb.min.z, 1.0],
        mvp * na::point![aabb.max.x, aabb.min.y, aabb.min.z, 1.0],
        mvp * na::point![aabb.min.x, aabb.max.y, aabb.min.z, 1.0],
        mvp * na::point![aabb.max.x, aabb.max.y, aabb.min.z, 1.0],
        mvp * na::point![aabb.min.x, aabb.min.y, aabb.max.z, 1.0],
        mvp * na::point![aabb.max.x, aabb.min.y, aabb.max.z, 1.0],
        mvp * na::point![aabb.min.x, aabb.max.y, aabb.max.z, 1.0],
        mvp * na::point![aabb.max.x, aabb.max.y, aabb.max.z, 1.0],
    ];

    let px = !corners_clip.iter().all(|point| point.x > point.w);
    let nx = !corners_clip.iter().all(|point| point.x < -point.w);
    let py = !corners_clip.iter().all(|point| point.y > point.w);
    let ny = !corners_clip.iter().all(|point| point.y < -point.w);
    let pz = !corners_clip.iter().all(|point| point.z > point.w);
    let nz = !corners_clip.iter().all(|point| point.z < -point.w);

    px && nx && py && ny && pz && nz
}

pub fn array4x4<T: Into<[[U; 4]; 4]>, U>(mat: T) -> [[U; 4]; 4] {
    mat.into()
}

pub fn array3<T: Into<[U; 3]>, U>(vec: T) -> [U; 3] {
    vec.into()
}
