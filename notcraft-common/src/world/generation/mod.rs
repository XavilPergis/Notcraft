use self::spline::Spline;

use super::{
    chunk::ChunkData,
    registry::{BlockId, BlockRegistry, AIR_BLOCK},
    BlockPos, ChunkPos, ChunkSectionPos,
};
use crate::{
    codec::{
        encode::{Encode, Encoder},
        NodeKind,
    },
    prelude::*,
    world::chunk::{CHUNK_LENGTH, CHUNK_LENGTH_2, CHUNK_LENGTH_3},
};
use noise::{Fbm, MultiFractal, NoiseFn, OpenSimplex, Perlin};
use rand::{rngs::SmallRng, FromEntropy, Rng, SeedableRng};
use std::{
    collections::hash_map::DefaultHasher,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
    time::{Duration, Instant},
};

pub mod spline;

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
    heightmaps: flurry::HashMap<ChunkPos, SurfaceHeightmap>,
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
    seed: u64,
    pos: ChunkPos,
    shaping_curve: &Spline,
) -> SurfaceHeightmap {
    // let mix_noise = NoiseSampler::seeded(seed, Perlin::new()).with_scale(0.0001);
    // let rolling_noise = NoiseSampler::seeded(seed,
    // Perlin::new()).with_scale(0.0003);

    // let mountainous_noise_unwarped = NoiseSampler::seeded(seed,
    // Fbm::new()).with_scale(0.002); let mountainous_noise =
    // NoiseSampler::seeded(seed, Fbm::new()).with_scale(0.001);
    // let warp_noise_x = NoiseSampler::seeded(seed, Perlin::new())
    //     .with_offset([0.0, 0.5])
    //     .with_scale(0.003);
    // let warp_noise_z = NoiseSampler::seeded(seed, Perlin::new())
    //     .with_offset([0.5, 0.0])
    //     .with_scale(0.003);
    let noise = NoiseSamplerN::seeded(seed, Fbm::new().set_octaves(4)).with_scale(0.004);

    let mut min = i32::MAX;
    let mut max = i32::MIN;

    let mut heights = Vec::with_capacity(CHUNK_LENGTH_2);
    for dx in 0..CHUNK_LENGTH {
        for dz in 0..CHUNK_LENGTH {
            let (x, z) = (
                CHUNK_LENGTH as f32 * pos.x as f32 + dx as f32,
                CHUNK_LENGTH as f32 * pos.z as f32 + dz as f32,
            );

            // let wx = x + 200.0 * warp_noise_x.sample(x, z);
            // let wz = z + 200.0 * warp_noise_z.sample(x, z);

            // let warped = 2000.0 * (mountainous_noise.sample(wx, wz) * 0.5 + 0.5);
            // let mountain = 800.0 * (mountainous_noise_unwarped.sample(x, z) * 0.5 + 0.5);
            // let rolling = 100.0 * rolling_noise.sample(x, z);
            // let result = rolling + mix_noise.sample(x, z) * (warped + mountain);
            let result = shaping_curve.sample(noise.sample([x, z]));

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
    pub fn surface_heights(
        &self,
        seed: u64,
        shaping_curve: &Spline,
        pos: ChunkPos,
    ) -> SurfaceHeightmap {
        if let Some(cached) = self.heightmaps.pin().get(&pos) {
            cached.timestamp.store(self.timestamp(), Ordering::SeqCst);
            return SurfaceHeightmap::clone(cached);
        } else {
            let surface_heights = generate_surface_heights(self, seed, pos, shaping_curve);
            self.heightmaps.pin().insert(pos, surface_heights);
            self.surface_heights(seed, shaping_curve, pos)
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

struct NoiseSamplerN<F, const D: usize> {
    noise: F,
    offset: [f32; D],
    scale: f32,
}

impl<F, const D: usize> NoiseSamplerN<F, D>
where
    F: noise::Seedable,
{
    pub fn seeded(seed: u64, noise: F) -> Self {
        Self {
            noise: noise.set_seed(seed as u32),
            offset: [0.0; D],
            scale: 1.0,
        }
    }
}

impl<F> NoiseSamplerN<F, 3> {
    pub fn sample_block(&self, pos: BlockPos) -> f32
    where
        F: NoiseFn<[f64; 3]>,
    {
        self.sample([pos.x as f32, pos.y as f32, pos.z as f32])
    }
}

impl<F, const D: usize> NoiseSamplerN<F, D> {
    pub fn with_scale(mut self, scale: f32) -> Self {
        self.scale = scale;
        self
    }

    pub fn with_offset<I: Into<[f32; D]>>(mut self, offset: I) -> Self {
        self.offset = offset.into();
        self
    }

    pub fn sample<I>(&self, pos: I) -> f32
    where
        [f32; D]: From<I>,
        F: NoiseFn<[f64; D]>,
    {
        let mut pos = <[f32; D]>::from(pos);
        for i in 0..D {
            pos[i] = (self.offset[i] + pos[i]) * self.scale;
        }
        self.noise.get(pos.map(|elem| elem as f64)) as f32
    }
}

impl<F, const D: usize> From<F> for NoiseSamplerN<F, D>
where
    F: NoiseFn<[f64; D]>,
{
    fn from(noise: F) -> Self {
        Self {
            noise,
            offset: [0.0; D],
            scale: 1.0,
        }
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
            stone_id: registry.lookup("stone"),
            dirt_id: registry.lookup("dirt"),
            grass_id: registry.lookup("grass"),
            water_id: registry.lookup("water"),
            sand_id: registry.lookup("sand"),
            detail_grass_id: registry.lookup("detail_grass"),
        }
    }

    fn pick_block<F: NoiseFn<[f64; 3]>>(
        &self,
        rng: &mut SmallRng,
        open_noise: &NoiseSamplerN<F, 3>,
        stringy_noise: &NoiseSamplerN<F, 3>,
        pos: BlockPos,
        surface: i32,
    ) -> BlockId {
        let distance = pos.y - surface;
        if distance < -20 && {
            let d1 = open_noise.sample_block(pos);
            let d2 = stringy_noise.sample_block(pos);

            d1.abs() < 0.05 && d2.abs() < 0.05

            // let density = stringy_noise.sample_block(pos);
            // let stringy_bias = util::clamp(
            //     0.0,
            //     1.0,
            //     util::remap(-1.0, 1.0, -1.5, 3.0,
            // open_noise.sample_block(pos)), );
            // let distance_bias = util::clamp(0.0, 1.0, -distance as f32 /
            // 100.0);

            // let density = util::lerp(1.0, density, distance_bias *
            // stringy_bias); density.abs() < 0.02
        } {
            AIR_BLOCK
        } else if distance < 0 {
            self.stone_id
        } else {
            AIR_BLOCK
        }
        // if distance < -4 {
        //     self.stone_id
        // } else if distance < -1 {
        //     self.dirt_id
        // } else if distance < 0 {
        //     if y < CHUNK_LENGTH as i32 + 3 {
        //         self.sand_id
        //     } else {
        //         self.grass_id
        //     }
        // } else if y < CHUNK_LENGTH as i32 {
        //     self.water_id
        // } else if distance < 1 {
        //     if rng.gen_bool(1.0 / 3.0) {
        //         self.detail_grass_id
        //     } else {
        //         AIR_BLOCK
        //     }
        // } else {
        //     AIR_BLOCK
        // }
    }

    pub fn make_chunk(
        &self,
        seed: u64,
        pos: ChunkSectionPos,
        heights: &SurfaceHeightmap,
    ) -> ChunkData<BlockId> {
        let base_x = pos.origin().x;
        let base_y = pos.origin().y;
        let base_z = pos.origin().z;

        let stringy_noise = NoiseSamplerN::seeded(seed, OpenSimplex::new()).with_scale(0.015);
        let open_noise = NoiseSamplerN::seeded(seed + 3, OpenSimplex::new()).with_scale(0.015);

        let seed = make_chunk_section_seed(seed, pos);
        let mut rng = SmallRng::seed_from_u64(seed);

        if base_y > heights.max {
            // if pos.y < 1 {
            //     return ChunkData::Homogeneous(self.water_id);
            // } else {
            // }
            return ChunkData::Homogeneous(AIR_BLOCK);
        }
        //  else if (base_y + CHUNK_LENGTH as i32) < heights.min {
        //     return ChunkData::Homogeneous(self.stone_id);
        // }

        let mut chunk_data = Vec::with_capacity(CHUNK_LENGTH_3);
        for x in 0..CHUNK_LENGTH {
            for z in 0..CHUNK_LENGTH {
                let surface_height = heights.data[CHUNK_LENGTH * x + z];
                chunk_data.extend((0..CHUNK_LENGTH).map(|y| {
                    self.pick_block(
                        &mut rng,
                        &open_noise,
                        &stringy_noise,
                        BlockPos {
                            x: base_x + x as i32,
                            y: base_y + y as i32,
                            z: base_z + z as i32,
                        },
                        surface_height,
                    )
                }));
            }
        }

        assert!(!chunk_data.is_empty());
        ChunkData::Array(chunk_data.into_boxed_slice().try_into().unwrap())
    }
}

impl<W: std::io::Write> Encode<W> for SurfaceHeightmap {
    const KIND: NodeKind = NodeKind::List;

    fn encode(&self, encoder: Encoder<W>) -> Result<()> {
        encoder.encode_rle_list(self.data.iter().copied())
    }
}

fn make_chunk_section_seed(world_seed: u64, chunk: ChunkSectionPos) -> u64 {
    let mut rng = SmallRng::seed_from_u64(world_seed);
    let x = rng.gen::<u64>() ^ SmallRng::seed_from_u64(chunk.x as u64).gen::<u64>();
    let y = rng.gen::<u64>() ^ SmallRng::seed_from_u64(chunk.x as u64).gen::<u64>();
    let z = rng.gen::<u64>() ^ SmallRng::seed_from_u64(chunk.z as u64).gen::<u64>();
    x ^ y ^ z
}

fn make_chunk_seed(world_seed: u64, chunk: ChunkPos) -> u64 {
    let mut rng = SmallRng::seed_from_u64(world_seed);
    let x = rng.gen::<u64>() ^ SmallRng::seed_from_u64(chunk.x as u64).gen::<u64>();
    let z = rng.gen::<u64>() ^ SmallRng::seed_from_u64(chunk.z as u64).gen::<u64>();
    x ^ z
}
