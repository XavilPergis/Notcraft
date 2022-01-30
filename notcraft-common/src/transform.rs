use nalgebra::{vector, Matrix4, Point3, Translation3, UnitQuaternion, Vector3};

#[derive(Copy, Clone, Debug, PartialEq)]
pub struct EulerAngles {
    pub pitch: f32,
    pub yaw: f32,
    pub roll: f32,
}

impl EulerAngles {
    pub fn new(pitch: f32, yaw: f32, roll: f32) -> Self {
        Self { pitch, yaw, roll }
    }

    pub fn to_quaternion(&self) -> UnitQuaternion<f32> {
        UnitQuaternion::from_euler_angles(self.pitch, self.yaw, self.roll)
    }
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Transform {
    pub translation: Translation3<f32>,
    pub rotation: EulerAngles,
    pub scale: Vector3<f32>,
}

impl Transform {
    pub fn to(point: Point3<f32>) -> Self {
        Self {
            translation: Translation3::from(point),
            ..Default::default()
        }
    }

    pub fn translate_local(&mut self, translation: Vector3<f32>) {
        let transformed_translation = self.rotation.to_quaternion() * translation;
        self.translation.vector += transformed_translation;
    }

    pub fn translate_global(&mut self, translation: Vector3<f32>) {
        self.translation.vector += translation.component_mul(&self.scale);
    }

    pub fn translated(&self, translation: &Vector3<f32>) -> Transform {
        Transform {
            translation: Translation3::from(
                self.translation.vector + translation.component_mul(&self.scale),
            ),
            ..*self
        }
    }

    pub fn pos(&self) -> Point3<f32> {
        self.translation.vector.into()
    }

    pub fn to_matrix(&self) -> Matrix4<f32> {
        // The model/world matrix takes points in local space and converts them to world
        // space.
        self.rotation
            .to_quaternion()
            .to_homogeneous()
            .append_translation(&self.translation.vector)
            .prepend_nonuniform_scaling(&self.scale)
    }
}

impl Default for Transform {
    fn default() -> Self {
        Transform {
            translation: Translation3::from(vector!(0.0, 0.0, 0.0)),
            rotation: EulerAngles::new(0.0, 0.0, 0.0),
            scale: vector!(1.0, 1.0, 1.0),
        }
    }
}

impl From<Point3<f32>> for Transform {
    fn from(point: Point3<f32>) -> Self {
        Transform {
            translation: Translation3::from(point.coords),
            rotation: EulerAngles::new(0.0, 0.0, 0.0),
            scale: vector!(1.0, 1.0, 1.0),
        }
    }
}
