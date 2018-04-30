use collision::Aabb3;
use std::collections::HashSet;
use gl_api::error::GlResult;
use std::collections::HashMap;
use cgmath::Vector3;
use cgmath::{SquareMatrix, Matrix4};
use engine::chunk::*;
use engine::mesh::Mesh;
use gl_api::shader::program::LinkedProgram;
use gl_api::buffer::UsageType;
use std::sync::mpsc;

struct AroundVector {
    center: Vector3<i32>,
    radius: i32,
    amount: i32,
}

impl AroundVector {
    fn new(center: Vector3<i32>, radius: i32) -> Self {
        AroundVector {
            center,
            radius,
            amount: 0,
        }
    }
}

impl Iterator for AroundVector {
    type Item = Vector3<i32>;
    fn next(&mut self) -> Option<Self::Item> {
        let length = 2*self.radius+1;
        if self.amount >= length*length*length { return None; }
        let x = self.amount % length;
        let y = self.amount/length % length;
        let z = self.amount/length/length % length;
        self.amount += 1;
        Some(Vector3::new(x, y, z) + self.center - Vector3::new(self.radius,self.radius,self.radius))
    }
}

/// Get a chunk position from a world position
fn get_chunk_pos(pos: Vector3<i32>) -> (Vector3<i32>, Vector3<i32>) {
    const SIZE: i32 = super::chunk::CHUNK_SIZE as i32;
    let cx = ::util::floor_div(pos.x, SIZE);
    let cy = ::util::floor_div(pos.y, SIZE);
    let cz = ::util::floor_div(pos.z, SIZE);

    let cpos = Vector3::new(cx, cy, cz);
    let bpos = pos - (SIZE*cpos);

    (cpos, bpos)

    // // The almighty modulo-matic operation that Does What I Want:tm:
    // // (aka does not go negative when x goes negative)
    // // (x % N + N) % N

    // // let x = (pos.x % SIZE + SIZE) % SIZE;
    // let x = pos.x % SIZE + if pos.x < 0 { SIZE } else { 0 };
    // let y = pos.y % SIZE + if pos.y < 0 { SIZE } else { 0 };
    // let z = pos.z % SIZE + if pos.z < 0 { SIZE } else { 0 };

    // (Vector3::new(cx, cy, cz), Vector3::new(x, y, z))
}

pub struct ChunkManager<T> {
    chunks: HashMap<Vector3<i32>, Chunk<T>>,
    meshes: HashMap<Vector3<i32>, Mesh<ChunkVertex, u32>>,
    dirty: HashSet<Vector3<i32>>,
    gen_queue: HashSet<Vector3<i32>>,
    mesh_queue: HashSet<Vector3<i32>>,
    center: Vector3<i32>,
    radius: i32,
    chunk_tx: mpsc::Sender<Vector3<i32>>,
    chunk_rx: mpsc::Receiver<(Vector3<i32>, Chunk<T>)>,
    // mesh_tx: mpsc::Sender<Vector3<i32>>,
    // mesh_rx: mpsc::Receiver<(Vec<ChunkVertex>, Vec<u32>)>,
}

use super::terrain::ChunkGenerator;

impl<T: Voxel + Clone + Send + Sync + 'static> ChunkManager<T> {
    pub fn new<G: ChunkGenerator<T> + Send + 'static>(generator: G) -> Self {
        use std::thread;
        let (chunk_req_tx, chunk_req_rx) = mpsc::channel();
        let (chunk_tx, chunk_rx) = mpsc::channel();
        thread::spawn(move || {
            while let Ok(request) = chunk_req_rx.recv() {
                chunk_tx.send((request, generator.generate(request)));
            }    
        });

        let mut manager = ChunkManager {
            chunks: HashMap::new(),
            meshes: HashMap::new(),
            dirty: HashSet::new(),
            gen_queue: HashSet::new(),
            mesh_queue: HashSet::new(),
            center: Vector3::new(0, 0, 0),
            radius: 2,
            chunk_tx: chunk_req_tx,
            chunk_rx,
        };
        manager.queue_in_range();
        manager
    }

    pub fn set_voxel(&mut self, pos: Vector3<i32>, voxel: T) where T: PartialEq {
        const SIZE: i32 = super::chunk::CHUNK_SIZE as i32;
        let (cpos, bpos) = get_chunk_pos(pos);
        // get the chunk the voxel is in, if it is loaded
        if let Some(chunk) = self.chunks.get_mut(&cpos) {
            let pos = (bpos.x as usize, bpos.y as usize, bpos.z as usize);
            // Don't trigger a remesh if there is no change in the type of block
            if chunk[pos] != voxel {
                chunk[pos] = voxel;
                // Mark as dirty for remeshing
                self.dirty.insert(cpos);
                // Also mark neighboring chunks as dirty if we destroy a block on the
                // border of a chunk
                if bpos.x == 0    { println!("Left"); self.dirty.insert(cpos - Vector3::unit_x()); } // Left
                if bpos.x == SIZE { println!("Right"); self.dirty.insert(cpos + Vector3::unit_x()); } // Right
                if bpos.y == 0    { println!("Bottom"); self.dirty.insert(cpos - Vector3::unit_y()); } // Bottom
                if bpos.y == SIZE { println!("Top"); self.dirty.insert(cpos + Vector3::unit_y()); } // Top
                if bpos.z == 0    { println!("Back"); self.dirty.insert(cpos - Vector3::unit_z()); } // Back
                if bpos.z == SIZE { println!("Front"); self.dirty.insert(cpos + Vector3::unit_z()); } // Front
            }
        }
    }

    /// Get the mesher for the chunk passed in, on none if not all of the neighbor chunks
    /// are loaded yet
    fn get_mesher<'c>(&'c self, pos: Vector3<i32>) -> Option<CullMesher<'c, T>> {
        let chunk = self.chunks.get(&pos)?;
        let top = self.chunks.get(&(pos + Vector3::unit_y()))?;
        let bottom = self.chunks.get(&(pos - Vector3::unit_y()))?;
        let right = self.chunks.get(&(pos + Vector3::unit_x()))?;
        let left = self.chunks.get(&(pos - Vector3::unit_x()))?;
        let front = self.chunks.get(&(pos + Vector3::unit_z()))?;
        let back = self.chunks.get(&(pos - Vector3::unit_z()))?;

        Some(CullMesher::new(chunk, top, bottom, left, right, front, back))
    }

    /// Add chunks to generator queue that are not already generated
    fn queue_in_range(&mut self) {
        // Generate chunks one outside the radius so the mesher can properly
        // mesh all the chunks in the radius
        for x in self.center.x - self.radius - 1..self.center.x + self.radius + 1 {
            for y in self.center.y - self.radius - 1..self.center.y + self.radius + 1 {
                for z in self.center.z - self.radius - 1..self.center.z + self.radius + 1 {
                    let pos = Vector3::new(x, y, z);
                    if !self.chunks.contains_key(&pos) {
                        self.gen_queue.insert(pos);
                        self.chunk_tx.send(pos).unwrap();
                    }
                }
            }
        }

        for x in self.center.x - self.radius..self.center.x + self.radius {
            for y in self.center.y - self.radius..self.center.y + self.radius {
                for z in self.center.z - self.radius..self.center.z + self.radius {
                    let pos = Vector3::new(x, y, z);
                    if !self.meshes.contains_key(&pos) {
                        self.mesh_queue.insert(Vector3::new(x, y, z));
                    }
                }
            }
        }
    }

    fn unload(&mut self) {
        let mut to_unload = Vec::new();
        for loaded_coords in self.chunks.keys() {
            let x = loaded_coords.x > self.center.x + self.radius + 1 ||
                    loaded_coords.x < self.center.x - self.radius - 1;
            let y = loaded_coords.y > self.center.y + self.radius + 1 ||
                    loaded_coords.y < self.center.y - self.radius - 1;
            let z = loaded_coords.z > self.center.z + self.radius + 1 ||
                    loaded_coords.z < self.center.z - self.radius - 1;

            if x || y || z {
                to_unload.push(*loaded_coords);
            }
        }

        for pos in to_unload {
            self.chunks.remove(&pos);
            // Removing an empty space doesn't do anything bad
            self.meshes.remove(&pos);
        }
    }

    pub fn update_player_position(&mut self, pos: Vector3<f32>) {
        let x = (pos.x / super::chunk::CHUNK_SIZE as f32).ceil() as i32;
        let y = (pos.y / super::chunk::CHUNK_SIZE as f32).ceil() as i32;
        let z = (pos.z / super::chunk::CHUNK_SIZE as f32).ceil() as i32;
        let pos = Vector3::new(x, y, z);
        // Don't run the expensive stuff if we haven't moved
        if pos == self.center { return; }
        self.center = pos;
        self.queue_in_range();
        self.unload();
    }

    pub fn tick(&mut self) {
        use rayon::iter::{IntoParallelIterator, ParallelIterator};
        if self.gen_queue.len() > 0 {
            for (pos, chunk) in self.chunk_rx.try_iter() {
                self.chunks.insert(pos, chunk);
                self.gen_queue.remove(&pos);
                println!("Generated chunk ({:?}), gen_queue: {}", pos, self.gen_queue.len());
            }
        }

        if self.mesh_queue.len() > 0 {
            // This sets up all the meshers, filtering out any mesher that doesn't
            // have all the neighbor chunks generated yet. All the mess with passing
            // around `pos` in a tuple is because we need to know which meshes belong
            // to which chunks.
            let meshers = self.mesh_queue.iter()
                .map(|pos| (pos, self.get_mesher(*pos)))
                .filter(|&(_, ref opt)| if let &None = opt { false } else { true })
                .map(|(pos, opt)| (*pos, opt.unwrap()))
                .take(4)
                .collect::<Vec<_>>();
            
            // The heavy-lifting parallel iterator that iterates the meshers in parallel
            // and generates the mesh data
            let meshes = meshers.into_par_iter()
                .map(|(pos, mesher)| (pos, mesher.gen_vertex_data()))
                .collect::<Vec<_>>();
            
            // Iterate the generated meshes, construct the actual mesh (the one with the
            // actual vertex and index buffer), and insert them into the mesh map.
            // NOTE: We need to construct the mesh on the main OpenGL thread.
            for (pos, mesh) in meshes.into_iter() {
                let (vertices, indices) = mesh;
                let mut mesh = Mesh::new().unwrap(); // TODO: unwrap
                mesh.upload(vertices, indices, UsageType::Static).unwrap();
                self.meshes.insert(pos, mesh);
                self.mesh_queue.remove(&pos);
                println!("Meshed chunk ({:?}), mesh_queue: {}", pos, self.meshes.len());
            }
        }

        if self.dirty.len() > 0 {
            // Update all dirty at once to avoid problems where unfinished meshes flash
            // after a block is destroyed.
            let meshes = self.dirty.drain()
                .collect::<Vec<_>>()
                .into_iter()
                .map(|pos| (pos, self.get_mesher(pos)))
                .filter_map(|(pos, mesher)| mesher.map(|m| (pos, m.gen_mesh().unwrap())))
                .collect::<Vec<_>>();
            
            for (pos, mesh) in meshes {
                self.meshes.insert(pos, mesh);
            }
        }
    }

    fn get_voxel(&self, pos: Vector3<i32>) -> Option<&T> {
        let (cpos, bpos) = get_chunk_pos(pos);
        if let Some(chunk) = self.chunks.get(&cpos) {
            let pos = (bpos.x as usize, bpos.y as usize, bpos.z as usize);
            Some(&chunk[pos])
        } else {
            None
        }
    }

    pub fn colliders_around_point(&self, pos: Vector3<i32>, radius: i32) -> Vec<Aabb3<f32>> {
        assert!(radius >= 0);
        let mut buf = Vec::with_capacity((radius*radius*radius) as usize);
        for x in pos.x - radius..pos.x + radius {
            for y in pos.y - radius..pos.y + radius {
                for z in pos.z - radius..pos.z + radius {
                    let pos = Vector3::new(x, y, z);
                    let fpos = Vector3::new(x as f32, y as f32, z as f32);
                    if let Some(voxel) = self.get_voxel(pos) {
                        if !voxel.has_transparency() {
                            buf.push(Aabb3::new(
                                ::util::to_point(fpos),
                                ::util::to_point(fpos + Vector3::new(1.0, 1.0, 1.0)),
                            ));
                        }
                    }
                }
            }
        }
        buf
    }

    pub fn draw(&mut self, pipeline: &mut LinkedProgram) -> GlResult<()> {
        pipeline.set_uniform("u_Transform", &Matrix4::<f32>::identity());
        for mesh in self.meshes.values() {
            mesh.draw_with(&pipeline)?;
        }
        Ok(())
    }
}
