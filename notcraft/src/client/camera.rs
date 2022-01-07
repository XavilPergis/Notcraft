use crate::common::{prelude::*, transform::Transform};
use bevy_ecs::system::SystemParam;
use nalgebra::{Matrix4, Perspective3, Point3};

#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Camera {
    pub projection: Perspective3<f32>,
}

#[derive(Copy, Clone, Debug, PartialEq, Default)]
pub struct ActiveCamera(pub Option<Entity>);

impl Camera {
    pub fn projection_matrix(&self) -> Matrix4<f32> {
        self.projection.into()
    }
}

impl Default for Camera {
    fn default() -> Self {
        Camera {
            projection: Perspective3::new(1.0, std::f32::consts::PI / 2.0, 0.1, 1000.0),
        }
    }
}

#[derive(SystemParam)]
pub struct CurrentCamera<'a> {
    active: Res<'a, ActiveCamera>,
    query: Query<'a, (&'static Camera, &'static Transform)>,
}

impl<'a> CurrentCamera<'a> {
    pub fn pos(&self) -> Point3<f32> {
        self.active
            .0
            .and_then(|active| self.query.get(active).ok())
            .map(|(_, transform)| Point3::from(transform.translation.vector))
            .unwrap_or(point![0.0, 0.0, 0.0])
    }

    pub fn view(&self) -> Matrix4<f32> {
        self.active
            .0
            .and_then(|active| self.query.get(active).ok())
            .and_then(|(_, transform)| transform.to_matrix().try_inverse())
            .unwrap_or_else(|| Matrix4::identity())
    }

    pub fn projection(&self, (width, height): (u32, u32)) -> Perspective3<f32> {
        let mut proj = self
            .active
            .0
            .and_then(|active| self.query.get(active).ok())
            .map(|(camera, _)| camera.projection)
            .unwrap_or_else(|| Camera::default().projection);
        proj.set_aspect(width as f32 / height as f32);
        proj
    }
}
