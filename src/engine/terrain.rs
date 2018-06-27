use engine::chunk::CHUNK_SIZE;
use cgmath::Vector3;
use smallvec::SmallVec;
use std::collections::HashMap;
use engine::ChunkPos;
use cgmath::Point3;
use noise::NoiseFn;
use engine::Voxel;
use engine::chunk::Chunk;

pub trait ChunkGenerator<T> {
    fn generate(&self, pos: ChunkPos) -> Chunk<T>;
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
