use cgmath::PerspectiveFov;
use collision::Ray3;
use engine::prelude::*;

#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Camera {
    pub position: Point3<f64>,
    pub orientation: Vector2<Deg<f64>>,
    pub projection: PerspectiveFov<f64>,
}

impl Camera {
    pub fn view_matrix(&self) -> Matrix4<f64> {
        Matrix4::from_angle_x(self.orientation.x)
            * Matrix4::from_angle_y(self.orientation.y)
            * Matrix4::from_translation(-::util::to_vector(self.position))
    }

    pub fn projection_matrix(&self) -> Matrix4<f64> {
        self.projection.into()
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
}

impl Default for Camera {
    fn default() -> Self {
        Camera {
            position: Point3::new(0.0, 0.0, 0.0),
            orientation: Vector2::new(Deg(0.0), Deg(0.0)),
            projection: PerspectiveFov {
                fovy: Deg(80.0).into(),
                aspect: 1.0,
                near: 0.001,
                far: 1000.0,
            },
        }
    }
}
