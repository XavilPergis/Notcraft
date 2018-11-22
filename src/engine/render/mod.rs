use cgmath::Vector3;

pub mod debug;
pub mod mesh;
pub mod mesher;
pub mod terrain;

type Quad = [Vector3<f64>; 4];

#[derive(Copy, Clone, Debug, PartialEq, Deserialize)]
pub struct BlockQuad {
    positions: Quad,
    /// index into the model's name vec.
    texture_index: usize,
}

#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct BlockModel {
    texture_names: Vec<String>,
}
