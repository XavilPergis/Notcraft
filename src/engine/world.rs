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
    crate queue: HashSet<ChunkPos>,
    crate gen_tx: mpsc::Sender<ChunkPos>,
    crate gen_rx: mpsc::Receiver<(ChunkPos, Chunk<T>)>,
}

impl<T> World<T> {
    pub fn new<G: ChunkGenerator<T> + Send + 'static>(generator: G, chunk_pos: Arc<RwLock<ChunkPos>>) -> Self where T: Send + 'static {
        use std::thread;
        let (req_tx, req_rx) = mpsc::channel();
        let (tx, rx) = mpsc::channel();
        thread::spawn(move || {
            while let Ok(request) = req_rx.recv() {
                // Deref and drop the guard so we don't hold up the lock while we generate
                // the chunk
                let pos = *chunk_pos.read().unwrap();
                // Skip if not in range
                if !::util::in_range(request, pos, 4) { continue; }
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
        let (cpos, bpos) = ::util::get_chunk_pos(pos);
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

    pub fn unload(&mut self, center: WorldPos, radius: i32) {
        // Try removing any chunks from the queue that are out of range (player moved
        // really fast or teleported)
        self.queue.retain(|&pos| ::util::in_range(pos, center, radius));
        self.chunks.retain(|&pos, _| ::util::in_range(pos, center, radius));
    }

    crate fn tick(&mut self) {
        for (pos, chunk) in self.gen_rx.try_iter() {
            println!("Generated chunk at ({:?}) => queue.len() = {}", pos, self.queue.len());
            self.chunks.insert(pos, chunk);
            self.queue.remove(&pos);
        }
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

    pub fn around_voxel<U, F: Fn(WorldPos, &T) -> Option<U>>(&self, pos: WorldPos, radius: u32, func: F) -> Vec<U> {
        let radius = radius as i32;
        // TODO: We allocate here, but likely don't need to. It would be better if this
        // function returned an iterator...
        let mut buf = Vec::with_capacity((radius*radius*radius) as usize);
        for x in pos.x - radius..pos.x + radius {
            for y in pos.y - radius..pos.y + radius {
                for z in pos.z - radius..pos.z + radius {
                    let pos = Point3::new(x, y, z);
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
