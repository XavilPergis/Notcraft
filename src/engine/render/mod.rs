use cgmath::{Vector2, Vector3};
use engine::render::{
    mesh::Mesh,
    terrain::{BlockVertex, LiquidVertex},
};
use specs::prelude::*;

pub mod debug;
pub mod mesh;
pub mod mesher;
pub mod terrain;
pub mod ui;

pub mod verts {
    use cgmath::{Vector2, Vector3};
    vertex! {
        vertex Pos {
            pos: Vector3<f32>,
        }
    }

    vertex! {
        vertex PosUv {
            pos: Vector3<f32>,
            uv: Vector2<f32>,
        }
    }

    vertex! {
        vertex PosUvNorm {
            pos: Vector3<f32>,
            normal: Vector3<f32>,
            uv: Vector2<f32>,
        }
    }

    const fn pos(x: f32, y: f32, z: f32) -> Vector3<f32> {
        Vector3 { x, y, z }
    }

    const fn uv(x: f32, y: f32) -> Vector2<f32> {
        Vector2 { x, y }
    }

    const fn puv(x: f32, y: f32, z: f32, u: f32, v: f32) -> PosUv {
        PosUv {
            pos: pos(x, y, z),
            uv: uv(u, v),
        }
    }

    pub static UV_QUAD_CW: &[PosUv] = &[
        puv(-1.0, -1.0, 0.0, 0.0, 0.0),
        puv(-1.0, 1.0, 0.0, 0.0, 1.0),
        puv(1.0, -1.0, 0.0, 1.0, 0.0),
        puv(1.0, 1.0, 0.0, 1.0, 1.0),
        puv(1.0, -1.0, 0.0, 1.0, 0.0),
        puv(-1.0, 1.0, 0.0, 0.0, 1.0),
    ];
}

#[derive(Debug, Default, Component)]
#[storage(DenseVecStorage)]
pub struct TerrainMeshes {
    pub terrain: Mesh<BlockVertex, u32>,
    pub liquid: Mesh<LiquidVertex, u32>,
}
