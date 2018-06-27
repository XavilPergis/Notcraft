use cgmath::Vector3;
use std::sync::RwLock;
use std::sync::Arc;
use std::sync::mpsc;
use std::collections::HashSet;
use std::collections::HashMap;
use cgmath::Point3;
use engine::terrain::ChunkGenerator;
use engine::chunk::Chunk;
use engine::{WorldPos, ChunkPos};

pub struct World<T> {
    crate chunks: HashMap<ChunkPos, Chunk<T>>,
}

impl<T> World<T> {
    pub fn new() -> Self {
        World { chunks: HashMap::new() }
    }

    pub fn set_voxel(&mut self, pos: WorldPos, voxel: T) where T: PartialEq {
        let (cpos, bpos) = ::util::get_chunk_pos(pos);
        // get the chunk the voxel is in, if it is loaded
        if let Some(chunk) = self.chunks.get_mut(&cpos) {
            let pos = (bpos.x as usize, bpos.y as usize, bpos.z as usize);
            chunk[pos] = voxel;
        }
    }

    /// Forcefully unload all the chunks in `radius` around `center`
    pub fn unload(&mut self, center: WorldPos, radii: Vector3<i32>) {
        self.chunks.retain(|&pos, _| ::util::in_range(pos, center, radii));
    }

    pub fn get_voxel(&self, pos: WorldPos) -> Option<&T> {
        let (cpos, bpos) = ::util::get_chunk_pos(pos);
        let pos = (bpos.x as usize, bpos.y as usize, bpos.z as usize);
        self.chunks.get(&cpos).map(|chunk| &chunk[pos])
    }

    pub fn get_voxel_mut(&mut self, pos: WorldPos) -> Option<&mut T> {
        let (cpos, bpos) = ::util::get_chunk_pos(pos);
        let pos = (bpos.x as usize, bpos.y as usize, bpos.z as usize);
        self.chunks.get_mut(&cpos).map(|chunk| &mut chunk[pos])
    }

    pub fn around_voxel<F: FnMut(WorldPos, &T)>(&self, pos: WorldPos, radius: u32, mut func: F) {
        let radius = radius as i32;
        for x in pos.x - radius..pos.x + radius {
            for y in pos.y - radius..pos.y + radius {
                for z in pos.z - radius..pos.z + radius {
                    let pos = Point3::new(x, y, z);
                    if let Some(voxel) = self.get_voxel(pos) {
                        func(pos, voxel);
                    }
                }
            }
        }
    }

    crate fn chunk_exists(&self, pos: ChunkPos) -> bool {
        self.chunks.contains_key(&pos)
    }

    crate fn tick(&mut self) {
        // for (pos, entity) in &mut self.tile_entities {
        //     entity.update(self);
        // }
    }
}

pub struct WorldGenerator<T> {
    queue: HashSet<ChunkPos>,
    chunk_pos: Arc<RwLock<ChunkPos>>,
    radii: Arc<RwLock<Vector3<i32>>>,
    gen_tx: mpsc::Sender<ChunkPos>,
    gen_rx: mpsc::Receiver<(ChunkPos, Chunk<T>)>,
}

impl<T> WorldGenerator<T> {
        pub fn new<G: ChunkGenerator<T> + Send + 'static>(generator: G, radii: Arc<RwLock<Vector3<i32>>>, chunk_pos: Arc<RwLock<ChunkPos>>) -> Self where T: Send + 'static {
        use std::thread;
        let (req_tx, req_rx) = mpsc::channel();
        let (tx, rx) = mpsc::channel();
        let thread_chunk_pos = chunk_pos.clone();
        let thread_radii = radii.clone();
        thread::spawn(move || {
            while let Ok(request) = req_rx.recv() {
                // Deref and drop the guard so we don't hold up the lock while we generate
                // the chunk
                let pos = *thread_chunk_pos.read().unwrap();
                // Skip if not in range
                if !::util::in_range(request, pos, *thread_radii.read().unwrap()) { continue; }
                // Err means the rx has hung up, so we can just shut down this thread
                // if that happens
                match tx.send((request, generator.generate(request))) {
                    Ok(_) => (),
                    Err(_) => break,
                }
            }    
        });

        WorldGenerator {
            queue: HashSet::new(),
            chunk_pos,
            radii,
            gen_tx: req_tx,
            gen_rx: rx,
        }
    }

    /// NOTE: It is your responsibility to not queue chunks that have already been generated
    pub fn queue(&mut self, pos: ChunkPos) {
        // Only send this request off to the generator thread if it was not previously queued
        if self.queue.insert(pos) {
            self.gen_tx.send(pos).unwrap();
        }
    }

    crate fn update_world(&mut self, world: &mut World<T>) {
        for (pos, chunk) in self.gen_rx.try_iter() {
            println!("Generated chunk at ({:?}) => queue.len() = {}", pos, self.queue.len());
            world.chunks.insert(pos, chunk);
            self.queue.remove(&pos);
        }
        let chunk_pos = *self.chunk_pos.read().unwrap();
        let radii = *self.radii.read().unwrap();
        self.queue.retain(|&pos| ::util::in_range(chunk_pos, pos, radii))
    }

}
