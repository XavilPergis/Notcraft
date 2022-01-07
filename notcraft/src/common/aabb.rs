use crate::common::{math::*, util, vector};

use super::transform::Transform;

#[rustfmt::skip]
fn spans_overlap(amin: f32, amax: f32, bmin: f32, bmax: f32) -> bool {
    util::is_between(bmin, amin, amax) || util::is_between(amin, bmin, bmax) ||
    util::is_between(bmax, amin, amax) || util::is_between(amax, bmin, bmax)
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Aabb {
    pub min: Point3<f32>,
    pub max: Point3<f32>,
}

impl Aabb {
    pub fn with_dimensions(dims: Vector3<f32>) -> Self {
        let half_dims = dims / 2.0;
        Aabb {
            min: Point3::from(-half_dims),
            max: Point3::from(half_dims),
        }
    }

    #[rustfmt::skip]
    pub fn contains(&self, point: &Point3<f32>) -> bool {
        util::is_between(point.x, self.min.x, self.max.x) &&
        util::is_between(point.y, self.min.y, self.max.y) &&
        util::is_between(point.z, self.min.z, self.max.z)
    }

    #[rustfmt::skip]
    pub fn intersects(&self, other: &Aabb) -> bool {
        spans_overlap(self.min.x, self.max.x, other.min.x, other.max.x) &&
        spans_overlap(self.min.y, self.max.y, other.min.y, other.max.y) &&
        spans_overlap(self.min.z, self.max.z, other.min.z, other.max.z)
    }

    pub fn dimensions(&self) -> Vector3<f32> {
        vector![
            self.max.x - self.min.x,
            self.max.y - self.min.y,
            self.max.z - self.min.z
        ]
    }

    pub fn center(&self) -> Point3<f32> {
        self.min + self.dimensions() / 2.0
    }

    pub fn translated(&self, translation: Vector3<f32>) -> Aabb {
        Aabb {
            min: self.min + translation,
            max: self.max + translation,
        }
    }

    pub fn inflate(&self, distance: f32) -> Aabb {
        Aabb {
            min: self.min - vector![distance, distance, distance],
            max: self.max + vector![distance, distance, distance],
        }
    }

    pub fn transformed(&self, transform: &Transform) -> Aabb {
        // you can think of this as a vector based at `self.min`, with its tip in the
        // center of the AABB.
        let half_dimensions = self.dimensions() / 2.0;
        // translate our center to the new center
        let center = self.min + half_dimensions;
        let center = transform.translation * center;
        // the whole reason we couln't just add the transform's translation is because
        // scaling the AABB when its center is not at the origin would have a
        // translating sort of effect. here we just scale the "to center" vector by the
        // scale and define the new AABB as displacements from the translated center
        let corner_displacement = half_dimensions.component_mul(&transform.scale);
        Aabb {
            min: center - corner_displacement,
            max: center + corner_displacement,
        }
    }
}
