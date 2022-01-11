use super::{
    chunk::ChunkData,
    registry::{BlockId, BlockRegistry, AIR},
    ChunkHeightmapPos, ChunkPos,
};
use crate::world::chunk::{CHUNK_LENGTH, CHUNK_VOLUME};
use noise::{Fbm, NoiseFn, Perlin};
use rand::{rngs::SmallRng, FromEntropy, Rng};
use std::{
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
    time::{Duration, Instant},
};

#[derive(Clone, Debug)]
pub struct SurfaceHeightmap {
    min: i32,
    max: i32,
    timestamp: Arc<AtomicU64>,
    data: Arc<[i32]>,
}

impl SurfaceHeightmap {
    pub fn data(&self) -> &Arc<[i32]> {
        &self.data
    }
}

#[derive(Debug)]
pub struct SurfaceHeighmapCache {
    time_reference: Instant,
    heightmaps: flurry::HashMap<ChunkHeightmapPos, SurfaceHeightmap>,
}

impl Default for SurfaceHeighmapCache {
    fn default() -> Self {
        Self {
            time_reference: Instant::now(),
            heightmaps: Default::default(),
        }
    }
}

fn generate_surface_heights(
    cache: &SurfaceHeighmapCache,
    pos: ChunkHeightmapPos,
) -> SurfaceHeightmap {
    let mix_noise = NoiseSampler::new(Perlin::new()).with_scale(0.0001);
    let rolling_noise = NoiseSampler::new(Perlin::new()).with_scale(0.0003);

    let mountainous_noise_unwarped = NoiseSampler::new(Fbm::new()).with_scale(0.002);
    let mountainous_noise = NoiseSampler::new(Fbm::new()).with_scale(0.001);
    let warp_noise_x = NoiseSampler::new(Perlin::new())
        .with_offset([0.0, 0.5])
        .with_scale(0.003);
    let warp_noise_z = NoiseSampler::new(Perlin::new())
        .with_offset([0.5, 0.0])
        .with_scale(0.003);

    let mut min = i32::MAX;
    let mut max = i32::MIN;

    let mut heights = Vec::with_capacity(CHUNK_LENGTH * CHUNK_LENGTH);
    for dx in 0..CHUNK_LENGTH {
        for dz in 0..CHUNK_LENGTH {
            let (x, z) = (
                CHUNK_LENGTH as f32 * pos.x as f32 + dx as f32,
                CHUNK_LENGTH as f32 * pos.z as f32 + dz as f32,
            );

            let wx = x + 200.0 * warp_noise_x.sample(x, z);
            let wz = z + 200.0 * warp_noise_z.sample(x, z);

            let warped = 2000.0 * (mountainous_noise.sample(wx, wz) * 0.5 + 0.5);
            let mountain = 800.0 * (mountainous_noise_unwarped.sample(x, z) * 0.5 + 0.5);
            let rolling = 100.0 * rolling_noise.sample(x, z);
            let result = rolling + mix_noise.sample(x, z) * (warped + mountain);

            // let result = 100.0 * f32::sin(x / 30.0) * f32::cos(z / 30.0);

            let result = result.floor() as i32;

            min = i32::min(min, result);
            max = i32::max(max, result);

            heights.push(result);
        }
    }

    SurfaceHeightmap {
        min,
        max,
        timestamp: Arc::new(cache.timestamp().into()),
        data: heights.into_boxed_slice().into(),
    }
}

impl SurfaceHeighmapCache {
    pub fn surface_heights(&self, pos: ChunkHeightmapPos) -> SurfaceHeightmap {
        if let Some(cached) = self.heightmaps.pin().get(&pos) {
            cached.timestamp.store(self.timestamp(), Ordering::SeqCst);
            return SurfaceHeightmap::clone(cached);
        } else {
            let surface_heights = generate_surface_heights(self, pos);
            self.heightmaps.pin().insert(pos, surface_heights);
            self.surface_heights(pos)
        }
    }

    fn timestamp(&self) -> u64 {
        self.time_reference.elapsed().as_secs()
    }

    pub fn evict_after(&self, delay: Duration) {
        self.heightmaps.pin().retain(|_, val| {
            self.timestamp() - val.timestamp.load(Ordering::SeqCst) > delay.as_secs()
        });
    }
}

struct NoiseSampler<F> {
    noise_fn: F,
    offset: [f32; 2],
    scale: f32,
}

impl<F> NoiseSampler<F> {
    fn new(noise_fn: F) -> Self {
        Self {
            noise_fn,
            offset: [0.0, 0.0],
            scale: 1.0,
        }
    }

    fn with_scale(mut self, scale: f32) -> Self {
        self.offset[0] *= scale;
        self.offset[1] *= scale;
        self.scale = scale;
        self
    }

    fn with_offset<I: Into<[f32; 2]>>(mut self, offset: I) -> Self {
        self.offset = offset.into();
        self
    }

    fn sample(&self, x: f32, z: f32) -> f32
    where
        F: NoiseFn<[f64; 2]>,
    {
        let [dx, dz] = self.offset;
        let mapped_x = (dx + x * self.scale) as f64;
        let mapped_z = (dz + z * self.scale) as f64;
        self.noise_fn.get([mapped_x, mapped_z]) as f32
    }
}

#[derive(Debug)]
pub struct ChunkGenerator {
    stone_id: BlockId,
    dirt_id: BlockId,
    grass_id: BlockId,
    water_id: BlockId,
    sand_id: BlockId,
    detail_grass_id: BlockId,
}

impl ChunkGenerator {
    pub fn new_default(registry: &BlockRegistry) -> Self {
        Self {
            stone_id: registry.get_id("stone"),
            dirt_id: registry.get_id("dirt"),
            grass_id: registry.get_id("grass"),
            water_id: registry.get_id("water"),
            sand_id: registry.get_id("sand"),
            detail_grass_id: registry.get_id("detail_grass"),
        }
    }

    fn pick_block(&self, rng: &mut SmallRng, y: i32, surface: i32) -> BlockId {
        let distance = y - surface;
        if distance < -4 {
            self.stone_id
        } else if distance < -1 {
            self.dirt_id
        } else if distance < 0 {
            if y < CHUNK_LENGTH as i32 + 3 {
                self.sand_id
            } else {
                self.grass_id
            }
        } else if y < CHUNK_LENGTH as i32 {
            self.water_id
        } else if distance < 1 {
            if rng.gen_bool(1.0 / 3.0) {
                self.detail_grass_id
            } else {
                AIR
            }
        } else {
            AIR
        }
    }

    pub fn make_chunk(&self, pos: ChunkPos, heights: SurfaceHeightmap) -> ChunkData<BlockId> {
        let base_y = pos.origin().y;

        let mut rng = SmallRng::from_entropy();

        if base_y > heights.max {
            if pos.y < 1 {
                return ChunkData::Homogeneous(self.water_id);
            } else {
                return ChunkData::Homogeneous(AIR);
            }
        } else if (base_y + CHUNK_LENGTH as i32) < heights.min {
            return ChunkData::Homogeneous(self.stone_id);
        }

        let mut chunk_data = Vec::with_capacity(CHUNK_VOLUME);
        for xz in 0..CHUNK_LENGTH * CHUNK_LENGTH {
            let surface_height = heights.data[xz];
            chunk_data.extend(
                (0..CHUNK_LENGTH)
                    .map(|y| self.pick_block(&mut rng, base_y + y as i32, surface_height)),
            );
        }

        assert!(!chunk_data.is_empty());
        ChunkData::Array(chunk_data.into_boxed_slice().try_into().unwrap())
    }
}
