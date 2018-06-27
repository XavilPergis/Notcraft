use cgmath::{Deg, Matrix4, Matrix3, Point3, Vector3};

#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Camera {
    pub position: Point3<f64>,
    pub pitch: Deg<f64>,
    pub yaw: Deg<f64>,
}

pub enum Rotation {
    AboutX(Deg<f64>),
    AboutY(Deg<f64>),
}

impl Default for Camera {
    fn default() -> Self {
        Camera {
            position: Point3::new(0.0, 0.0, 0.0),
            pitch: Deg(0.0),
            yaw: Deg(0.0),
        }
    }
}

impl Camera {
    pub fn rotate(&mut self, rotation: Rotation) {
        match rotation {
            Rotation::AboutX(angle) => {
                self.pitch = ::util::clamp(self.pitch + angle, Deg(-90.0), Deg(90.0));
            },
            Rotation::AboutY(angle) => {
                self.yaw += angle;
            },
        }
    }

    pub fn translate(&mut self, translation: Vector3<f64>) {
        self.position += translation;
    }

    pub fn get_look_vec(&self) -> Vector3<f64> {
        use cgmath::{Angle, InnerSpace};
        let a = Matrix3::from_angle_x(self.pitch) * -Vector3::unit_z();
        let b = Matrix3::from_angle_y(self.yaw) * -Vector3::unit_z();
        let hs = self.pitch.cos();

        Vector3::new(hs * b.x, a.y, -1.0 * hs * b.z).normalize()
    }

    /// Get the camera (forward, right) vectors without a pitch component
    pub fn get_spin_vecs(&self) -> (Vector3<f64>, Vector3<f64>) {
        let yaw = Matrix3::from_angle_y(self.yaw);
        let mut forward = yaw * -Vector3::unit_z();
        let mut right = yaw * Vector3::unit_x();
        forward.z *= -1.0;
        right.z *= -1.0;
        (forward, right)
    }

    pub fn transform_matrix(&self) -> Matrix4<f64> {
        let pitch = Matrix4::from_angle_x(self.pitch);
        let yaw = Matrix4::from_angle_y(self.yaw);
        pitch * yaw * Matrix4::from_translation(-::util::to_vector(self.position))
    }
}
