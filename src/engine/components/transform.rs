use specs::prelude::*;
use cgmath::{Vector3, Vector2, Deg, Matrix4, Matrix3, Zero};

#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Transform {
    pub position: Vector3<f64>,
    pub orientation: Vector2<Deg<f64>>,
    pub scale: Vector3<f64>,
}

impl Default for Transform {
    fn default() -> Self {
        Transform {
            position: Vector3::zero(),
            orientation: Vector2::new(Deg(0.0), Deg(0.0)),
            scale: Vector3::new(1.0, 1.0, 1.0),
        }
    }
}

impl Component for Transform {
    type Storage = DenseVecStorage<Self>;
}

impl Transform {
    pub fn as_matrix(&self) -> Matrix4<f64> {
        Matrix4::from_nonuniform_scale(self.scale.x, self.scale.y, self.scale.z)
        * Matrix4::from_angle_x(self.orientation.x)
        * Matrix4::from_angle_y(self.orientation.y)
        * Matrix4::from_translation(self.position)
    }

    pub fn basis_vectors(&self) -> (Vector3<f64>, Vector3<f64>) {
        let yaw = Matrix3::from_angle_y(self.orientation.y);
        let mut forward = yaw * -Vector3::unit_z();
        let mut right = yaw * Vector3::unit_x();
        forward.z *= -1.0;
        right.z *= -1.0;
        (forward, right)
    }
}
