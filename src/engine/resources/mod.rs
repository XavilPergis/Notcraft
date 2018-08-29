use std::time::Duration;
use cgmath::{Deg, Vector3};

#[derive(Copy, Clone, Debug, PartialEq, Default)]
pub struct CursorPos {
    pub x: f64,
    pub y: f64,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash, Default)]
pub struct StopGameLoop(pub bool);

#[derive(Copy, Clone, Debug, PartialEq)]
pub struct ViewFrustum {
    pub fov: Deg<f64>,
    pub near_plane: f64,
    pub far_plane: f64,
}

#[derive(Copy, Clone, Debug, PartialEq, Default)]
pub struct FramebufferSize {
    pub x: f64,
    pub y: f64,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub struct ViewDistance(pub Vector3<i32>);

impl Default for ViewDistance {
    fn default() -> Self { ViewDistance(Vector3::new(3, 3, 3)) }
}

#[derive(Copy, Clone, Debug, PartialEq, Default)]
pub struct Dt(pub Duration);

impl Dt {
    pub fn as_secs(&self) -> f64 {
        self.0.as_secs() as f64 + self.0.subsec_nanos() as f64 * 1e-9
    }
}
