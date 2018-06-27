use std::ops::Neg;
use cgmath::{Zero, One, Point3};
use gl_api::layout::GlLayout;
use cgmath::{Vector2, Vector3};

pub mod camera;
pub mod chunk;
pub mod mesh;
pub mod terrain;
pub mod world;
pub mod mesher;
pub mod block;

pub type WorldPos = Point3<i32>;
pub type ChunkPos = Point3<i32>;

#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Precomputed {
    pub side: Side,
    pub pos: Vector3<f32>,
    pub norm: Vector3<f32>,
    pub face_offset: Vector2<f32>,
    pub face: i32,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, Default)]
pub struct VoxelProperties {
    pub opaque: bool,
}

pub trait Voxel: Copy + PartialEq {
    fn properties(&self) -> VoxelProperties;
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

impl Side {
    pub fn offset<S: One + Zero + Neg<Output=S>>(&self) -> Vector3<S> {
        match *self {
            Side::Top => Vector3::new(S::zero(), S::one(), S::zero()),
            Side::Bottom => Vector3::new(S::zero(), -S::one(), S::zero()),
            Side::Right => Vector3::new(S::one(), S::zero(), S::zero()),
            Side::Left => Vector3::new(-S::one(), S::zero(), S::zero()),
            Side::Front => Vector3::new(S::zero(), S::zero(), S::one()),
            Side::Back => Vector3::new(S::zero(), S::zero(), -S::one()),
        }
    }
}
