use nalgebra::{vector, Vector3};
use num_traits::{One, Zero};
use std::ops::Neg;

// pub mod error;
pub mod world;

pub mod audio;
pub mod input;
pub mod loader;
pub mod physics;
pub mod render;
pub mod transform;

use std::time::Duration;

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash, Default)]
pub struct StopGameLoop(pub bool);

#[derive(Copy, Clone, Debug, PartialEq, Default)]
pub struct Dt(pub Duration);

impl Dt {
    pub fn as_secs(&self) -> f32 {
        self.0.as_secs() as f32 + self.0.subsec_nanos() as f32 * 1e-9
    }
}

pub mod math {
    pub use nalgebra::{
        self as na, Matrix3, Matrix4, Point1, Point2, Point3, Vector2, Vector3, Vector4,
    };
}

// pub mod prelude {
//     pub use super::world::{
//         block::{self, BlockId},
//         chunk, BlockPos, Chunk, ChunkPos, VoxelWorld, WorldPos,
//     };
//     pub use crate::util;
// }

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
            Side::Top => vector!(S::zero(), S::one(), S::zero()),
            Side::Bottom => vector!(S::zero(), -S::one(), S::zero()),
            Side::Right => vector!(S::one(), S::zero(), S::zero()),
            Side::Left => vector!(-S::one(), S::zero(), S::zero()),
            Side::Front => vector!(S::zero(), S::zero(), S::one()),
            Side::Back => vector!(S::zero(), S::zero(), -S::one()),
        }
    }

    pub fn axis(&self) -> Axis {
        match self {
            Side::Left | Side::Right => Axis::X,
            Side::Top | Side::Bottom => Axis::Y,
            Side::Front | Side::Back => Axis::Z,
        }
    }

    /// take coordinates (u, v, l) where (u, v) is parallel to this face and
    /// convert it to a relative xyz coord
    pub fn uvl_to_xyz<S: nalgebra::Scalar + Copy + Zero + Neg<Output = S>>(
        &self,
        u: S,
        v: S,
        l: S,
    ) -> Vector3<S> {
        let axis = self.axis();
        let l = [-l, l][self.facing_positive() as usize];

        let mut vec = vector![S::zero(), S::zero(), S::zero()];
        vec[axis as usize % 3] = l;
        vec[(axis as usize + 1) % 3] = u;
        vec[(axis as usize + 2) % 3] = v;
        vec
    }
}
