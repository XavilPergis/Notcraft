use smallvec::SmallVec;
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
use super::terrain::ChunkGenerator;

/// Get a chunk position from a world position
pub fn get_chunk_pos(pos: Vector3<i32>) -> (Vector3<i32>, Vector3<i32>) {
    const SIZE: i32 = super::chunk::CHUNK_SIZE as i32;
    let cx = ::util::floor_div(pos.x, SIZE);
    let cy = ::util::floor_div(pos.y, SIZE);
    let cz = ::util::floor_div(pos.z, SIZE);

    let cpos = Vector3::new(cx, cy, cz);
    let bpos = pos - (SIZE*cpos);

    (cpos, bpos)
}

pub type WorldPos = Vector3<i32>;
pub type ChunkPos = Vector3<i32>;

pub struct World<T> {
    chunks: HashMap<ChunkPos, Chunk<T>>,
    queue: HashSet<ChunkPos>,
    gen_tx: mpsc::Sender<ChunkPos>,
    gen_rx: mpsc::Receiver<(ChunkPos, Chunk<T>)>,
}

impl<T> World<T> {
    pub fn new<G: ChunkGenerator<T> + Send + 'static>(generator: G) -> Self where T: Send + 'static {
        use std::thread;
        let (req_tx, req_rx) = mpsc::channel();
        let (tx, rx) = mpsc::channel();
        thread::spawn(move || {
            while let Ok(request) = req_rx.recv() {
                // Err means the rx has hung up, so we can just shut down this thread
                // if that happens
                match tx.send((request, generator.generate(request))) {
                    Ok(_) => (),
                    Err(_) => break,
                }
            }    
        });

        World {
            chunks: HashMap::new(),
            queue: HashSet::new(),
            gen_tx: req_tx,
            gen_rx: rx,
        }
    }

    pub fn set_voxel(&mut self, pos: WorldPos, voxel: T) where T: PartialEq {
        let (cpos, bpos) = get_chunk_pos(pos);
        // get the chunk the voxel is in, if it is loaded
        if let Some(chunk) = self.chunks.get_mut(&cpos) {
            let pos = (bpos.x as usize, bpos.y as usize, bpos.z as usize);
            chunk[pos] = voxel;
        }
    }

    #[inline]
    pub fn queue(&mut self, pos: ChunkPos) {
        if !self.chunks.contains_key(&pos) {
            self.queue.insert(pos);
            self.gen_tx.send(pos).unwrap();
        }
    }

    pub fn unload<I: IntoIterator<Item=ChunkPos>>(&mut self, iter: I) {
        for location in iter {
            self.chunks.remove(&location);
        }
    }

    fn tick(&mut self) {
        for (pos, chunk) in self.gen_rx.try_iter() {
            println!("Generated chunk at ({:?}) => queue.len() = {}", pos, self.queue.len());
            self.queue.remove(&pos);
            self.chunks.insert(pos, chunk);
        }
    }

    pub fn get_voxel(&self, pos: WorldPos) -> Option<&T> {
        let (cpos, bpos) = get_chunk_pos(pos);
        let pos = (bpos.x as usize, bpos.y as usize, bpos.z as usize);
        self.chunks.get(&cpos).map(|chunk| &chunk[pos])
    }

    pub fn get_voxel_mut(&mut self, pos: WorldPos) -> Option<&mut T> {
        let (cpos, bpos) = get_chunk_pos(pos);
        let pos = (bpos.x as usize, bpos.y as usize, bpos.z as usize);
        self.chunks.get_mut(&cpos).map(|chunk| &mut chunk[pos])
    }

    pub fn around_voxel<U, F: Fn(WorldPos, &T) -> Option<U>>(&self, pos: WorldPos, radius: u32, func: F) -> Vec<U> {
        let radius = radius as i32;
        // TODO: We allocate here, but likely don't need to. It would be better if this
        // function returned an iterator...
        let mut buf = Vec::with_capacity((radius*radius*radius) as usize);
        for x in pos.x - radius..pos.x + radius {
            for y in pos.y - radius..pos.y + radius {
                for z in pos.z - radius..pos.z + radius {
                    let pos = Vector3::new(x, y, z);
                    if let Some(voxel) = self.get_voxel(pos) {
                        if let Some(item) = func(pos, voxel) {
                            buf.push(item);
                        }
                    }
                }
            }
        }
        buf
    }
}

pub struct ChunkManager<T> {
    world: World<T>,
    meshes: HashMap<ChunkPos, Mesh<ChunkVertex, u32>>,
    dirty: HashSet<ChunkPos>,
    queue: HashSet<ChunkPos>,
    center: ChunkPos,
    radius: i32,
}

impl<T: Voxel + Clone + Send + Sync + 'static> ChunkManager<T> {
    pub fn new<G: ChunkGenerator<T> + Send + 'static>(generator: G) -> Self {
        let mut manager = ChunkManager {
            world: World::new(generator),
            meshes: HashMap::new(),
            dirty: HashSet::new(),
            queue: HashSet::new(),
            center: Vector3::new(0, 0, 0),
            radius: 3,
        };

        manager.queue_in_range();
        manager
    }

    pub fn world(&self) -> &World<T> { &self.world }

    /// Be careful with this! It is *your* responsibility to not clear voxels without
    /// remeshing, because this will **NOT** cause a remesh of any chunks!
    pub fn world_mut(&mut self) -> &mut World<T> { &mut self.world }

    /// Set a voxel, causing remeshes as needed.
    pub fn set_voxel(&mut self, pos: WorldPos, voxel: T) where T: PartialEq {
        const SIZE: i32 = super::chunk::CHUNK_SIZE as i32;
        let (cpos, bpos) = get_chunk_pos(pos);
        if let Some(world_voxel) = self.world.get_voxel_mut(pos) {
            if *world_voxel != voxel {
                *world_voxel = voxel;
                // Mark as dirty for remeshing
                self.dirty.insert(cpos);
                // Also mark neighboring chunks as dirty if we destroy a block on the
                // border of a chunk
                if bpos.x == 0      { self.dirty.insert(cpos - Vector3::unit_x()); } // Left
                if bpos.x == SIZE-1 { self.dirty.insert(cpos + Vector3::unit_x()); } // Right
                if bpos.y == 0      { self.dirty.insert(cpos - Vector3::unit_y()); } // Bottom
                if bpos.y == SIZE-1 { self.dirty.insert(cpos + Vector3::unit_y()); } // Top
                if bpos.z == 0      { self.dirty.insert(cpos - Vector3::unit_z()); } // Back
                if bpos.z == SIZE-1 { self.dirty.insert(cpos + Vector3::unit_z()); } // Front
            }
        }
    }

    /// Get the mesher for the chunk passed in, on none if not all of the neighbor chunks
    /// are loaded yet
    fn get_mesher<'c>(&'c self, pos: ChunkPos) -> Option<CullMesher<'c, T>> {
        let chunk = self.world.chunks.get(&pos)?;
        let top = self.world.chunks.get(&(pos + Vector3::unit_y()))?;
        let bottom = self.world.chunks.get(&(pos - Vector3::unit_y()))?;
        let right = self.world.chunks.get(&(pos + Vector3::unit_x()))?;
        let left = self.world.chunks.get(&(pos - Vector3::unit_x()))?;
        let front = self.world.chunks.get(&(pos + Vector3::unit_z()))?;
        let back = self.world.chunks.get(&(pos - Vector3::unit_z()))?;

        Some(CullMesher::new(chunk, top, bottom, left, right, front, back))
    }

    /// Add chunks to generator queue that are not already generated
    fn queue_in_range(&mut self) {
        // Generate chunks one outside the radius so the mesher can properly
        // mesh all the chunks in the radius
        println!("self.center = {:?}, self.radius = {:?}", self.center, self.radius);
        for x in self.center.x - self.radius - 1..self.center.x + self.radius + 1 {
            for y in self.center.y - self.radius - 1..self.center.y + self.radius + 1 {
                for z in self.center.z - self.radius - 1..self.center.z + self.radius + 1 {
                    self.world.queue(Vector3::new(x, y, z));
                }
            }
        }

        for x in self.center.x - self.radius..self.center.x + self.radius {
            for y in self.center.y - self.radius..self.center.y + self.radius {
                for z in self.center.z - self.radius..self.center.z + self.radius {
                    let pos = Vector3::new(x, y, z);
                    if !self.meshes.contains_key(&pos) {
                        self.queue.insert(Vector3::new(x, y, z));
                    }
                }
            }
        }
    }

    fn unload(&mut self) {
        let to_unload: SmallVec<[ChunkPos; 32]> = self.world.chunks.keys()
            .filter(|p| {
                p.x > self.center.x + self.radius + 1 ||
                p.x < self.center.x - self.radius - 1 ||
                p.y > self.center.y + self.radius + 1 ||
                p.y < self.center.y - self.radius - 1 ||
                p.z > self.center.z + self.radius + 1 ||
                p.z < self.center.z - self.radius - 1
            })
            .map(|p| *p).collect();
        for pos in &to_unload {
            self.meshes.remove(pos);
        }
        self.world.unload(to_unload);
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

        self.world.tick();
        if self.queue.len() > 0 {
            // This sets up all the meshers, filtering out any mesher that doesn't
            // have all the neighbor chunks generated yet. All the mess with passing
            // around `pos` in a tuple is because we need to know which meshes belong
            // to which chunks.
            let meshers = self.queue.iter()
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
                self.queue.remove(&pos);
                println!("Meshed chunk ({:?}), meshes: {}", pos, self.meshes.len());
            }
        }

        if self.dirty.len() > 0 {
            // Update all dirty at once to avoid problems where unfinished meshes flash
            // after a block is destroyed.
            // NOTE: any dirty positions that were marked in the one-chunk gap between
            // meshed chunks and nothing will get removed here, but this is not a problem
            // since we haven't meshed them anyways.
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

    pub fn draw(&mut self, pipeline: &mut LinkedProgram) -> GlResult<()> {
        pipeline.set_uniform("u_Transform", &Matrix4::<f32>::identity());
        for mesh in self.meshes.values() {
            if mesh.vertex_count() > 0 {
                mesh.draw_with(&pipeline)?;
            }
        }
        Ok(())
    }
}
