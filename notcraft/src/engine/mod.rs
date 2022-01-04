use nalgebra::{vector, Vector3};
use num_traits::{One, Zero};
use std::ops::Neg;

// pub mod error;
pub mod audio;
pub mod input;
pub mod loader;
pub mod physics;
pub mod render;
pub mod transform;
pub mod world;

pub mod math {
    pub use nalgebra::{
        self as na, Matrix3, Matrix4, Point1, Point2, Point3, Vector2, Vector3, Vector4,
    };
}

pub mod prelude {
    pub use bevy_app as app;
    pub use bevy_ecs as ecs;

    pub use crate::util;

    pub use app::prelude::*;
    pub use bevy_core::prelude::*;
    pub use ecs::prelude::*;

    pub type Result<T, E = anyhow::Error> = std::result::Result<T, E>;
    pub use anyhow::{anyhow, bail};
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
