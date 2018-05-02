use gl_api::error::GlResult;
use cgmath::Vector3;
use gl_api::buffer::UsageType;
use engine::mesh::{IndexingType, Mesh};
use gl_api::layout::InternalLayout;

pub const CHUNK_SIZE: usize = 32;
const CHUNK_VOLUME: usize = CHUNK_SIZE*CHUNK_SIZE*CHUNK_SIZE;

vertex! {
    vertex ChunkVertex {
        pos: Vector3<f32>,
        norm: Vector3<f32>,
        color: Vector3<f32>,
        face: i32,
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct Chunk<T> {
    pub(crate) x: i32, pub(crate) y: i32, pub(crate) z: i32,
    solid_count: usize,
    pub(crate) data: Box<[T]>,
}

pub trait Voxel {
    fn has_transparency(&self) -> bool;
    fn color(&self) -> Vector3<f32>;
}

impl<T: Voxel> Chunk<T> {
    pub fn new(x: i32, y: i32, z: i32, voxels: Vec<T>) -> Self {
        let solid_count = voxels.iter().filter(|voxel| !voxel.has_transparency()).count();
        Chunk { x, y, z, solid_count, data: voxels.into_boxed_slice() }
    }
}

impl<T> Chunk<T> {
    pub fn get(&self, x: usize, y: usize, z: usize) -> Option<&T> {
        if x >= CHUNK_SIZE || y >= CHUNK_SIZE || z >= CHUNK_SIZE {
            None
        } else {
            Some(&self.data[CHUNK_SIZE * CHUNK_SIZE * z + CHUNK_SIZE * y + x])
        }
    }

    pub fn size(&self) -> usize { self.data.len() }

    pub fn in_chunk_bounds(&self, x: isize, y: isize, z: isize) -> bool {
        const SIZE_I: isize = CHUNK_SIZE as isize;
        x < SIZE_I && y < SIZE_I && z < SIZE_I && x >= 0 && y >= 0 && z >= 0
    }

    pub fn all_transparent(&self) -> bool {
        self.solid_count == 0
    }
}

use std::ops::{Index, IndexMut};

impl<T> Index<(usize, usize, usize)> for Chunk<T> {
    type Output = T;
    fn index(&self, index: (usize, usize, usize)) -> &T {
        &self.data[CHUNK_SIZE * CHUNK_SIZE * index.2 + CHUNK_SIZE * index.1 + index.0]
    }
}

impl<T> IndexMut<(usize, usize, usize)> for Chunk<T> {
    fn index_mut(&mut self, index: (usize, usize, usize)) -> &mut T {
        &mut self.data[CHUNK_SIZE * CHUNK_SIZE * index.2 + CHUNK_SIZE * index.1 + index.0]
    }
}

pub trait Mesher<V, I> {
    fn gen_vertex_data(self) -> (Vec<V>, Vec<I>);
    fn gen_mesh(self) -> GlResult<Mesh<V, I>> where V: InternalLayout, I: IndexingType, Self: Sized {
        let (vertices, indices) = self.gen_vertex_data();
        let mut mesh = Mesh::new()?;
        mesh.upload(&vertices, &indices, UsageType::Static)?;
        Ok(mesh)
    }
}

// NOTE: You probably should never debug print this, unless CHUNK_SIZE is pretty small.
// Otherwise, your terminal will be spitting out text for a solid 3 minutes straight.
pub struct CullMesher<'c, T: Voxel + 'c> {
    chunk: &'c Chunk<T>,
    top: &'c Chunk<T>,
    bottom: &'c Chunk<T>,
    left: &'c Chunk<T>,
    right: &'c Chunk<T>,
    front: &'c Chunk<T>,
    back: &'c Chunk<T>,
    vertices: Vec<ChunkVertex>,
    indices: Vec<u32>,
}

use super::Side;

impl<'c, T: Voxel + 'c> CullMesher<'c, T> {
    pub fn new(chunk: &'c Chunk<T>,
               top: &'c Chunk<T>,
               bottom: &'c Chunk<T>,
               left: &'c Chunk<T>,
               right: &'c Chunk<T>,
               front: &'c Chunk<T>,
               back: &'c Chunk<T>,) -> Self {
        CullMesher {
            chunk,
            top,
            bottom,
            left,
            right,
            front,
            back,
            vertices: Vec::with_capacity(CHUNK_VOLUME),
            indices: Vec::with_capacity(CHUNK_VOLUME),
        }
    }

    fn add_face(&mut self, side: Side, pos: Vector3<isize>, color: Vector3<f32>) {
        let index_len = self.vertices.len() as u32;
        let cx = CHUNK_SIZE as f32 * self.chunk.x as f32;
        let cy = CHUNK_SIZE as f32 * self.chunk.y as f32;
        let cz = CHUNK_SIZE as f32 * self.chunk.z as f32;
        let x = pos.x as f32;
        let y = pos.y as f32;
        let z = pos.z as f32;

        macro_rules! offset_arr {
            ($offset:expr, [$($item:expr),*]) => {[$($offset + $item),*]}
        }

        macro_rules! face {
            (ind [$($index:expr),*],
             vert [$($vx:expr, $vy:expr, $vz:expr);*],
             norm $nx:expr,$ny:expr,$nz:expr;,
             face $face:expr) => {{
                self.indices.extend(&offset_arr!(index_len, [$($index),*]));
                self.vertices.extend(&[$(ChunkVertex {
                    pos: Vector3::new(cx+x+$vx as f32, cy+y+$vy as f32, cz+z+$vz as f32),
                    norm: Vector3::new($nx as f32, $ny as f32, $nz as f32),
                    color, face: $face
                },)*]);
            }}
        }

        match side {
            Side::Front  => face! { ind [0,1,2,3,2,1], vert [0,1,1; 1,1,1; 0,0,1; 1,0,1], norm 0,0, 1;, face 0 },
            Side::Back   => face! { ind [0,2,1,3,1,2], vert [0,1,0; 1,1,0; 0,0,0; 1,0,0], norm 0,0,-1;, face 0 },
            Side::Top    => face! { ind [0,1,2,3,2,1], vert [0,1,0; 1,1,0; 0,1,1; 1,1,1], norm 0, 1,0;, face 1 },
            Side::Bottom => face! { ind [0,2,1,3,1,2], vert [0,0,0; 1,0,0; 0,0,1; 1,0,1], norm 0,-1,0;, face 1 },
            Side::Left   => face! { ind [0,1,2,3,2,1], vert [0,1,0; 0,1,1; 0,0,0; 0,0,1], norm -1,0,0;, face 2 },
            Side::Right  => face! { ind [0,2,1,3,1,2], vert [1,1,0; 1,1,1; 1,0,0; 1,0,1], norm 1, 0,0;, face 2 },
        }
    }

    fn needs_face(&self, x: isize, y: isize, z: isize) -> bool {
        let (ux, uy, uz) = (x as usize, y as usize, z as usize);
        let in_bounds = self.chunk.in_chunk_bounds(x, y, z);
        if in_bounds {
            self.chunk.get(ux, uy, uz).unwrap().has_transparency()
        } else {
            if      x >= CHUNK_SIZE as isize { self.right.get(0, uy, uz).unwrap().has_transparency() }
            else if x < 0 as isize { self.left.get(CHUNK_SIZE - 1, uy, uz).unwrap().has_transparency() }
            else if y >= CHUNK_SIZE as isize { self.top.get(ux, 0, uz).unwrap().has_transparency() }
            else if y < 0 as isize { self.bottom.get(ux, CHUNK_SIZE - 1, uz).unwrap().has_transparency() }
            else if z >= CHUNK_SIZE as isize { self.front.get(ux, uy, 0).unwrap().has_transparency() }
            else if z < 0 as isize { self.back.get(ux, uy, CHUNK_SIZE - 1).unwrap().has_transparency() }
            else { false }
        }
    }
}

impl<'c, T: Voxel + 'c> Mesher<ChunkVertex, u32> for CullMesher<'c, T> {
    fn gen_vertex_data(mut self) -> (Vec<ChunkVertex>, Vec<u32>) {
        for i in 0..(CHUNK_SIZE as isize) {
            for j in 0..(CHUNK_SIZE as isize) {
                for k in 0..(CHUNK_SIZE as isize) {
                    let block = self.chunk.get(i as usize, j as usize, k as usize).unwrap();
                    if block.has_transparency() { continue; }
                    let pos = Vector3::new(i, j, k);

                    if self.needs_face(i, j, k+1) { self.add_face(Side::Front, pos, block.color()) }
                    if self.needs_face(i, j, k-1) { self.add_face(Side::Back, pos, block.color()) }
                    if self.needs_face(i, j+1, k) { self.add_face(Side::Top, pos, block.color()) }
                    if self.needs_face(i, j-1, k) { self.add_face(Side::Bottom, pos, block.color()) }
                    if self.needs_face(i+1, j, k) { self.add_face(Side::Right, pos, block.color()) }
                    if self.needs_face(i-1, j, k) { self.add_face(Side::Left, pos, block.color()) }
                }
            }
        }
        (self.vertices, self.indices)
    }
}

// pub struct GreedyMesher<'c, T: Voxel + 'c> {
//     chunk: &'c Chunk<T>,
//     visited_mask: Box<[bool]>,
//     vertices: Vec<ChunkVertex>,
//     indices: Vec<u32>,
// }

// impl<'c, T: Voxel + 'c> GreedyMesher<'c, T> {
//     fn new(chunk: &'c Chunk<T>) -> Self {
//         GreedyMesher {
//             chunk,
//             visited_mask: vec![false; chunk.size()].into_boxed_slice(),
//             vertices: Vec::new(),
//             indices: Vec::new(),
//         }
//     }

//     fn try_expand_right(&self, x: usize, y: usize, z: usize) -> Quad where T: PartialEq {
//         // Infinite iterator, but chunk.get should return None in
//         // less than CHUNK_SIZE + 1 iterations
//         for x_off in 0.. {
//             let cur = self.chunk.get(x+x_off, y, z);
//             let next = self.chunk.get(x+x_off+1, y, z);
//             let quad = Quad { x, y, w: x_off + 1, h: 1 };
//             match next {
//                 // Having no next face means we are at a chunk border and can't
//                 // expand the quad more anyways
//                 None => return quad,
//                 // UNWRAP: next having a value means that cur also has a value.
//                 Some(voxel) => if cur.unwrap() != voxel { return quad; }
//             }
//         }

//         // Should always return before this point
//         unreachable!()
//     }

//     fn try_expand_down(&self, quad: Quad, z: usize) -> Quad where T: PartialEq {
//         assert!(quad.h >= 1);
//         for y_off in 0.. {
//             let row = self.chunk.slice_row(quad.x..quad.x+quad.w, quad.y+y_off, z);
//             let next_row = self.chunk.slice_row(quad.x..quad.x+quad.w, quad.y+y_off+1, z);
//             // Quad to return if we can't go further down in this iteration
//             let quad = Quad { x: quad.x, y: quad.y, w: quad.w, h: quad.h + y_off };
//             if let Some(next_row) = next_row {
//                 if next_row.iter().zip(row.unwrap()).any(|(a, b)| a != b) {
//                     return quad;
//                 }
//             } else {
//                 return quad;
//             }
//         }

//         unreachable!()
//     }
// }

// struct Quad {
//     x: usize,
//     y: usize,
//     w: usize,
//     h: usize,
// }

// impl<'c, T: Voxel + 'c> Mesher<T, ChunkVertex, u32> for GreedyMesher<'c, T> {
//     fn generate_mesh(&mut self) -> Mesh<ChunkVertex, u32> {
//         unimplemented!()
//     }
// }