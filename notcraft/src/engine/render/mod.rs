use crate::engine::{
    render::mesh::{GpuMesh, Mesh},
    world::ChunkPos,
};
use cgmath::{Vector2, Vector3};
use glium::{backend::Facade, index::PrimitiveType, texture::Texture2dArray, Vertex};
use specs::prelude::*;
use std::collections::HashMap;

pub mod debug;
pub mod mesh;
pub mod mesher;
pub mod terrain;
// pub mod ui;

glium::implement_vertex!(LiquidVertex, pos, uv, normal, tex_id);
#[derive(Copy, Clone, Debug, PartialEq, Default)]
pub struct LiquidVertex {
    pub pos: [f32; 3],
    pub uv: [f32; 2],
    pub normal: [f32; 3],
    pub tex_id: i32,
}

glium::implement_vertex!(TerrainVertex, pos, uv, normal, tex_id, ao);
#[derive(Copy, Clone, Debug, PartialEq, Default)]
pub struct TerrainVertex {
    pub pos: [f32; 3],
    pub uv: [f32; 2],
    pub normal: [f32; 3],
    pub tex_id: i32,
    pub ao: f32,
}

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
    pub textures: Texture2dArray,
}

impl GraphicsData {
    pub fn new(textures: Texture2dArray) -> Self {
        GraphicsData {
            terrain_meshes: Default::default(),
            textures,
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
