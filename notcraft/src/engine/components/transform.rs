use nalgebra::{Matrix4, Point3, Rotation3, Translation3, Unit, UnitQuaternion, Vector2, Vector3};
use specs::prelude::*;

// FIXME: roll doesn't work right so we just don't do that...
fn euler_to_quat(x: f32, y: f32, z: f32) -> UnitQuaternion<f32> {
    let rx = UnitQuaternion::from_axis_angle(&Unit::new_unchecked(Vector3::new(1.0, 0.0, 0.0)), x);
    let ry = UnitQuaternion::from_axis_angle(&Unit::new_unchecked(Vector3::new(0.0, 1.0, 0.0)), y);
    // let rz = UnitQuaternion::from_axis_angle(&Unit::new_unchecked(Vector3::new(0.
    // 0, 0.0, 1.0)), z);

    ry * rx
}

fn euler_to_rotation(x: f32, y: f32, z: f32) -> Rotation3<f32> {
    euler_to_quat(x, y, z).to_rotation_matrix()
}

// NOTE: It'd be nice to split this up into different components at some point
// for better cache utilization... But for now, I don't want to deal with the
// extra complexity.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Transform {
    /// Euler angles
    pub rotation: Vector3<f32>,
    pub translation: Translation3<f32>,
    pub scale: Vector3<f32>,
}

impl Component for Transform {
    type Storage = DenseVecStorage<Self>;
}

impl Transform {
    pub fn apply(&mut self, other: &Transform) {
        self.translation.vector += other.translation.vector;
        self.rotation += other.rotation;
        self.scale.component_mul_assign(&other.scale);
    }

    pub fn translate_local(&mut self, translation: &Translation3<f32>) {
        // TODO: why do I have to negate here???
        let rotation = euler_to_quat(self.rotation.x, self.rotation.y, self.rotation.z);
        let transformed_translation = rotation * translation.vector;
        self.translation.vector += transformed_translation;
    }

    pub fn translate_global(&mut self, translation: &Translation3<f32>) {
        self.translation.vector += translation.vector.component_mul(&self.scale);
    }

    pub fn to_matrix(&self) -> Matrix4<f32> {
        // The model/world matrix takes points in local space and vonverts them to world
        // space.
        euler_to_rotation(-self.rotation.x, -self.rotation.y, -self.rotation.z)
            .to_homogeneous()
            .append_translation(&self.translation.vector)
            .prepend_nonuniform_scaling(&self.scale)
    }

    pub fn view_matrix(&self) -> Matrix4<f32> {
        // The view matrix is the inverse of the world matrix, as it "undoes" all of the
        // transformations that the world matrix did.
        //
        // We have to translate and then rotate so that the object is at the origin when
        // the rotation happens. Otherwise, it'd orbit around the origin instead of
        // rotating in-place.
        let inv_scale = Vector3::new(1.0 / self.scale.x, 1.0 / self.scale.y, 1.0 / self.scale.z);
        euler_to_rotation(self.rotation.x, self.rotation.y, self.rotation.z)
            .inverse()
            .to_homogeneous()
            .append_nonuniform_scaling(&inv_scale)
            .prepend_translation(&-self.translation.vector)
    }
}

/// Note that `axis` is really in the XZ plane and not the XY plane.
pub fn creative_flight(transform: &mut Transform, translation_xz: Vector2<f32>) {
    let lateral_rotation = euler_to_quat(0.0, transform.rotation.y, 0.0);
    let local_translation = Vector3::new(translation_xz.x, 0.0, translation_xz.y);
    let translation = Translation3::from(lateral_rotation * local_translation);
    transform.translate_global(&translation);
}

impl Default for Transform {
    fn default() -> Self {
        Transform {
            rotation: Vector3::new(0.0, 0.0, 0.0),
            translation: Translation3::from(Vector3::new(0.0, 0.0, 0.0)),
            scale: Vector3::new(1.0, 1.0, 1.0),
        }
    }
}

impl From<Point3<f32>> for Transform {
    fn from(point: Point3<f32>) -> Self {
        Transform {
            translation: Translation3::from(point.coords),
            rotation: Vector3::new(0.0, 0.0, 0.0),
            scale: Vector3::new(1.0, 1.0, 1.0),
        }
    }
}

/// Computed to-world transformation matrices.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct GlobalTransform(pub Transform);

impl Component for GlobalTransform {
    type Storage = DenseVecStorage<Self>;
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub struct Parent(pub Entity);

impl Component for Parent {
    type Storage = DenseVecStorage<Self>;
}

fn calculate_global_transform(
    entity: Entity,
    parents: &ReadStorage<'_, Parent>,
    transforms: &ReadStorage<'_, Transform>,
) -> GlobalTransform {
    let mut accum = Transform::default();

    let mut current_entity = entity;
    while let Some(transform) = transforms.get(current_entity) {
        accum.apply(transform);
        if let Some(parent) = parents.get(current_entity) {
            current_entity = parent.0;
        } else {
            break;
        }
    }

    GlobalTransform(accum)
}

#[derive(Debug)]
pub struct TransformHierarchyManager;

impl TransformHierarchyManager {
    pub fn new() -> Self {
        TransformHierarchyManager
    }
}

impl<'a> System<'a> for TransformHierarchyManager {
    type SystemData = (
        Entities<'a>,
        ReadStorage<'a, Parent>,
        ReadStorage<'a, Transform>,
        WriteStorage<'a, GlobalTransform>,
    );

    fn run(&mut self, (entities, parents, transforms, mut computed): Self::SystemData) {
        // TODO: caching
        for (entity, _transform) in (&entities, &transforms).join() {
            let global = calculate_global_transform(entity, &parents, &transforms);

            computed
                .insert(entity, global)
                .expect("Failed to insert computed transform.");
        }
    }
}
