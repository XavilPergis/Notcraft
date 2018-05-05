use gl_api::layout::InternalLayout;
use cgmath::{Vector2, Vector3};

pub mod camera;
pub mod chunk_manager;
pub mod chunk;
pub mod mesh;
pub mod terrain;

#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Precomputed {
    pub side: Side,
    pub pos: Vector3<f32>,
    pub norm: Vector3<f32>,
    pub face_offset: Vector2<f32>,
    pub face: i32,
}

pub trait Voxel {
    type PerVertex: InternalLayout;
    fn has_transparency(&self) -> bool;
    fn vertex_data(&self, precomputed: Precomputed) -> Self::PerVertex;
}

/// Six sides of a cube.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub enum Side {
    /// Positive Y.
    Top,
    /// Negative Y.
    Bottom,
    /// Positive X.
    Right,
    /// Negative X.
    Left,
    /// Positive Z.
    Front,
    /// Negative Z.
    Back,
}
