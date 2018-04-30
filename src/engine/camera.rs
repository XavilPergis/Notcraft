use cgmath::{Deg, Matrix4, Matrix3, Vector3};

#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Camera {
    pub position: Vector3<f32>,
    pub pitch: Deg<f32>,
    pub yaw: Deg<f32>,
}

pub enum Rotation {
    AboutX(Deg<f32>),
    AboutY(Deg<f32>),
}

impl Default for Camera {
    fn default() -> Self {
        Camera {
            position: Vector3::new(0.0, 0.0, 0.0),
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

    pub fn translate(&mut self, translation: Vector3<f32>) {
        self.position += translation;
    }

    pub fn get_horizontal_look_vec(&self) -> Vector3<f32> {
        use cgmath::InnerSpace;
        let a = Matrix3::from_angle_x(self.pitch) * -Vector3::unit_z();
        let b = Matrix3::from_angle_y(self.yaw) * -Vector3::unit_z();

        Vector3::new(b.x, a.y, -1.0 * b.z).normalize()
    }

    pub fn transform_matrix(&self) -> Matrix4<f32> {
        let pitch = Matrix4::from_angle_x(self.pitch);
        let yaw = Matrix4::from_angle_y(self.yaw);
        pitch * yaw * Matrix4::from_translation(self.position)
    }
}
