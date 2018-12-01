use cgmath::Deg;
use cgmath::Vector3;
use specs::prelude::*;

#[derive(Copy, Clone, Debug, PartialEq, Component)]
#[storage(HashMapStorage)]
pub struct LookTarget {
    pub x: Deg<f64>,
    pub y: Deg<f64>,
}

impl Default for LookTarget {
    fn default() -> Self {
        LookTarget {
            x: Deg(0.0),
            y: Deg(0.0),
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Component)]
#[storage(HashMapStorage)]
pub struct MoveDelta(pub Vector3<f64>);

impl Default for MoveDelta {
    fn default() -> Self {
        MoveDelta(Vector3::new(0.0, 0.0, 0.0))
    }
}
