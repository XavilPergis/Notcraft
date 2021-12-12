use noise::{Fbm, NoiseFn, Perlin};

use crate::{
    engine::world::{
        chunk::{SIZE, VOLUME},
        Chunk,
    },
    util,
};

use super::{
    chunk::ChunkType,
    registry::{BlockId, BlockRegistry, AIR},
    ChunkPos,
};

#[derive(Clone, Debug)]
pub struct NoiseGenerator {
    stone_id: BlockId,
    dirt_id: BlockId,
    grass_id: BlockId,
}

struct NoiseSampler<F> {
    noise_fn: F,
    scale: f32,
}

impl<F> NoiseSampler<F> {
    fn new(noise_fn: F) -> Self {
        Self {
            noise_fn,
            scale: 1.0,
        }
    }

    fn with_scale(mut self, scale: f32) -> Self {
        self.scale = scale;
        self
    }

    fn sample(&self, x: f32, z: f32) -> f32
    where
        F: NoiseFn<[f64; 2]>,
    {
        let mapped_x = (x * self.scale) as f64;
        let mapped_z = (z * self.scale) as f64;
        self.noise_fn.get([mapped_x, mapped_z]) as f32
    }
}

impl NoiseGenerator {
    pub fn new_default(registry: &BlockRegistry) -> Self {
        Self {
            stone_id: registry.get_id("stone"),
            dirt_id: registry.get_id("dirt"),
            grass_id: registry.get_id("grass"),
        }
    }

    fn block_from_surface_dist(&self, distance: f32) -> BlockId {
        if distance < -4.0 {
            self.stone_id
        } else if distance < -1.0 {
            self.dirt_id
        } else if distance < 0.0 {
            self.grass_id
        } else {
            AIR
        }
    }

    pub fn make_chunk(&self, pos: ChunkPos) -> ChunkType {
        let base = pos.base().0;

        let mix_noise = NoiseSampler::new(Perlin::new()).with_scale(0.001);
        let rolling_noise = NoiseSampler::new(Perlin::new()).with_scale(0.003);
        let mountainous_noise = NoiseSampler::new(Fbm::new()).with_scale(0.007);

        // if base.y as f32 - noise_min > 0.0 {
        //     return ChunkType::Homogeneous(block::AIR);
        // }

        let mut chunk_data = Vec::with_capacity(VOLUME);
        for x in 0..SIZE {
            for z in 0..SIZE {
                let (x, z) = (base.x as f32 + x as f32, base.z as f32 + z as f32);

                let a = 20.0 * rolling_noise.sample(x, z);
                let b = 200.0 * (mountainous_noise.sample(x, z) * 0.5 + 0.5);
                let surface_height = a + mix_noise.sample(x, z) * b;

                for y in 0..SIZE {
                    let id =
                        self.block_from_surface_dist(base.y as f32 + y as f32 - surface_height);
                    chunk_data.push(id);
                }
            }
        }

        assert!(!chunk_data.is_empty());
        ChunkType::Array(Chunk::new(chunk_data))
    }
}
