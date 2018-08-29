use cgmath::Vector3;
use specs::prelude::*;
use cgmath::Deg;

#[derive(Copy, Clone, Debug, PartialEq)]
pub struct LookTarget {
    pub x: Deg<f64>,
    pub y: Deg<f64>
}

impl Default for LookTarget {
    fn default() -> Self {
        LookTarget {
            x: Deg(0.0),
            y: Deg(0.0),
        }
    }
}

impl Component for LookTarget {
    type Storage = HashMapStorage<Self>;
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub struct MoveDelta(pub Vector3<f64>);

impl Default for MoveDelta {
    fn default() -> Self {
        MoveDelta(Vector3::new(0.0, 0.0, 0.0))
    }
}

impl Component for MoveDelta {
    type Storage = HashMapStorage<Self>;
}

#[derive(Copy, Clone, Debug, PartialEq, Default)]
pub struct ActiveDirections {
    pub front: bool,
    pub back: bool,
    pub left: bool,
    pub right: bool,
    pub down: bool,
    pub up: bool,
}

impl Component for ActiveDirections {
    type Storage = HashMapStorage<Self>;
}
