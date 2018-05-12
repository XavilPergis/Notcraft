use engine::ChunkPos;
use cgmath::Point3;
use engine::Precomputed;
use engine::Voxel;
use cgmath::Vector2;
use gl_api::error::GlResult;
use cgmath::Vector3;
use gl_api::buffer::UsageType;
use engine::mesh::{IndexingType, Mesh};
use gl_api::layout::InternalLayout;

pub const CHUNK_SIZE: usize = 64;
const CHUNK_VOLUME: usize = CHUNK_SIZE*CHUNK_SIZE*CHUNK_SIZE;

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct Chunk<T> {
    crate data: Box<[T]>,
}

impl<T: Voxel> Chunk<T> {
    pub fn new(voxels: Vec<T>) -> Self {
        Chunk { data: voxels.into_boxed_slice() }
    }
}

impl<T> Chunk<T> {
    pub fn get(&self, pos: Point3<i32>) -> Option<&T> {
        if self.in_chunk_bounds(pos) {
            let Point3 { x, y, z }: Point3<usize> = pos.cast().unwrap();
            Some(&self.data[CHUNK_SIZE * CHUNK_SIZE * y + CHUNK_SIZE * z + x])
        } else { None }
    }

    pub fn size(&self) -> usize { self.data.len() }

    pub fn in_chunk_bounds(&self, pos: Point3<i32>) -> bool {
        const SIZE: i32 = CHUNK_SIZE as i32;
        pos.x < SIZE && pos.y < SIZE && pos.z < SIZE && pos.x >= 0 && pos.y >= 0 && pos.z >= 0
    }

    pub fn get_mut(&mut self, pos: Point3<i32>) -> Option<&mut T> {
        if self.in_chunk_bounds(pos) {
            let Point3 { x, y, z }: Point3<usize> = pos.cast().unwrap();
            Some(&mut self.data[CHUNK_SIZE * CHUNK_SIZE * y + CHUNK_SIZE * z + x])
        } else { None }
    }
}

use std::ops::{Index, IndexMut};

impl<T> Index<(usize, usize, usize)> for Chunk<T> {
    type Output = T;
    fn index(&self, index: (usize, usize, usize)) -> &T {
        &self.data[CHUNK_SIZE * CHUNK_SIZE * index.1 + CHUNK_SIZE * index.2 + index.0]
    }
}

impl<T> IndexMut<(usize, usize, usize)> for Chunk<T> {
    fn index_mut(&mut self, index: (usize, usize, usize)) -> &mut T {
        &mut self.data[CHUNK_SIZE * CHUNK_SIZE * index.1 + CHUNK_SIZE * index.2 + index.0]
    }
}

pub trait Mesher<V, I> {
    fn gen_vertex_data(self) -> (Vec<V>, Vec<I>);
    fn gen_mesh(self) -> GlResult<Mesh<V, I>> where V: InternalLayout, I: IndexingType, Self: Sized {
        let (vertices, indices) = self.gen_vertex_data();
        let mut mesh = Mesh::new()?;
        mesh.upload(&vertices, &indices, UsageType::StaticDraw)?;
        Ok(mesh)
    }
}

pub struct Neighborhood<T> {
    pub top: T,
    pub bottom: T,
    pub left: T,
    pub right: T,
    pub front: T,
    pub back: T,
}

// NOTE: You probably should never debug print this, unless CHUNK_SIZE is pretty small.
// Otherwise, your terminal will be spitting out text for a solid 3 minutes straight.
pub struct CullMesher<'c, T: Voxel + 'c> {
    pos: ChunkPos,
    chunk: &'c Chunk<T>,
    neighbors: Neighborhood<&'c Chunk<T>>,
    vertices: Vec<T::PerVertex>,
    indices: Vec<u32>,
}

use super::Side;

impl<'c, T: Voxel + 'c> CullMesher<'c, T> {
    pub fn new(pos: ChunkPos,
               chunk: &'c Chunk<T>,
               top: &'c Chunk<T>,
               bottom: &'c Chunk<T>,
               left: &'c Chunk<T>,
               right: &'c Chunk<T>,
               front: &'c Chunk<T>,
               back: &'c Chunk<T>,) -> Self {
        CullMesher {
            pos,
            chunk,
            neighbors: Neighborhood { top, bottom, left, right, front, back },
            vertices: Vec::with_capacity(CHUNK_VOLUME),
            indices: Vec::with_capacity(CHUNK_VOLUME),
        }
    }

    fn add_face(&mut self, side: Side, pos: Point3<i32>, voxel: &T) {
        let index_len = self.vertices.len() as u32;
        let cx = CHUNK_SIZE as f32 * self.pos.x as f32;
        let cy = CHUNK_SIZE as f32 * self.pos.y as f32;
        let cz = CHUNK_SIZE as f32 * self.pos.z as f32;
        let x = pos.x as f32;
        let y = pos.y as f32;
        let z = pos.z as f32;

        macro_rules! offset_arr {
            ($offset:expr, [$($item:expr),*]) => {[$($offset + $item),*]}
        }

        macro_rules! face { 
            (side $side:ident,
             ind [$($index:expr),*],
             vert [$($vx:expr, $vy:expr, $vz:expr);*],
             off [$($ou:expr, $ov:expr);*],
             norm $nx:expr,$ny:expr,$nz:expr;,
             face $face:expr) => {{
                self.indices.extend(&offset_arr!(index_len, [$($index),*]));
                $(self.vertices.push(voxel.vertex_data(Precomputed {
                    side: Side::$side,
                    face_offset: Vector2::new($ou as f32, $ov as f32),
                    pos: Vector3::new(cx+x+$vx as f32, cy+y+$vy as f32, cz+z+$vz as f32),
                    norm: Vector3::new($nx as f32, $ny as f32, $nz as f32),
                    face: $face
                }));)*
            }}
        }

        match side {
            Side::Top    => face! { side Top,    ind [0,1,2,3,2,1], vert [0,1,0; 1,1,0; 0,1,1; 1,1,1], off [0,1; 1,1; 0,0; 1,0], norm 0, 1,0;, face 1 },
            Side::Bottom => face! { side Bottom, ind [0,2,1,3,1,2], vert [0,0,0; 1,0,0; 0,0,1; 1,0,1], off [0,1; 1,1; 0,0; 1,0], norm 0,-1,0;, face 1 },
            Side::Front  => face! { side Front,  ind [0,1,2,3,2,1], vert [0,1,1; 1,1,1; 0,0,1; 1,0,1], off [0,0; 1,0; 0,1; 1,1], norm 0,0, 1;, face 0 },
            Side::Back   => face! { side Back,   ind [0,2,1,3,1,2], vert [0,1,0; 1,1,0; 0,0,0; 1,0,0], off [0,0; 1,0; 0,1; 1,1], norm 0,0,-1;, face 0 },
            Side::Left   => face! { side Left,   ind [0,1,2,3,2,1], vert [0,1,0; 0,1,1; 0,0,0; 0,0,1], off [0,0; 1,0; 0,1; 1,1], norm -1,0,0;, face 2 },
            Side::Right  => face! { side Right,  ind [0,2,1,3,1,2], vert [1,1,0; 1,1,1; 1,0,0; 1,0,1], off [0,0; 1,0; 0,1; 1,1], norm 1, 0,0;, face 2 },
        }
    }

    fn needs_face(&self, pos: Point3<i32>) -> bool {
        let in_bounds = self.chunk.in_chunk_bounds(pos);
        if in_bounds {
            self.chunk.get(pos).unwrap().has_transparency()
        } else {
            if pos.x >= CHUNK_SIZE as i32 {
                self.neighbors.right.get(Point3::new(0, pos.y, pos.z)).unwrap().has_transparency()
            } else if pos.x < 0 {
                self.neighbors.left.get(Point3::new(CHUNK_SIZE as i32 - 1, pos.y, pos.z)).unwrap().has_transparency()
            } else if pos.y >= CHUNK_SIZE as i32 {
                self.neighbors.top.get(Point3::new(pos.x, 0, pos.z)).unwrap().has_transparency()
            } else if pos.y < 0 {
                self.neighbors.bottom.get(Point3::new(pos.x, CHUNK_SIZE as i32 - 1, pos.z)).unwrap().has_transparency()
            } else if pos.z >= CHUNK_SIZE as i32 {
                self.neighbors.front.get(Point3::new(pos.x, pos.y, 0)).unwrap().has_transparency()
            } else if pos.z < 0 {
                self.neighbors.back.get(Point3::new(pos.x, pos.y, CHUNK_SIZE as i32 - 1)).unwrap().has_transparency()
            } else { false }
        }
    }
}

impl<'c, T: Voxel + 'c> Mesher<T::PerVertex, u32> for CullMesher<'c, T> {
    fn gen_vertex_data(mut self) -> (Vec<T::PerVertex>, Vec<u32>) {
        for i in 0..(CHUNK_SIZE as i32) {
            for j in 0..(CHUNK_SIZE as i32) {
                for k in 0..(CHUNK_SIZE as i32) {
                    let pos = Point3::new(i, j, k);
                    let block = self.chunk.get(pos).unwrap();
                    if block.has_transparency() { continue; }

                    if self.needs_face(pos + Vector3::unit_z()) { self.add_face(Side::Front, pos, &block) }
                    if self.needs_face(pos - Vector3::unit_z()) { self.add_face(Side::Back, pos, &block) }
                    if self.needs_face(pos + Vector3::unit_y()) { self.add_face(Side::Top, pos, &block) }
                    if self.needs_face(pos - Vector3::unit_y()) { self.add_face(Side::Bottom, pos, &block) }
                    if self.needs_face(pos + Vector3::unit_x()) { self.add_face(Side::Right, pos, &block) }
                    if self.needs_face(pos - Vector3::unit_x()) { self.add_face(Side::Left, pos, &block) }
                }
            }
        }
        (self.vertices, self.indices)
    }
}

// pub struct GreedyMesher<'c, T: Voxel + 'c> {
//     chunk: &'c Chunk<T>,
//     neighbors: Neighborhood<&'c Chunk<T>>,
//     visited_mask: Box<[bool]>,
//     vertices: Vec<T::PerVertex>,
//     indices: Vec<u32>,
// }

// impl<'c, T: Voxel + 'c> GreedyMesher<'c, T> {
//     fn new(chunk: &'c Chunk<T>, neighbors: Neighborhood<&'c Chunk<T>>) -> Self {
//         GreedyMesher {
//             chunk,
//             neighbors,
//             visited_mask: vec![false; chunk.size()].into_boxed_slice(),
//             vertices: Vec::new(),
//             indices: Vec::new(),
//         }
//     }

//     fn is_occluded(&self, x: isize, y: isize, z: isize) -> bool {
//         // let up = 
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