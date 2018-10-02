use cgmath::{Point3, Vector3};

pub const SIZE: usize = 32;
pub const AREA: usize = SIZE * SIZE;
pub const VOLUME: usize = SIZE * SIZE * SIZE;

pub fn in_chunk_bounds(pos: Point3<i32>) -> bool {
    const SIZEI: i32 = SIZE as i32;
    pos.x < SIZEI && pos.y < SIZEI && pos.z < SIZEI && pos.x >= 0 && pos.y >= 0 && pos.z >= 0
}

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct Chunk<T> {
    crate data: ::nd::Array3<T>,
}

impl<T> Chunk<T> {
    pub fn new(voxels: ::nd::Array3<T>) -> Self {
        Chunk {
            data: ::nd::Array3::from(voxels)
        }
    }
}

impl<T> Chunk<T> {
    pub fn get(&self, pos: Point3<i32>) -> Option<&T> {
        if in_chunk_bounds(pos) {
            let pos: Point3<usize> = pos.cast().unwrap();
            Some(&self[pos])
        } else {
            None
        }
    }

    pub fn get_mut(&mut self, pos: Point3<i32>) -> Option<&mut T> {
        if in_chunk_bounds(pos) {
            let pos: Point3<usize> = pos.cast().unwrap();
            Some(&mut self[pos])
        } else {
            None
        }
    }

    pub fn size(&self) -> usize {
        self.data.len()
    }
}

use std::ops::{Index, IndexMut};

macro_rules! gen_index {
    ($name:ident : $type:ty => $x:expr, $y:expr, $z:expr) => {
        impl<T> Index<$type> for Chunk<T> {
            type Output = T;
            fn index(&self, $name: $type) -> &T {
                debug_assert!(in_chunk_bounds(Point3::new(
                    $x as i32, $y as i32, $z as i32
                )));
                &self.data[($x, $y, $z)]
            }
        }

        impl<T> IndexMut<$type> for Chunk<T> {
            fn index_mut(&mut self, $name: $type) -> &mut T {
                debug_assert!(in_chunk_bounds(Point3::new(
                    $x as i32, $y as i32, $z as i32
                )));
                &mut self.data[($x, $y, $z)]
            }
        }
    };
}

gen_index!(point: Point3<usize> => point.x, point.y, point.z);
gen_index!(point: Point3<i32> => point.x as usize, point.y as usize, point.z as usize);
gen_index!(point: Vector3<usize> => point.x, point.y, point.z);
gen_index!(point: Vector3<i32> => point.x as usize, point.y as usize, point.z as usize);
gen_index!(point: (usize, usize, usize) => point.0, point.1, point.2);
