use nalgebra::Vector3;
use num_traits::{One, Zero};
use std::ops::Neg;

// pub mod error;
pub mod world;

pub mod audio;
pub mod components;
pub mod input;
pub mod job;
pub mod loader;
pub mod physics;
pub mod render;
pub mod resources;

pub mod prelude {
    pub use super::{
        components as comp, job, resources as res,
        world::{
            block::{self, BlockId},
            chunk, BlockPos, Chunk, ChunkPos, VoxelWorld, WorldPos,
        },
    };
    pub use crate::util;
    pub use nalgebra::{
        self as na, Matrix3, Matrix4, Point1, Point2, Point3, Vector2, Vector3, Vector4,
    };
    pub use shrev::EventChannel;
    pub use specs::prelude::*;
}

#[repr(u8)]
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub enum Axis {
    X = 0,
    Y = 1,
    Z = 2,
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
    pub fn facing_positive(&self) -> bool {
        match self {
            Side::Top | Side::Right | Side::Front => true,
            _ => false,
        }
    }

    pub fn normal<S: nalgebra::Scalar + One + Zero + Neg<Output = S>>(&self) -> Vector3<S> {
        match *self {
            Side::Top => Vector3::new(S::zero(), S::one(), S::zero()),
            Side::Bottom => Vector3::new(S::zero(), -S::one(), S::zero()),
            Side::Right => Vector3::new(S::one(), S::zero(), S::zero()),
            Side::Left => Vector3::new(-S::one(), S::zero(), S::zero()),
            Side::Front => Vector3::new(S::zero(), S::zero(), S::one()),
            Side::Back => Vector3::new(S::zero(), S::zero(), -S::one()),
        }
    }

    /// take coordinates (u, v, l) where (u, v) is parallel to this face and
    /// convert it to a relative xyz coord
    pub fn uvl_to_xyz(&self, u: i32, v: i32, l: i32) -> Vector3<i32> {
        let mut vec = Vector3::new(0, 0, 0);
        let axis: Axis = (*self).into();
        let l = if self.facing_positive() { l } else { -l };
        vec[axis as usize % 3] = l;
        vec[(axis as usize + 1) % 3] = u;
        vec[(axis as usize + 2) % 3] = v;
        vec
    }
}

impl From<Side> for Axis {
    fn from(side: Side) -> Self {
        match side {
            Side::Left | Side::Right => Axis::X,
            Side::Top | Side::Bottom => Axis::Y,
            Side::Front | Side::Back => Axis::Z,
        }
    }
}
