use nalgebra::{vector, Vector3};
use num_traits::{One, Zero};
use serde::{Deserialize, Serialize};
use std::ops::Neg;

pub mod aabb;
pub mod codec;
pub mod physics;
pub mod transform;
pub mod util;
pub mod world;

pub mod debug;

pub mod math {
    pub use nalgebra::{Matrix3, Matrix4, Point1, Point2, Point3, Vector2, Vector3, Vector4};
}

pub mod prelude {
    pub use super::util;

    pub use bevy_app::prelude::*;
    pub use bevy_core::prelude::*;
    pub use bevy_ecs::prelude::*;

    pub type Result<T, E = anyhow::Error> = std::result::Result<T, E>;
    pub use anyhow::{anyhow, bail};

    pub use nalgebra::{point, vector};
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

    pub fn enumerate<F>(mut func: F)
    where
        F: FnMut(Side),
    {
        func(Side::Right);
        func(Side::Left);
        func(Side::Top);
        func(Side::Bottom);
        func(Side::Front);
        func(Side::Back);
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

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct Faces<T> {
    pub top: T,
    pub bottom: T,
    pub right: T,
    pub left: T,
    pub front: T,
    pub back: T,
}

impl<T> Faces<T> {
    pub fn map<U, F>(self, mut func: F) -> Faces<U>
    where
        F: FnMut(T) -> U,
    {
        Faces {
            top: func(self.top),
            bottom: func(self.bottom),
            left: func(self.left),
            right: func(self.right),
            front: func(self.front),
            back: func(self.back),
        }
    }

    pub fn all<F>(&self, mut func: F) -> bool
    where
        F: FnMut(&T) -> bool,
    {
        func(&self.top)
            && func(&self.bottom)
            && func(&self.left)
            && func(&self.right)
            && func(&self.front)
            && func(&self.back)
    }

    pub fn any<F>(&self, mut func: F) -> bool
    where
        F: FnMut(&T) -> bool,
    {
        func(&self.top)
            || func(&self.bottom)
            || func(&self.left)
            || func(&self.right)
            || func(&self.front)
            || func(&self.back)
    }
}

impl<T> std::ops::Index<Side> for Faces<T> {
    type Output = T;

    fn index(&self, index: Side) -> &Self::Output {
        match index {
            Side::Top => &self.top,
            Side::Bottom => &self.bottom,
            Side::Right => &self.right,
            Side::Left => &self.left,
            Side::Front => &self.front,
            Side::Back => &self.back,
        }
    }
}
