// use engine::mesher::BlockVertex;
// use engine::block::Block;
// use cgmath::{Matrix4, Point3, SquareMatrix, Vector3};
// use collision::Aabb3;
// use engine::mesh::Mesh;
// use engine::mesher::{CullMesher, GreedyMesher};
// use engine::terrain::ChunkGenerator;
// use engine::world::World;
// use engine::world::WorldGenerator;
// use engine::{Side, Voxel};
// use engine::{ChunkPos, WorldPos};
// use gl_api::buffer::UsageType;
// use gl_api::error::GlResult;
// use gl_api::shader::program::LinkedProgram;
// use std::collections::{HashMap, HashSet};
// use std::sync::Arc;
// use std::sync::RwLock;
// use cgmath::Vector2;

// type ChunkMesher<'c> = CullMesher<'c>;

// pub struct ChunkManager {
//     world: World<Block>,
//     world_generator: WorldGenerator<Block>,
//     meshes: HashMap<ChunkPos, Mesh<BlockVertex, u32>>,
//     dirty: HashSet<ChunkPos>,
//     queue: HashSet<ChunkPos>,
//     center: Arc<RwLock<ChunkPos>>,
//     radii: Arc<RwLock<Vector3<i32>>>,
// }

// impl ChunkManager {
//     pub fn new<G: ChunkGenerator<Block> + Send + 'static>(generator: G) -> Self {
//         let center = Arc::new(RwLock::new(Point3::new(0, 0, 0)));
//         let radii = Arc::new(RwLock::new(Vector3::new(2, 2, 2)));
//         let mut manager = ChunkManager {
//             world: World::new(),
//             world_generator: WorldGenerator::new(generator, radii.clone(), center.clone()),
//             meshes: HashMap::new(),
//             dirty: HashSet::new(),
//             queue: HashSet::new(),
//             center,
//             radii,
//         };

//         manager.queue_in_range();
//         manager
//     }

//     pub fn world(&self) -> &World<Block> {
//         &self.world
//     }

//     /// Be careful with this! It is *your* responsibility to not change voxels
//     /// without remeshing, because this will **NOT** cause a remesh of any
//     /// chunks!
//     pub fn world_mut(&mut self) -> &mut World<Block> {
//         &mut self.world
//     }

//     /// Set many voxels at once
//     pub fn set_voxel_range(&mut self, aabb: Aabb3<i32>, voxel: Block) {
//         const SIZE: i32 = ::engine::chunk::CHUNK_SIZE as i32;
//         let start = aabb.min;
//         let end = aabb.max;

//         let (chunk_start, block_start) = ::util::get_chunk_pos(start);
//         let (chunk_end, block_end) = ::util::get_chunk_pos(end);

//         for x in start.x..end.x {
//             for y in start.y..end.y {
//                 for z in start.z..end.z {
//                     let pos = Point3::new(x, y, z);
//                     if let Some(world_voxel) = self.world.get_voxel_mut(pos) {
//                         if *world_voxel != voxel {
//                             *world_voxel = voxel.clone();
//                         }
//                     }
//                 }
//             }
//         }

//         // Mark all the chunks directly affected by the block updates as dirty
//         for x in chunk_start.x..chunk_end.x + 1 {
//             for y in chunk_start.y..chunk_end.y + 1 {
//                 for z in chunk_start.z..chunk_end.z + 1 {
//                     self.dirty.insert(Point3::new(x, y, z));
//                 }
//             }
//         }

//         // Mark chunks on chunk borders as dirty if they were affected.
//         if block_start.x == 0 {
//             for y in chunk_start.y..chunk_end.y + 1 {
//                 for z in chunk_start.z..chunk_end.z + 1 {
//                     self.dirty.insert(Point3::new(chunk_start.x - 1, y, z));
//                 }
//             }
//         }
//         if block_start.y == 0 {
//             for x in chunk_start.x..chunk_end.x + 1 {
//                 for z in chunk_start.z..chunk_end.z + 1 {
//                     self.dirty.insert(Point3::new(x, chunk_start.y - 1, z));
//                 }
//             }
//         }
//         if block_start.z == 0 {
//             for x in chunk_start.x..chunk_end.x + 1 {
//                 for y in chunk_start.y..chunk_end.y + 1 {
//                     self.dirty.insert(Point3::new(x, y, chunk_start.z - 1));
//                 }
//             }
//         }
//         if block_end.x == SIZE - 1 {
//             for y in chunk_start.y..chunk_end.y + 1 {
//                 for z in chunk_start.z..chunk_end.z + 1 {
//                     self.dirty.insert(Point3::new(chunk_end.x + 1, y, z));
//                 }
//             }
//         }
//         if block_end.y == SIZE - 1 {
//             for x in chunk_start.x..chunk_end.x + 1 {
//                 for z in chunk_start.z..chunk_end.z + 1 {
//                     self.dirty.insert(Point3::new(x, chunk_end.y + 1, z));
//                 }
//             }
//         }
//         if block_end.z == SIZE - 1 {
//             for x in chunk_start.x..chunk_end.x + 1 {
//                 for y in chunk_start.y..chunk_end.y + 1 {
//                     self.dirty.insert(Point3::new(x, y, chunk_end.z + 1));
//                 }
//             }
//         }

//         println!("Dirty queue: {:?}", self.dirty);
//     }

//     /// Set a voxel, causing remeshes as needed.
//     pub fn set_voxel(&mut self, pos: WorldPos, voxel: Block) {
//         const SIZE: i32 = ::engine::chunk::CHUNK_SIZE as i32;
//         let (cpos, bpos) = ::util::get_chunk_pos(pos);
//         if let Some(world_voxel) = self.world.get_voxel_mut(pos) {
//             if *world_voxel != voxel {
//                 *world_voxel = voxel;
//                 // Mark as dirty for remeshing
//                 self.dirty.insert(cpos);
//                 // Also mark neighboring chunks as dirty if we destroy a block on the
//                 // border of a chunk
//                 if bpos.x == 0 {
//                     self.dirty.insert(cpos - Vector3::unit_x());
//                 } // Left
//                 if bpos.x == SIZE - 1 {
//                     self.dirty.insert(cpos + Vector3::unit_x());
//                 } // Right
//                 if bpos.y == 0 {
//                     self.dirty.insert(cpos - Vector3::unit_y());
//                 } // Bottom
//                 if bpos.y == SIZE - 1 {
//                     self.dirty.insert(cpos + Vector3::unit_y());
//                 } // Top
//                 if bpos.z == 0 {
//                     self.dirty.insert(cpos - Vector3::unit_z());
//                 } // Back
//                 if bpos.z == SIZE - 1 {
//                     self.dirty.insert(cpos + Vector3::unit_z());
//                 } // Front
//             }
//         }
//     }

//     /// Get the mesher for the chunk passed in, on none if not all of the neighbor chunks
//     /// are loaded yet
//     fn get_mesher<'c>(&'c self, pos: ChunkPos) -> Option<ChunkMesher<'c>> {
//         let chunk = self.world.chunks.get(&pos)?;
//         let top = self.world.chunks.get(&(pos + Vector3::unit_y()))?;
//         let bottom = self.world.chunks.get(&(pos - Vector3::unit_y()))?;
//         let right = self.world.chunks.get(&(pos + Vector3::unit_x()))?;
//         let left = self.world.chunks.get(&(pos - Vector3::unit_x()))?;
//         let front = self.world.chunks.get(&(pos + Vector3::unit_z()))?;
//         let back = self.world.chunks.get(&(pos - Vector3::unit_z()))?;

//         Some(ChunkMesher::new(
//             pos, chunk, top, bottom, left, right, front, back,
//         ))
//     }

//     fn center(&self) -> ChunkPos {
//         *self.center.read().unwrap()
//     }

//     fn radii(&self) -> Vector3<i32> {
//         *self.radii.read().unwrap()
//     }

//     /// Add chunks to generator queue that are not already generated
//     fn queue_in_range(&mut self) {
//         let radii = self.radii();
//         let center = self.center();
//         // Generate chunks one outside the radius so the mesher can properly
//         // mesh all the chunks in the radius
//         println!(
//             "self.center = {:?}, radii = {:?}",
//             self.center, radii
//         );
//         for x in -radii.x - 1..radii.x + 1 {
//             for y in -radii.y - 1..radii.y + 1 {
//                 for z in -radii.z - 1..radii.z + 1 {
//                     let pos = center + Vector3::new(x, y, z);
//                     // Don't queue chunks that are already loaded
//                     if !self.world.chunk_exists(pos) {
//                         self.world_generator.queue(pos);
//                     }
//                 }
//             }
//         }

//         for x in -radii.x..radii.x {
//             for y in -radii.y..radii.y {
//                 for z in -radii.z..radii.z {
//                     let pos = center + Vector3::new(x, y, z);
//                     if !self.meshes.contains_key(&pos) {
//                         self.queue.insert(Point3::new(x, y, z));
//                     }
//                 }
//             }
//         }
//     }

//     fn unload(&mut self) {
//         let (center, radii) = (self.center(), self.radii());
//         self.meshes
//             .retain(|&pos, _| ::util::in_range(pos, center, radii));
//         // make sure to unload that one-chunk buffer
//         self.world.unload(center, radii + Vector3::new(1, 1, 1));
//     }

//     pub fn update_player_position(&mut self, pos: Point3<f32>) {
//         let x = (pos.x / ::engine::chunk::CHUNK_SIZE as f32).ceil() as i32;
//         let y = (pos.y / ::engine::chunk::CHUNK_SIZE as f32).ceil() as i32;
//         let z = (pos.z / ::engine::chunk::CHUNK_SIZE as f32).ceil() as i32;
//         let pos = Point3::new(x, y, z);
//         // Don't run the expensive stuff if we haven't moved
//         if pos == self.center() {
//             return;
//         }
//         *self.center.write().unwrap() = pos;
//         self.queue_in_range();
//         self.unload();
//     }

//     pub fn tick(&mut self) {
//         use rayon::iter::{IntoParallelIterator, ParallelIterator};

//         self.world_generator.update_world(&mut self.world);
//         if self.queue.len() > 0 {
//             // This sets up all the meshers, filtering out any mesher that doesn't
//             // have all the neighbor chunks generated yet. All the mess with passing
//             // around `pos` in a tuple is because we need to know which meshes belong
//             // to which chunks.
//             let meshers = self.queue
//                 .iter()
//                 .map(|pos| (pos, self.get_mesher(*pos)))
//                 .filter(|&(_, ref opt)| if let &None = opt { false } else { true })
//                 .map(|(pos, opt)| (*pos, opt.unwrap()))
//                 .take(4)
//                 .collect::<Vec<_>>();

//             // The heavy-lifting parallel iterator that iterates the meshers in parallel
//             // and generates the mesh data
//             let meshes = meshers
//                 .into_par_iter()
//                 .map(|(pos, mut mesher)| (pos, { mesher.mesh(); mesher }))
//                 .collect::<Vec<_>>();
            
//             // TODO: unwrap
//             let meshes = meshes.into_iter().map(|(pos, mesher)| (pos, mesher.create_mesh().unwrap())).collect::<Vec<_>>();

//             // Iterate the generated meshes, construct the actual mesh (the one with the
//             // actual vertex and index buffer), and insert them into the mesh map.
//             // NOTE: We need to construct the mesh on the main OpenGL thread.
//             for (pos, mesh) in meshes.into_iter() {
//                 self.meshes.insert(pos, mesh);
//                 self.queue.remove(&pos);
//                 println!("Meshed chunk ({:?}), meshes: {}, queue: {}", pos, self.meshes.len(), self.queue.len());
//             }
//         }

//         if self.dirty.len() > 0 {
//             // Update all dirty at once to avoid problems where unfinished
//             // meshes flash after a block is destroyed. NOTE: any dirty
//             // positions that were marked in the one-chunk gap between meshed
//             // chunks and nothing will get removed here, but this is not a
//             // problem since we haven't meshed them anyways.
//             let meshes = self.dirty
//                 .drain()
//                 .collect::<Vec<_>>()
//                 .into_iter()
//                 .map(|pos| (pos, self.get_mesher(pos)))
//                 .filter_map(|(pos, mesher)| mesher.map(|mut mesher| (pos, { mesher.mesh(); mesher.create_mesh().unwrap() })))
//                 .collect::<Vec<_>>();

//             for (pos, mesh) in meshes {
//                 self.meshes.insert(pos, mesh);
//             }
//         }
//     }

//     pub fn draw(&mut self, pipeline: &mut LinkedProgram) -> GlResult<()> {
//         pipeline.set_uniform("u_Transform", &Matrix4::<f32>::identity());
//         for mesh in self.meshes.values() {
//             mesh.draw_with(&pipeline)?;
//         }
//         Ok(())
//     }
// }
