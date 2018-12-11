use cgmath::{Deg, Matrix3, Matrix4, Point3, Vector2, Vector3};
use specs::prelude::*;

#[derive(Copy, Clone, Debug, PartialEq, Component)]
#[storage(DenseVecStorage)]
pub struct Transform {
    pub position: Point3<f32>,
    pub orientation: Vector2<Deg<f32>>,
    pub scale: Vector3<f32>,
}

impl Default for Transform {
    fn default() -> Self {
        Transform {
            position: Point3::new(0.0, 0.0, 0.0),
            orientation: Vector2::new(Deg(0.0), Deg(0.0)),
            scale: Vector3::new(1.0, 1.0, 1.0),
        }
    }
}

impl Transform {
    pub fn with_position(self, position: Point3<f32>) -> Self {
        Transform { position, ..self }
    }

    pub fn model_matrix(&self) -> Matrix4<f32> {
        Matrix4::from_nonuniform_scale(self.scale.x, self.scale.y, self.scale.z)
            * Matrix4::from_angle_x(self.orientation.x)
            * Matrix4::from_angle_y(self.orientation.y)
            * Matrix4::from_translation(crate::util::to_vector(self.position))
    }

    pub fn rotation_matrix(&self) -> Matrix3<f32> {
        Matrix3::from_angle_x(self.orientation.x) * Matrix3::from_angle_y(self.orientation.y)
    }
}
