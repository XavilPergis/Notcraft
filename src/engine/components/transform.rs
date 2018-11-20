use cgmath::Point3;
use cgmath::{Deg, Matrix3, Matrix4, Vector2, Vector3, Vector4};
use collision::Ray3;
use engine::systems::debug_render::{DebugSection, Shape};
use engine::world::WorldPos;
use specs::prelude::*;

#[derive(Copy, Clone, Debug, PartialEq, Component)]
#[storage(DenseVecStorage)]
pub struct Transform {
    pub position: Point3<f64>,
    pub orientation: Vector2<Deg<f64>>,
    pub scale: Vector3<f64>,
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
    pub fn as_matrix(&self) -> Matrix4<f64> {
        Matrix4::from_nonuniform_scale(self.scale.x, self.scale.y, self.scale.z)
            * Matrix4::from_angle_x(self.orientation.x)
            * Matrix4::from_angle_y(self.orientation.y)
            * Matrix4::from_translation(-::util::to_vector(self.position))
    }

    pub fn rotation_matrix(&self) -> Matrix3<f64> {
        Matrix3::from_angle_x(self.orientation.x) * Matrix3::from_angle_y(self.orientation.y)
    }

    pub fn basis_vectors(&self) -> (Vector3<f64>, Vector3<f64>) {
        let yaw = Matrix3::from_angle_y(self.orientation.y);
        let mut forward = yaw * Vector3::unit_z();
        let mut right = yaw * Vector3::unit_x();
        forward.z *= -1.0;
        right.z *= -1.0;
        (forward, right)
    }

    pub fn camera_ray(&self) -> Ray3<f64> {
        let yaw = Matrix3::from_angle_y(self.orientation.y);
        let pitch = Matrix3::from_axis_angle(yaw * Vector3::unit_x(), self.orientation.x);

        let mut forward = pitch * yaw * Vector3::unit_z();
        forward.z *= -1.0;

        Ray3::new(self.position, forward)
    }

    pub fn debug(&self, section: &mut DebugSection) {
        let basis = self.basis_vectors();
        section.draw(Shape::Line(
            5.0,
            WorldPos(self.position - Vector3::unit_y()),
            basis.0,
            Vector4::new(1.0, 0.0, 0.0, 1.0),
        ));
        section.draw(Shape::Line(
            5.0,
            WorldPos(self.position - Vector3::unit_y()),
            basis.1,
            Vector4::new(0.0, 0.0, 1.0, 1.0),
        ));
    }
}
