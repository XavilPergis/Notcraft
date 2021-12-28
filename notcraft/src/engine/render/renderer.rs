use super::Tex;
use crate::{
    engine::{
        loader,
        math::*,
        render::{
            camera::{ActiveCamera, Camera},
            mesher::TerrainMesh,
        },
        transform::Transform,
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
    Blend, BlitTarget, Display, DrawParameters, Program, Rect, Surface,
};
use legion::{world::Event, Entity, IntoQuery, Read, Resources, World};
use na::vector;
use nalgebra::{self as na, Perspective3};
use parking_lot::RwLock;
use std::{
    collections::{HashMap, HashSet},
    marker::PhantomData,
    rc::Rc,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
    time::{Duration, Instant},
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
    debug_renderer: DebugLinesRenderer,
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

    render_debug(
        &mut ctx.debug_renderer,
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
        let debug_renderer = DebugLinesRenderer::new(Rc::clone(&shared))?;

        Ok(Renderer {
            _shared: shared,
            terrain_renderer,
            post_renderer,
            sky_renderer,
            debug_renderer,
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

            camera_pos: array3(&cam_pos),
            projection_matrix: array4x4(&proj.to_homogeneous()),
            view_matrix: array4x4(&view),
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
            camera_pos: array3(&cam_pos),
            projection_matrix: array4x4(&proj.to_homogeneous()),
            view_matrix: array4x4(&view),
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
) -> Option<(&'a Camera, &'a Transform)> {
    let active = resources.get::<ActiveCamera>().and_then(|id| id.0)?;

    let camera = Read::<Camera>::query().get(world, active).ok()?;
    let global = Read::<Transform>::query().get(world, active).ok()?;

    Some((camera, global))
}

fn get_view_projection(
    width: u32,
    height: u32,
    cam_transform: Option<(&Camera, &Transform)>,
) -> (Matrix4<f32>, Perspective3<f32>) {
    let (view, mut proj) = cam_transform
        .map(|(cam, transform)| (transform.to_matrix().try_inverse().unwrap(), cam.projection))
        .unwrap_or_else(|| (na::Matrix4::identity(), Camera::default().projection));
    proj.set_aspect(width as f32 / height as f32);
    (view, proj)
}

fn get_cam_pos(cam_transform: Option<(&Camera, &Transform)>) -> Vector3<f32> {
    match cam_transform {
        Some((_, transform)) => transform.translation.vector,
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

    let mut query = <(&Transform, &RenderMeshComponent<TerrainMesh>)>::query();
    for &entity in ctx.terrain_meshes.render_component_entities.iter() {
        if let Ok((transform, RenderMeshComponent(handle))) = query.get(world, entity) {
            let buffers =
                ctx.terrain_meshes.meshes.get(&handle.0.id).expect(
                    "RenderMeshComponent existed for entity that was not in terrain_entities",
                );

            let model = transform.to_matrix();
            let mvp = viewproj * model;

            if !should_draw_aabb(&mvp, &buffers.aabb) {
                continue;
            }

            target.draw(
                &buffers.vertices,
                &buffers.indices,
                &ctx.terrain_program,
                &uniform! {
                    model: array4x4(&model),
                    view: array4x4(&view),
                    projection: array4x4(&proj.to_homogeneous()),
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
    pub fn with_dimensions(dims: Vector3<f32>) -> Self {
        let half_dims = dims / 2.0;
        Aabb {
            min: Point3::from(-half_dims),
            max: Point3::from(half_dims),
        }
    }

    #[rustfmt::skip]
    pub fn contains(&self, point: &Point3<f32>) -> bool {
        util::is_between(point.x, self.min.x, self.max.x) &&
        util::is_between(point.y, self.min.y, self.max.y) &&
        util::is_between(point.z, self.min.z, self.max.z)
    }

    #[rustfmt::skip]
    pub fn intersects(&self, other: &Aabb) -> bool {
        (self.contains(&other.min) || self.contains(&other.max)) ||
        (other.contains(&self.min) || other.contains(&self.max))
    }

    pub fn dimensions(&self) -> Vector3<f32> {
        na::vector![
            self.max.x - self.min.x,
            self.max.y - self.min.y,
            self.max.z - self.min.z
        ]
    }

    pub fn center(&self) -> Point3<f32> {
        self.min + self.dimensions() / 2.0
    }

    pub fn translated(&self, translation: Vector3<f32>) -> Aabb {
        Aabb {
            min: self.min + translation,
            max: self.max + translation,
        }
    }

    pub fn inflate(&self, distance: f32) -> Aabb {
        Aabb {
            min: self.min - vector![distance, distance, distance],
            max: self.max + vector![distance, distance, distance],
        }
    }

    pub fn transformed(&self, transform: &Transform) -> Aabb {
        // you can think of this as a vector based at `self.min`, with its tip in the
        // center of the AABB.
        let half_dimensions = self.dimensions() / 2.0;
        // translate our center to the new center
        let center = self.min + half_dimensions;
        let center = transform.translation * center;
        // the whole reason we couln't just add the transform's translation is because
        // scaling the AABB when its center is not at the origin would have a
        // translating sort of effect. here we just scale the "to center" vector by the
        // scale and define the new AABB as displacements from the translated center
        let corner_displacement = half_dimensions.component_mul(&transform.scale);
        Aabb {
            min: center - corner_displacement,
            max: center + corner_displacement,
        }
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

pub fn array4x4<T: Copy + Into<[[U; 4]; 4]>, U>(mat: &T) -> [[U; 4]; 4] {
    (*mat).into()
}

pub fn array3<T: Copy + Into<[U; 3]>, U>(vec: &T) -> [U; 3] {
    (*vec).into()
}

lazy_static::lazy_static! {
    static ref DEBUG_BOX_SENDER: RwLock<Option<Sender<DebugBox>>> = RwLock::new(None);
    static ref TRANSIENT_DEBUG_BOX_SENDER: RwLock<Option<Sender<(Duration, DebugBox)>>> = RwLock::new(None);
}

pub fn add_debug_box(debug_box: DebugBox) {
    if let Some(sender) = DEBUG_BOX_SENDER.read().as_ref() {
        sender.send(debug_box).unwrap();
    }
}

pub fn add_transient_debug_box(duration: Duration, debug_box: DebugBox) {
    if let Some(sender) = TRANSIENT_DEBUG_BOX_SENDER.read().as_ref() {
        sender.send((duration, debug_box)).unwrap();
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Default)]
#[repr(C)]
struct DebugVertex {
    pub pos: [f32; 3],
    pub color: [f32; 4],
    pub kind_end: u32,
}
glium::implement_vertex!(DebugVertex, pos, color, kind_end);

#[derive(Copy, Clone, Debug)]
#[repr(u8)]
pub enum DebugBoxKind {
    Solid = 0,
    Dashed = 1,
    Dotted = 2,
}

#[derive(Copy, Clone, Debug)]
pub struct DebugBox {
    pub bounds: Aabb,
    pub rgba: [f32; 4],
    pub kind: DebugBoxKind,
}

impl DebugBox {
    pub fn with_kind(mut self, kind: DebugBoxKind) -> Self {
        self.kind = kind;
        self
    }
}

struct DebugLinesRenderer {
    shared: Rc<CommonState>,
    debug_program: Program,
    debug_box_channel: util::ChannelPair<DebugBox>,
    transient_debug_box_channel: util::ChannelPair<(Duration, DebugBox)>,
    next_transient_id: usize,
    transient_debug_boxes: HashMap<usize, (Instant, Duration, DebugBox)>,
    dead_transient_debug_boxes: HashSet<usize>,
    line_buf: Vec<DebugVertex>,
}

impl DebugLinesRenderer {
    fn new(shared: Rc<CommonState>) -> Result<Self> {
        let debug_program = loader::load_shader(shared.display(), "resources/shaders/debug")?;
        let debug_box_channel = util::ChannelPair::new();
        let transient_debug_box_channel = util::ChannelPair::new();

        *DEBUG_BOX_SENDER.write() = Some(debug_box_channel.sender());
        *TRANSIENT_DEBUG_BOX_SENDER.write() = Some(transient_debug_box_channel.sender());

        Ok(Self {
            shared,
            debug_program,
            debug_box_channel,
            transient_debug_box_channel,
            next_transient_id: 0,
            transient_debug_boxes: Default::default(),
            dead_transient_debug_boxes: Default::default(),
            line_buf: Default::default(),
        })
    }
}

fn aabb_corners(aabb: &Aabb) -> [Vector3<f32>; 8] {
    [
        vector![aabb.min.x, aabb.min.y, aabb.min.z],
        vector![aabb.min.x, aabb.min.y, aabb.max.z],
        vector![aabb.min.x, aabb.max.y, aabb.min.z],
        vector![aabb.min.x, aabb.max.y, aabb.max.z],
        vector![aabb.max.x, aabb.min.y, aabb.min.z],
        vector![aabb.max.x, aabb.min.y, aabb.max.z],
        vector![aabb.max.x, aabb.max.y, aabb.min.z],
        vector![aabb.max.x, aabb.max.y, aabb.max.z],
    ]
}

fn write_debug_box(buf: &mut Vec<DebugVertex>, debug_box: &DebugBox) {
    let [nnn, nnp, npn, npp, pnn, pnp, ppn, ppp] = aabb_corners(&debug_box.bounds);

    let mut line = |start: &Vector3<f32>, end: &Vector3<f32>| {
        buf.push(DebugVertex {
            pos: array3(start),
            color: debug_box.rgba,
            kind_end: (debug_box.kind as u32) << 1,
        });
        buf.push(DebugVertex {
            pos: array3(end),
            color: debug_box.rgba,
            kind_end: ((debug_box.kind as u32) << 1) | 1,
        });
    };

    // bottom
    line(&nnn, &nnp);
    line(&nnp, &pnp);
    line(&pnp, &pnn);
    line(&pnn, &nnn);

    // top
    line(&npn, &npp);
    line(&npp, &ppp);
    line(&ppp, &ppn);
    line(&ppn, &npn);

    // connecting lines
    line(&nnn, &npn);
    line(&nnp, &npp);
    line(&pnp, &ppp);
    line(&pnn, &ppn);
}

fn render_debug<S: Surface>(
    ctx: &mut DebugLinesRenderer,
    target: &mut S,
    world: &mut World,
    resources: &mut Resources,
) -> anyhow::Result<()> {
    ctx.line_buf.clear();
    for debug_box in ctx.debug_box_channel.rx.try_iter() {
        write_debug_box(&mut ctx.line_buf, &debug_box);
    }
    for (duration, debug_box) in ctx.transient_debug_box_channel.rx.try_iter() {
        ctx.transient_debug_boxes
            .insert(ctx.next_transient_id, (Instant::now(), duration, debug_box));
        ctx.next_transient_id += 1;
        write_debug_box(&mut ctx.line_buf, &debug_box);
    }

    for (&i, (start, duration, debug_box)) in ctx.transient_debug_boxes.iter_mut() {
        let elapsed = start.elapsed();
        if elapsed > *duration {
            ctx.dead_transient_debug_boxes.insert(i);
        } else {
            let percent_completed = elapsed.as_secs_f32() / duration.as_secs_f32();
            let mut rgba = debug_box.rgba;
            rgba[3] *= 1.0 - percent_completed;
            write_debug_box(&mut ctx.line_buf, &DebugBox { rgba, ..*debug_box });
        }
    }

    for i in ctx.dead_transient_debug_boxes.drain() {
        ctx.transient_debug_boxes.remove(&i);
    }

    let vertices = VertexBuffer::immutable(ctx.shared.display(), &ctx.line_buf)?;

    let cam_transform = get_camera(world, resources);
    let (width, height) = ctx.shared.display().get_framebuffer_dimensions();
    let (view, proj) = get_view_projection(width, height, cam_transform);

    target.draw(
        &vertices,
        glium::index::NoIndices(glium::index::PrimitiveType::LinesList),
        &ctx.debug_program,
        &uniform! {
            view: array4x4(&view),
            projection: array4x4(&proj.to_homogeneous()),
        },
        &DrawParameters {
            line_width: Some(1.0),
            blend: Blend::alpha_blending(),
            depth: glium::Depth {
                test: glium::DepthTest::IfLess,
                write: false,
                ..Default::default()
            },
            ..Default::default()
        },
    )?;

    Ok(())
}
