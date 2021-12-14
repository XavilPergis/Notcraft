use legion::Entity;
use nalgebra as na;

#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Camera {
    pub projection: na::Perspective3<f32>,
}

#[derive(Copy, Clone, Debug, PartialEq, Default)]
pub struct ActiveCamera(pub Option<Entity>);

impl Camera {
    pub fn projection_matrix(&self) -> na::Matrix4<f32> {
        self.projection.into()
    }
}

impl Default for Camera {
    fn default() -> Self {
        Camera {
            projection: na::Perspective3::new(1.0, std::f32::consts::PI / 2.0, 0.01, 1000.0),
        }
    }
}
