macro_rules! offset_arr {
    ($offset:expr, [$($item:expr),*]) => {[$($offset + $item),*]}
}

pub trait Mesher<V, I> {
    fn gen_vertex_data(self) -> (Vec<V>, Vec<I>);
    fn gen_mesh(self) -> GlResult<Mesh<V, I>>
    where
        V: InternalLayout,
        I: IndexingType,
        Self: Sized,
    {
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

use cgmath::{Point2, Point3, Vector2, Vector3};
use collision::Aabb2;
use engine::chunk::{in_chunk_bounds, Chunk, CHUNK_SIZE, CHUNK_VOLUME};
use engine::mesh::{IndexingType, Mesh};
use engine::{ChunkPos, Precomputed, Side, Voxel};
use gl_api::buffer::UsageType;
use gl_api::error::GlResult;
use gl_api::layout::InternalLayout;
use smallbitvec::SmallBitVec;
use std::cmp::Ordering;

impl<'c, T: Voxel + 'c> CullMesher<'c, T> {
    pub fn new(
        pos: ChunkPos,
        chunk: &'c Chunk<T>,
        top: &'c Chunk<T>,
        bottom: &'c Chunk<T>,
        left: &'c Chunk<T>,
        right: &'c Chunk<T>,
        front: &'c Chunk<T>,
        back: &'c Chunk<T>,
    ) -> Self {
        CullMesher {
            pos,
            chunk,
            neighbors: Neighborhood {
                top,
                bottom,
                left,
                right,
                front,
                back,
            },
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
        let in_bounds = in_chunk_bounds(pos);
        if in_bounds {
            self.chunk.get(pos).unwrap().has_transparency()
        } else {
            if pos.x >= CHUNK_SIZE as i32 {
                self.neighbors
                    .right
                    .get(Point3::new(0, pos.y, pos.z))
                    .unwrap()
                    .has_transparency()
            } else if pos.x < 0 {
                self.neighbors
                    .left
                    .get(Point3::new(CHUNK_SIZE as i32 - 1, pos.y, pos.z))
                    .unwrap()
                    .has_transparency()
            } else if pos.y >= CHUNK_SIZE as i32 {
                self.neighbors
                    .top
                    .get(Point3::new(pos.x, 0, pos.z))
                    .unwrap()
                    .has_transparency()
            } else if pos.y < 0 {
                self.neighbors
                    .bottom
                    .get(Point3::new(pos.x, CHUNK_SIZE as i32 - 1, pos.z))
                    .unwrap()
                    .has_transparency()
            } else if pos.z >= CHUNK_SIZE as i32 {
                self.neighbors
                    .front
                    .get(Point3::new(pos.x, pos.y, 0))
                    .unwrap()
                    .has_transparency()
            } else if pos.z < 0 {
                self.neighbors
                    .back
                    .get(Point3::new(pos.x, pos.y, CHUNK_SIZE as i32 - 1))
                    .unwrap()
                    .has_transparency()
            } else {
                false
            }
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
                    if block.has_transparency() {
                        continue;
                    }

                    if self.needs_face(pos + Vector3::unit_z()) {
                        self.add_face(Side::Front, pos, &block)
                    }
                    if self.needs_face(pos - Vector3::unit_z()) {
                        self.add_face(Side::Back, pos, &block)
                    }
                    if self.needs_face(pos + Vector3::unit_y()) {
                        self.add_face(Side::Top, pos, &block)
                    }
                    if self.needs_face(pos - Vector3::unit_y()) {
                        self.add_face(Side::Bottom, pos, &block)
                    }
                    if self.needs_face(pos + Vector3::unit_x()) {
                        self.add_face(Side::Right, pos, &block)
                    }
                    if self.needs_face(pos - Vector3::unit_x()) {
                        self.add_face(Side::Left, pos, &block)
                    }
                }
            }
        }
        (self.vertices, self.indices)
    }
}

// struct Adjacent<T> {
//     pub center: T,
//     pub top: T,
//     pub bottom: T,
//     pub left: T,
//     pub right: T,
//     pub front: T,
//     pub back: T,
// }

// struct Layer<T> {
//     data: Box<[T]>,
//     mask: Box<[bool]>,
// }

// impl<T> Layer<T> {
//     fn mask_from_world(world: Adjacent<&Chunk<T>>) -> Box<[bool]> {

//     }

//     fn new_from_world(world: Adjacent<&Chunk<T>>) -> Self {

//     }
// }

type Quad = Aabb2<i32>;

pub struct GreedyMesher<'c, T: Voxel + 'c> {
    chunk: &'c Chunk<T>,
    pos: Point3<i32>,
    neighbors: Neighborhood<&'c Chunk<T>>,
    mask: Box<[bool]>,
    vertices: Vec<T::PerVertex>,
    indices: Vec<u32>,
}

const SIZE: i32 = CHUNK_SIZE as i32;

impl<'c, T: Voxel + 'c> GreedyMesher<'c, T> {
    pub fn new(
        pos: Point3<i32>,
        chunk: &'c Chunk<T>,
        top: &'c Chunk<T>,
        bottom: &'c Chunk<T>,
        left: &'c Chunk<T>,
        right: &'c Chunk<T>,
        front: &'c Chunk<T>,
        back: &'c Chunk<T>,
    ) -> Self {
        // let mut current_layer = Vec::with_capacity(CHUNK_SIZE*CHUNK_SIZE);
        // let mut next_layer = Vec::with_capacity(CHUNK_SIZE*CHUNK_SIZE);
        // for
        GreedyMesher {
            chunk,
            neighbors: Neighborhood {
                top,
                bottom,
                left,
                right,
                front,
                back,
            },
            pos,
            mask: vec![false; CHUNK_SIZE * CHUNK_SIZE].into_boxed_slice(),
            vertices: Vec::new(),
            indices: Vec::new(),
        }
    }

    fn get(&self, pos: Point3<i32>) -> &T {
        const SIZE: i32 = CHUNK_SIZE as i32;
        let wrapped = ::util::get_chunk_pos(pos).1;
        if in_chunk_bounds(pos) {
            &self.chunk[pos]
        } else if pos.x >= SIZE {
            &self.neighbors.right[wrapped]
        } else if pos.x < 0 {
            &self.neighbors.left[wrapped]
        } else if pos.y >= SIZE {
            &self.neighbors.top[wrapped]
        } else if pos.y < 0 {
            &self.neighbors.bottom[wrapped]
        } else if pos.z >= SIZE {
            &self.neighbors.front[wrapped]
        } else if pos.z < 0 {
            &self.neighbors.back[wrapped]
        } else {
            unreachable!()
        }
    }

    fn is_occluded(&self, pos: Point3<i32>) -> bool {
        !(self.get(pos + Vector3::unit_x()).has_transparency()
            || self.get(pos - Vector3::unit_x()).has_transparency()
            || self.get(pos + Vector3::unit_y()).has_transparency()
            || self.get(pos - Vector3::unit_y()).has_transparency()
            || self.get(pos + Vector3::unit_z()).has_transparency()
            || self.get(pos - Vector3::unit_z()).has_transparency())
    }

    fn set_mask(&mut self, x: i32, z: i32, value: bool) {
        self.mask[(SIZE * x + z) as usize] = value;
    }

    fn get_mask(&self, x: i32, z: i32) -> bool {
        if x >= SIZE || x < 0 || z >= SIZE || z < 0 { false }
        else { self.mask[(SIZE * x + z) as usize] }
    }

    // fn can_merge(&self, a: Point3<i32>, b: Point3<i32>) -> bool
    // where
    //     T: PartialEq,
    // {
    //     // NOTE: `a` must have the same y component as the mask is for, and the same y component as `b`
    //     // `a` must additionally be a face that is a visible component of the mask
    //     debug_assert_eq!(a.y, b.y);
    //     // println!("a.x = {}, a.z = {}", a.x, a.z);
    //     debug_assert!(self.get_mask(a.x, a.z));
    //     let a_voxel = &self.chunk[a];
    //     debug_assert!(!a_voxel.has_transparency());
    //     if let Some(b_voxel) = self.chunk.get(b) {
    //         // Can merge if the two faces are the same and are both inside the mask
    //         b_voxel == a_voxel && self.get_mask(b.x, b.z)
    //     } else {
    //         // Cannot merge if the second position lays outside of the chunk boundary
    //         false
    //     }
    // }

    fn expand_right(&self, pos: Point3<i32>) -> Quad
    where
        T: PartialEq + ::std::fmt::Debug,
    {
        let start = self.chunk.get(pos).unwrap();
        // println!("START {:?}", start);
        for xn in pos.x..SIZE {
            let cur_point = Point3::new(xn + 1, pos.y, pos.z);
            let cur = self.chunk.get(cur_point);
            // println!("{} -> CUR {:?}", xn, cur);
            if Some(start) != cur || !self.get_mask(xn + 1, pos.z) {
                return Aabb2::new(
                    Point2::new(pos.x, pos.z),
                    Point2::new(xn, pos.z)
                );
            }
        }
        unreachable!()
    }
    // fn expand_right(&self, pos: Point3<i32>) -> Quad
    // where
    //     T: PartialEq,
    // {
    //     // println!("RIGHT: {:?}", pos);
    //     // debug_assert!(in_chunk_bounds(pos));
    //     // debug_assert!(!self.chunk[pos].has_transparency());
    //     for offset in 1..=SIZE {
    //         let offset = Vector3::new(offset, 0, 0);
    //         let new_pos = pos + offset;
    //         if !self.can_merge(pos, new_pos) {
    //             // We've hit a place where the new face cannot be merged
    //             return Aabb2::new(
    //                 Point2::new(pos.x, pos.z),
    //                 Point2::new(new_pos.x - 1, new_pos.z),
    //             );
    //         }
    //     }
    //     unreachable!()
    // }

    // fn expand_down(&self, quad: Quad, layer: i32) -> Quad
    // where
    //     T: PartialEq + ::std::fmt::Debug,
    // {
    //     // let min_voxel = &self.chunk[Point3::new(quad.min.x, layer, quad.min.y)];
    //     // let max_voxel = &self.chunk[Point3::new(quad.max.x, layer, quad.max.y)];
    //     // println!("MIN/MAX: {:?}, {:?} {}", min_voxel, max_voxel, layer);
    //     // debug_assert!(in_chunk_bounds(Point3::new(quad.min.x, layer, quad.min.y)));
    //     // debug_assert!(in_chunk_bounds(Point3::new(quad.max.x, layer, quad.max.y)));
    //     // debug_assert!(!min_voxel.has_transparency());
    //     // debug_assert!(!max_voxel.has_transparency());
    //     let pos = Point3::new(quad.min.x, layer, quad.min.y);
    //     for offset_x in 0..=quad.max.x - quad.min.x {
    //         for offset_z in 1..=SIZE {
    //             let offset = Vector3::new(offset_x, 0, offset_z);
    //             let new_pos = pos + offset;
    //             if !self.can_merge(pos, new_pos) {
    //                 // We've hit a place where the new face cannot be merged
    //                 return Aabb2::new(
    //                     Point2::new(pos.x, pos.z),
    //                     Point2::new(quad.max.x, new_pos.z - 1),
    //                 );
    //             }
    //         }
    //     }
    //     unreachable!()
    // }
    fn expand_down(&self, quad: Quad, layer: i32) -> Quad
    where
        T: PartialEq + ::std::fmt::Debug,
    {
        let start = &self.chunk[Point3::new(quad.min.x, layer, quad.min.y)];
        for zn in quad.min.y..SIZE {
            for xn in quad.min.x..=quad.max.x {
                let cur_point = Point3::new(xn, layer, zn + 1);
                let cur = self.chunk.get(cur_point);
                if Some(start) != cur || !self.get_mask(xn, zn + 1) {
                    return Aabb2::new(
                        Point2::new(quad.min.x, quad.min.y),
                        Point2::new(quad.max.x, zn),
                    );
                }
            }
        }
        unreachable!()
    }

    fn fill_mask(&mut self, layer: i32)
    where
        T: ::std::fmt::Debug,
    {
        self.mask = vec![false; CHUNK_SIZE * CHUNK_SIZE].into_boxed_slice();
        for x in 0..SIZE {
            for z in 0..SIZE {
                let pos = Point3::new(x, layer, z);
                let current = &self.chunk[pos];
                // println!("MASK: {:?} -> {:?}", pos, current);
                let above = self.get(pos + Vector3::new(0, 1, 0));
                // let below = self.get(pos - Vector3::new(0, 1, 0));
                // We need to set the mask for any visible face. A face is visible if the voxel
                // above it is transparent, and the current voxel is not transparent.
                let val = !current.has_transparency() && above.has_transparency();
                self.set_mask(x, z, val);
            }
        }
    }

    fn print_solid(&self, layer: i32, text: &str) {
        println!("--- SOLID {} ---", text);
        for z in 0..SIZE {
            for x in 0..SIZE {
                let pos = Point3::new(x, layer, z);
                if self.chunk[pos].has_transparency() {
                    print!(". ");
                } else {
                    print!("x ");
                }
            }
            println!();
        }
    }

    fn print_mask(&self, text: &str) {
        println!("--- MASK {} ---", text);
        for z in 0..SIZE {
            for x in 0..SIZE {
                if self.get_mask(x, z) {
                    print!("x ");
                } else {
                    print!(". ");
                }
            }
            println!();
        }
    }

    fn print_quad(&self, mut quad: Quad, text: &str) {
        use collision::Contains;
        println!("--- QUAD {} ---", text);
        println!("{:?}", quad);
        quad.max.x += 1;
        quad.max.y += 1;
        for z in 0..SIZE {
            for x in 0..SIZE {
                if quad.contains(&Point2::new(x, z)) {
                    print!("# ");
                } else {
                    if self.get_mask(x, z) {
                        print!("x ");
                    } else {
                        print!(". ");
                    }
                }
            }
            println!();
        }
    }

    fn pick_pos(&self, layer: i32) -> Option<Point3<i32>> {
        for x in 0..SIZE {
            for z in 0..SIZE {
                if self.get_mask(x, z) {
                    return Some(Point3::new(x, layer, z));
                }
            }
        }
        None
    }

    fn add_quad(&mut self, mut quad: Quad, voxel: &T, layer: i32) {
        quad.max.x += 1;
        quad.max.y += 1;
        let index_len = self.vertices.len() as u32;
        let cx = CHUNK_SIZE as f32 * self.pos.x as f32;
        let cy = CHUNK_SIZE as f32 * self.pos.y as f32;
        let cz = CHUNK_SIZE as f32 * self.pos.z as f32;
        let fq: Aabb2<f32> = Aabb2::new(quad.min.cast().unwrap(), quad.max.cast().unwrap());

        self.indices
            .extend(&offset_arr!(index_len, [0, 1, 2, 3, 2, 1]));

        self.vertices.push(voxel.vertex_data(Precomputed {
            side: Side::Top,
            norm: Vector3::new(1.0, 1.0, 0.0),
            face: 1,
            face_offset: Vector2::new(0.0, 1.0),
            pos: Vector3::new(cx + fq.min.x, cy + layer as f32 + 1.0, cz + fq.min.y),
        }));
        // vert [0,1,0; 1,1,0; 0,1,1; 1,1,1], off [0,1; 1,1; 0,0; 1,0], norm 0, 1,0;, face 1
        self.vertices.push(voxel.vertex_data(Precomputed {
            side: Side::Top,
            norm: Vector3::new(1.0, 1.0, 0.0),
            face: 1,
            face_offset: Vector2::new(1.0, 1.0),
            pos: Vector3::new(cx + fq.max.x, cy + layer as f32 + 1.0, cz + fq.min.y),
        }));
        self.vertices.push(voxel.vertex_data(Precomputed {
            side: Side::Top,
            norm: Vector3::new(1.0, 1.0, 0.0),
            face: 1,
            face_offset: Vector2::new(0.0, 0.0),
            pos: Vector3::new(cx + fq.min.x, cy + layer as f32 + 1.0, cz + fq.max.y),
        }));
        self.vertices.push(voxel.vertex_data(Precomputed {
            side: Side::Top,
            norm: Vector3::new(1.0, 1.0, 0.0),
            face: 1,
            face_offset: Vector2::new(1.0, 0.0),
            pos: Vector3::new(cx + fq.max.x, cy + layer as f32 + 1.0, cz + fq.max.y),
        }));
    }

    fn mark_visited(&mut self, quad: Quad) {
        for x in quad.min.x..=quad.max.x {
            for z in quad.min.y..=quad.max.y {
                self.set_mask(x, z, false);
            }
        }
    }
}

impl<'c, T: Voxel + PartialEq + ::std::fmt::Debug + 'c> Mesher<T::PerVertex, u32>
    for GreedyMesher<'c, T>
{
    fn gen_vertex_data(mut self) -> (Vec<T::PerVertex>, Vec<u32>) {
        for layer in 0..SIZE {
            self.fill_mask(layer);
            // While unvisited faces remain, pick a position from the remaining
            while let Some(pos) = self.pick_pos(layer) {
                let voxel = &self.chunk[pos];
                // Construct a quad that reaches as far right as possible
                let quad = self.expand_right(pos);
                // Expand that quad as far down as possible
                let quad = self.expand_down(quad, layer);
                self.mark_visited(quad);
                self.add_quad(quad, voxel, layer);
            }
        }

        (self.vertices, self.indices)
    }
}
