use cgmath::{One, Vector3, Zero};
use std::ops::Neg;

pub mod mesh;
pub mod terrain;
pub mod world;

pub mod components;
pub mod resources;
pub mod systems;

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
    pub fn offset<S: One + Zero + Neg<Output = S>>(&self) -> Vector3<S> {
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
