use crate::{
    engine::{
        camera::Camera,
        render::mesh::{GpuMesh, Mesh},
        world::ChunkPos,
    },
    loader,
};
use cgmath::{Vector2, Vector3};
use glium::{
    backend::Facade,
    index::PrimitiveType,
    texture::{MipmapsOption, RawImage2d, Texture2dArray},
    BackfaceCullingMode, Depth, DepthTest, DrawParameters, PolygonMode, Surface, Vertex,
};
use image::RgbaImage;
use specs::prelude::*;
use std::collections::HashMap;

pub mod debug;
#[macro_use]
pub mod mesh;
pub mod mesher;
pub mod terrain;
// pub mod ui;

pub struct DeferredRenderPassContext<'c, 'f, 'd, F> {
    pub facade: &'c F,
    pub target: glium::framebuffer::MultiOutputFrameBuffer<'f>,
    pub data: &'d GraphicsData,

    pub camera: Camera,
    pub polygon_mode: PolygonMode,
}

impl<F> DeferredRenderPassContext<'_, '_, '_, F> {
    pub fn view_matrix(&self) -> [[f32; 4]; 4] {
        self.camera.view_matrix().into()
    }

    pub fn projection_matrix(&self) -> [[f32; 4]; 4] {
        self.camera.projection_matrix().into()
    }

    pub fn eye_pos(&self) -> [f32; 3] {
        self.camera.position.into()
    }

    pub fn default_draw_params<'p>(&self) -> DrawParameters<'p> {
        DrawParameters {
            polygon_mode: self.polygon_mode,
            backface_culling: BackfaceCullingMode::CullCounterClockwise,
            depth: Depth {
                test: DepthTest::IfLess,
                write: true,
                ..Default::default()
            },
            ..Default::default()
        }
    }
}

pub trait DeferredRenderPass {
    fn draw<F: Facade>(
        &mut self,
        ctx: &mut DeferredRenderPassContext<'_, '_, '_, F>,
    ) -> Result<(), glium::DrawError>;
}

pub trait ForwardRenderPass {
    fn draw<F: Facade, S: Surface>(
        &mut self,
        ctx: &F,
        surface: &S,
        graphics: &GraphicsData,
    ) -> Result<(), glium::DrawError>;
}

impl TerrainVertex {
    pub fn with_pos<T: Into<[f32; 3]>>(self, pos: T) -> Self {
        TerrainVertex {
            pos: pos.into(),
            ..self
        }
    }

    pub fn with_normal<T: Into<[f32; 3]>>(self, norm: T) -> Self {
        TerrainVertex {
            normal: norm.into(),
            ..self
        }
    }

    pub fn with_uv<T: Into<[f32; 2]>>(self, uv: T) -> Self {
        TerrainVertex {
            uv: uv.into(),
            ..self
        }
    }

    pub fn with_texture(self, id: usize) -> Self {
        TerrainVertex {
            tex_id: id as i32,
            ..self
        }
    }

    pub fn with_tangent<T: Into<[f32; 3]>>(self, tangent: T) -> Self {
        TerrainVertex {
            tangent: tangent.into(),
            ..self
        }
    }

    pub fn with_ao(self, ao: f32) -> Self {
        TerrainVertex { ao, ..self }
    }
}

impl LiquidVertex {
    pub fn with_pos<T: Into<[f32; 3]>>(self, pos: T) -> Self {
        LiquidVertex {
            pos: pos.into(),
            ..self
        }
    }

    pub fn with_normal<T: Into<[f32; 3]>>(self, norm: T) -> Self {
        LiquidVertex {
            normal: norm.into(),
            ..self
        }
    }

    pub fn with_uv<T: Into<[f32; 2]>>(self, uv: T) -> Self {
        LiquidVertex {
            uv: uv.into(),
            ..self
        }
    }

    pub fn with_texture(self, id: usize) -> Self {
        LiquidVertex {
            tex_id: id as i32,
            ..self
        }
    }

    pub fn with_tangent<T: Into<[f32; 3]>>(self, tangent: T) -> Self {
        LiquidVertex {
            tangent: tangent.into(),
            ..self
        }
    }
}

glium::implement_vertex!(LiquidVertex, pos, uv, normal, tangent, tex_id);
#[derive(Copy, Clone, Debug, PartialEq, Default)]
pub struct LiquidVertex {
    pub pos: [f32; 3],
    pub uv: [f32; 2],
    pub normal: [f32; 3],
    pub tangent: [f32; 3],
    pub tex_id: i32,
}

glium::implement_vertex!(TerrainVertex, pos, uv, normal, tangent, tex_id, ao);
#[derive(Copy, Clone, Debug, PartialEq, Default)]
pub struct TerrainVertex {
    pub pos: [f32; 3],
    pub uv: [f32; 2],
    pub normal: [f32; 3],
    pub tangent: [f32; 3],
    pub tex_id: i32,
    pub ao: f32,
}

impl_geom_vertex!(LiquidVertex, pos, uv, normal, tangent);
impl_geom_vertex!(TerrainVertex, pos, uv, normal, tangent);

pub type LiquidMesh = Mesh<LiquidVertex, u32>;
pub type TerrainMesh = Mesh<TerrainVertex, u32>;

#[derive(Debug)]
pub struct MeshPair<V: Copy> {
    pub dirty: bool,
    pub cpu: Mesh<V, u32>,
    pub gpu: Option<GpuMesh<V, u32>>,
}

impl<T: Copy + Vertex> MeshPair<T> {
    pub fn upload<F: Facade>(&mut self, ctx: &F) {
        if self.dirty || self.gpu.is_none() {
            self.gpu = Some(
                self.cpu
                    .to_gpu_mesh(ctx, PrimitiveType::TrianglesList)
                    .unwrap(),
            );
            self.dirty = false;
        }
    }
}

#[derive(Debug)]
pub struct GraphicsData {
    pub terrain_meshes: HashMap<ChunkPos, (MeshPair<TerrainVertex>, MeshPair<LiquidVertex>)>,

    pub albedo_maps: Texture2dArray,
    pub normal_maps: Texture2dArray,
    pub height_maps: Texture2dArray,
    pub roughness_maps: Texture2dArray,
    pub ao_maps: Texture2dArray,
    pub metallic_maps: Texture2dArray,
}

const ALBEDO: &str = "albedo";
const NORMAL: &str = "normal";
const HEIGHT: &str = "height";
const ROUGHNESS: &str = "roughness";
const AO: &str = "ao";
const METALLIC: &str = "metallic";

impl GraphicsData {
    pub fn new<F: Facade>(ctx: &F, names: Vec<String>) -> Self {
        let defaults = loader::load_textures(ctx, "resources/textures/defaults").unwrap();

        // TODO: return errors n shit instead of just panic-ing
        assert!(defaults.contains_key(ALBEDO));
        assert!(defaults.contains_key(NORMAL));
        assert!(defaults.contains_key(HEIGHT));
        assert!(defaults.contains_key(ROUGHNESS));
        assert!(defaults.contains_key(AO));
        assert!(defaults.contains_key(METALLIC));

        let texture_maps = names
            .iter()
            .map(|name| loader::load_textures(ctx, format!("resources/textures/{}", name)).unwrap())
            .collect::<Vec<_>>();

        let mut albedo = vec![];
        let mut normal = vec![];
        let mut height = vec![];
        let mut roughness = vec![];
        let mut ao = vec![];
        let mut metallic = vec![];

        for textures in &texture_maps {
            // let mut textures =
            let or_default = |name| {
                let tex = textures
                    .get(name)
                    .unwrap_or_else(|| defaults.get(name).unwrap());

                RawImage2d::from_raw_rgba_reversed(&*tex, tex.dimensions())
            };

            albedo.push(or_default(ALBEDO));
            normal.push(or_default(NORMAL));
            height.push(or_default(HEIGHT));
            roughness.push(or_default(ROUGHNESS));
            ao.push(or_default(AO));
            metallic.push(or_default(METALLIC));
        }

        let albedo_maps =
            Texture2dArray::with_mipmaps(ctx, albedo, MipmapsOption::EmptyMipmaps).unwrap();
        let normal_maps =
            Texture2dArray::with_mipmaps(ctx, normal, MipmapsOption::EmptyMipmaps).unwrap();
        let height_maps =
            Texture2dArray::with_mipmaps(ctx, height, MipmapsOption::EmptyMipmaps).unwrap();
        let roughness_maps =
            Texture2dArray::with_mipmaps(ctx, roughness, MipmapsOption::EmptyMipmaps).unwrap();
        let ao_maps = Texture2dArray::with_mipmaps(ctx, ao, MipmapsOption::EmptyMipmaps).unwrap();
        let metallic_maps =
            Texture2dArray::with_mipmaps(ctx, metallic, MipmapsOption::EmptyMipmaps).unwrap();

        unsafe {
            albedo_maps.generate_mipmaps();
            normal_maps.generate_mipmaps();
            height_maps.generate_mipmaps();
            roughness_maps.generate_mipmaps();
            ao_maps.generate_mipmaps();
            metallic_maps.generate_mipmaps();
        }

        GraphicsData {
            albedo_maps,
            normal_maps,
            height_maps,
            roughness_maps,
            ao_maps,
            metallic_maps,
            terrain_meshes: Default::default(),
        }
    }

    pub fn iter_terrain(&self) -> impl Iterator<Item = (ChunkPos, &MeshPair<TerrainVertex>)> + '_ {
        self.terrain_meshes.iter().map(|(&k, v)| (k, &v.0))
    }

    pub fn update<F: Facade>(&mut self, ctx: &F, center: ChunkPos) {
        for (_, &mut (ref mut terrain, ref mut liquid)) in &mut self.terrain_meshes {
            terrain.upload(ctx);
            liquid.upload(ctx);
        }
    }
}
