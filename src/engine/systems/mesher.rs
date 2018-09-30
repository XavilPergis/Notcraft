// use engine::mesh::Mesh;
// use engine::world::block::BlockRegistry;
// use engine::world::block::BlockId;
// use engine::ChunkPos;
// use engine::world::VoxelWorld;
// use specs::prelude::*;
// use cgmath::{Point3, Vector3};
// use engine::world::chunk::SIZE;
// use engine::world::Chunk;

// use nd;

// #[repr(u8)]
// #[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
// enum Axis {
//     X = 0, Y = 1, Z = 2
// }

// struct ChunkView<'c, T> {
//     chunk: &'c Chunk<T>,
//     axis: Axis,
// }

// impl<'c, T: Clone> ChunkView<'c, T> {
//     fn to_local_space(&self, chunk_space: Point3<i32>) -> Point3<i32> {
//         let (u, l, v) = chunk_space.into();
//         match self.axis {
//             Axis::X => Point3::new(v, l, u),
//             Axis::Y => Point3::new(l, u, v),
//             Axis::Z => Point3::new(u, v, l),
//         }

//         // Side::Up => Vector3::new(0, 1, 0),
//         // Side::Right => Vector3::new(1, 0, 0),
//         // Side::Front => Vector3::new(0, 0, 1),
//         // Side::Down => Vector3::new(0, -1, 0),
//         // Side::Left => Vector3::new(-1, 0, 0),
//         // Side::Back => Vector3::new(0, 0, -1),
//     }
// }

// // ++ +- -- -+
// #[derive(Copy, Clone, Debug, Eq, PartialEq, Hash, Default)]
// struct VoxelFace {
//     ao: u8,
//     visible: bool,
// }

// impl VoxelFace {
//     const AO_POS_POS: u8 = 6;
//     const AO_POS_NEG: u8 = 4;
//     const AO_NEG_NEG: u8 = 2;
//     const AO_NEG_POS: u8 = 0;

//     fn corner_ao(&self, bits: u8) -> u8 {
//         (self.ao & (3 << bits)) >> bits
//     }

// }

// struct MeshConstructor {
//     pos: Point3<i32>,
//     index: u32,
//     // mesh: Mesh<BlockVertex, u32>,
// }

// impl MeshConstructor {
//     // fn push()
// }

// fn window<I>(a: impl Iterator<Item=I> + Clone) -> impl Iterator<Item=(I, I)> {
//     a.clone().zip(a.skip(1))
// }

// struct PaddedSlice<'c> {
//     center: nd::ArrayView2<'c, BlockId>,

//     top: nd::ArrayView1<'c, BlockId>,
//     bottom: nd::ArrayView1<'c, BlockId>,
//     right: nd::ArrayView1<'c, BlockId>,
//     left: nd::ArrayView1<'c, BlockId>,

//     top_right: BlockId,
//     bottom_right: BlockId,
//     bottom_left: BlockId,
//     top_left: BlockId,
// }

// impl<'c> PaddedSlice<'c> {
//     fn bottom_
// }

// struct GreedyMesher<'w, 'r> {
//     world: &'w VoxelWorld,
//     chunk: &'w Chunk<BlockId>,
//     registry: &'r BlockRegistry,
//     slice: nd::Array2<VoxelFace>,
//     mesh_constructor: MeshConstructor,
// }


// impl<'c, 'r> GreedyMesher<'c, 'r> {
//     fn face(&self, axis: Axis, u: usize, v: usize) -> (VoxelFace, VoxelFace) {
//         unimplemented!()
//     }

//     fn 

//     fn mesh(&mut self) {
//         // for each dimension...
//         for &axis in &[Axis::X, Axis::Y, Axis::Z] { // X=0, Y=1, Z=2
//             // bottom: slice for -axis + 0
//             // top: slice for +axis + chunk_size - 1
//             self.chunk.data.axis_iter(nd::Axis(axis as usize))
//                 .map(|slice|);
//             // for (layer, (slice, next_slice)) in window(self.chunk.data.axis_iter(nd::Axis(axis as usize))).enumerate() {
//             //     for ((idx, item), next_item) in slice.indexed_iter().zip(next_slice.iter()) {
//             //         // let (front, back) = self.face(axis, idx.0, idx.1, layer);
//             //         // self.slice_forward[idx] = front;
//             //         // self.slice_backward[idx] = back;
//             //     }
//             // }
//         }
//     }
// }
