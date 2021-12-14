use std::{
    collections::HashMap,
    sync::{Arc, RwLock},
    time::Instant,
};

use noise::{Fbm, NoiseFn, Perlin};

use crate::engine::world::chunk::{CHUNK_LENGTH, CHUNK_VOLUME};

use super::{
    chunk::ChunkKind,
    registry::{BlockId, BlockRegistry, AIR},
    ChunkPos, VoxelWorld,
};

#[derive(Debug)]
struct CachedSurface {
    added_at: Instant,
    min: f32,
    max: f32,
    data: Arc<[f32]>,
}

#[derive(Debug)]
pub struct NoiseGenerator {
    stone_id: BlockId,
    dirt_id: BlockId,
    grass_id: BlockId,

    surface_height_cache: RwLock<HashMap<[i32; 2], CachedSurface>>,
}

impl NoiseGenerator {
    fn get_surface_heights(&self, x: i32, z: i32) -> (f32, f32, Arc<[f32]>) {
        if let Some(cached) = self.surface_height_cache.read().unwrap().get(&[x, z]) {
            return (cached.min, cached.max, Arc::clone(&cached.data));
        }

        let mix_noise = NoiseSampler::new(Perlin::new()).with_scale(0.001);
        let rolling_noise = NoiseSampler::new(Perlin::new()).with_scale(0.003);
        let mountainous_noise = NoiseSampler::new(Fbm::new()).with_scale(0.007);

        let mut min = f32::MAX;
        let mut max = f32::MIN;

        let mut heights = Vec::with_capacity(CHUNK_LENGTH * CHUNK_LENGTH);
        for dx in 0..CHUNK_LENGTH {
            for dz in 0..CHUNK_LENGTH {
                let (x, z) = (
                    CHUNK_LENGTH as f32 * x as f32 + dx as f32,
                    CHUNK_LENGTH as f32 * z as f32 + dz as f32,
                );

                let a = 20.0 * rolling_noise.sample(x, z);
                let b = 200.0 * (mountainous_noise.sample(x, z) * 0.5 + 0.5);
                let result = a + mix_noise.sample(x, z) * b;

                min = f32::min(min, result);
                max = f32::max(max, result);

                heights.push(result);
            }
        }

        self.surface_height_cache
            .write()
            .unwrap()
            .insert([x, z], CachedSurface {
                added_at: Instant::now(),
                min,
                max,
                data: heights.into_boxed_slice().into(),
            });

        self.get_surface_heights(x, z)
    }

    fn evict_old_items(&self) {
        let mut cache = self.surface_height_cache.write().unwrap();
        cache.retain(|_, cached| cached.added_at.elapsed().as_secs() < 10);
    }
}

#[legion::system]
pub fn update_surface_cache(#[resource] world: &VoxelWorld) {
    world.noise_generator.evict_old_items();
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
            surface_height_cache: Default::default(),
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

    pub fn make_chunk(&self, pos: ChunkPos) -> ChunkKind {
        let base_y = pos.origin().y as f32;
        let (min, max, heights) = self.get_surface_heights(pos.x, pos.z);

        if base_y > max {
            return ChunkKind::Homogeneous(AIR);
        } else if (base_y + CHUNK_LENGTH as f32) < min {
            return ChunkKind::Homogeneous(self.stone_id);
        }

        let mut chunk_data = Vec::with_capacity(CHUNK_VOLUME);
        for xz in 0..CHUNK_LENGTH * CHUNK_LENGTH {
            let surface_height = heights[xz];
            chunk_data.extend(
                (0..CHUNK_LENGTH)
                    .map(|y| self.block_from_surface_dist(base_y + y as f32 - surface_height)),
            );
        }

        assert!(!chunk_data.is_empty());
        ChunkKind::Array(chunk_data.into_boxed_slice().try_into().unwrap())
    }
}
