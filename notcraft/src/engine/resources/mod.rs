use cgmath::{Deg, Matrix3, Matrix4, PerspectiveFov, Point3, Vector2, Vector3};
use collision::Ray3;
use std::time::Duration;

#[derive(Copy, Clone, Debug, PartialEq, Default)]
pub struct CursorPos {
    pub x: f32,
    pub y: f32,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash, Default)]
pub struct StopGameLoop(pub bool);

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub struct ViewDistance(pub Vector3<i32>);

impl Default for ViewDistance {
    fn default() -> Self {
        ViewDistance(Vector3::new(3, 3, 3))
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Default)]
pub struct Dt(pub Duration);

impl Dt {
    pub fn as_secs(&self) -> f32 {
        self.0.as_secs() as f32 + self.0.subsec_nanos() as f32 * 1e-9
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Default)]
pub struct ActiveDirections {
    pub front: bool,
    pub back: bool,
    pub left: bool,
    pub right: bool,
    pub down: bool,
    pub up: bool,
}
