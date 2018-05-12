use engine::chunk::CHUNK_SIZE;
use cgmath::Vector3;
use smallvec::SmallVec;
use std::collections::HashMap;
use engine::ChunkPos;
use cgmath::Point3;
use noise::NoiseFn;
use engine::Voxel;
use engine::chunk::Chunk;

pub struct Layer<T> {
    crate data: Box<[T]>,
}

impl<T> Layer<T> {
    fn set(&mut self, x: usize, z: usize, voxel: T) {
        self.data[CHUNK_SIZE * z + x] = voxel;
    }
}

impl<T: Default + Copy> Default for Layer<T> {
    fn default() -> Self {
        Layer {
            data: vec![T::default(); CHUNK_SIZE*CHUNK_SIZE].into()
        }
    }
}

pub struct Overflow<T> {
    // chunk position
    crate overflow_layers: HashMap<ChunkPos, HashMap<usize, Layer<T>>>,
}

impl<T> Overflow<T> {
    pub fn new() -> Self { Overflow { overflow_layers: HashMap::new() } }

    fn set(&mut self, chunk: ChunkPos, offset: Vector3<usize>, voxel: T) where T: Default + Copy {
        debug_assert!(offset.x < CHUNK_SIZE && offset.y < CHUNK_SIZE && offset.z < CHUNK_SIZE);
        // If the entry for the chunk does't exist, create it
        let layers = self.overflow_layers.entry(chunk).or_insert_with(|| HashMap::new());
        // If the entry for the layer doesn't exist, create it
        let layer = layers.entry(offset.y).or_insert_with(|| Layer::default());
        layer.set(offset.x, offset.z, voxel);
    }
}

pub trait ChunkGenerator<T> {
    fn generate(&self, pos: ChunkPos) -> Chunk<T>;
}

pub trait ChunkDecorator<T> {
    fn decorate(&self, pos: ChunkPos, chunk: &mut Chunk<T>) -> Overflow<T>;
}

struct OverflowChunk<'c, T: 'c> {
    chunk: &'c mut Chunk<T>,
    overflow: Overflow<T>,
}

impl<'c, T: 'c> OverflowChunk<'c, T> {
    pub fn new(chunk: &'c mut Chunk<T>) -> Self {
        OverflowChunk { chunk, overflow: Overflow::new() }
    }

    pub fn set(&mut self, pos: Point3<i32>, voxel: T) {
        
    }
}

impl<'c, T: 'c> From<OverflowChunk<'c, T>> for Overflow<T> {
    fn from(oc: OverflowChunk<T>) -> Overflow<T> { oc.overflow }
}

pub struct OctaveNoise<N> {
    pub lacunarity: f64,
    pub persistance: f64,
    pub height: f64,
    pub octaves: usize,
    crate noise: N,
}

impl<N: NoiseFn<[f64; 3]>> NoiseFn<[f64; 3]> for OctaveNoise<N> {
    fn get(&self, point: [f64; 3]) -> f64 {
        let mut total = 0.0;
        let x = point[0];
        let y = point[1];
        let z = point[2];
        
        for octave in 0..self.octaves-1 {
            let x = x * self.lacunarity.powf(octave as f64);
            let y = y * self.lacunarity.powf(octave as f64);
            let z = z * self.lacunarity.powf(octave as f64);
            total += self.height * self.persistance.powf(octave as f64) * self.noise.get([x, y, z]);
        }

        total
    }
}

impl<N: NoiseFn<[f64; 2]>> NoiseFn<[f64; 2]> for OctaveNoise<N> {
    fn get(&self, point: [f64; 2]) -> f64 {
        let mut total = 0.0;
        let x = point[0];
        let z = point[1];
        
        for octave in 0..self.octaves-1 {
            let x = x * self.lacunarity.powf(octave as f64);
            let z = z * self.lacunarity.powf(octave as f64);
            total += self.height * self.persistance.powf(octave as f64) * self.noise.get([x, z]);
        }

        total
    }
}
