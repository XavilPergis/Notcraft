use std::sync::Mutex;
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

pub trait AsyncChunkLoader<T> {
    fn request(&self, chunk: ChunkPos);
    fn finished(&self) -> Vec<(ChunkPos, Chunk<T>)>;
}

pub struct World<T> {
    crate chunks: HashMap<ChunkPos, Chunk<T>>,
    crate generator: Box<AsyncChunkLoader<T> + Send + Sync>,
}

impl<T> World<T> {
    pub fn new(generator: Box<AsyncChunkLoader<T> + Send + Sync>) -> Self {
        World { chunks: HashMap::new(), generator }
    }

    pub fn flush_finished(&mut self) {
        self.chunks.extend(self.generator.finished().into_iter());
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

    crate fn set_chunk(&mut self, pos: ChunkPos, chunk: Chunk<T>) {
        self.chunks.insert(pos, chunk);
    }

    crate fn chunk_exists(&self, pos: ChunkPos) -> bool {
        self.chunks.contains_key(&pos)
    }
}

impl<T> AsyncChunkLoader<T> for WorldGenerator<T> {
    fn request(&self, pos: ChunkPos) {
        if self.queue.lock().unwrap().insert(pos) {
            self.gen_tx.send(pos).unwrap();
        }
    }

    fn finished(&self) -> Vec<(ChunkPos, Chunk<T>)> {
        let finished = self.gen_rx.iter().collect();
        let mut queue = self.queue.lock().unwrap();
        for &(pos, _) in &finished { queue.remove(&pos); }
        finished
    }
}

pub struct WorldGenerator<T> {
    queue: Mutex<HashSet<ChunkPos>>,
    gen_tx: mpsc::Sender<ChunkPos>,
    gen_rx: mpsc::Receiver<(ChunkPos, Chunk<T>)>,
}

impl<T> WorldGenerator<T> {
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

        WorldGenerator {
            queue: Mutex::new(HashSet::new()),
            gen_tx: req_tx,
            gen_rx: rx,
        }
    }
}
