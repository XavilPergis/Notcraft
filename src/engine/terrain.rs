use engine::Voxel;
use noise::NoiseFn;
use cgmath::Vector3;
use engine::chunk::Chunk;

pub trait ChunkGenerator<T> {
    fn generate(&self, pos: Vector3<i32>) -> Chunk<T>;
}

pub struct NoiseGenerator<F, N> {
    pub lacunarity: f64,
    pub persistance: f64,
    pub height: f64,
    pub octaves: usize,
    noise: N,
    gen_func: F,
}

impl<F, N> NoiseGenerator<F, N> {
    pub fn new_default(noise: N, gen_func: F) -> Self {
        NoiseGenerator {
            noise, gen_func,
            lacunarity: 2.0,
            persistance: 0.5,
            height: 30.0,
            octaves: 4,
        }
    }
}

impl<V: Voxel, F, N> ChunkGenerator<V> for NoiseGenerator<F, N>
    where F: Fn(Vector3<f64>, f64) -> V, N: NoiseFn<[f64; 2]> {
    fn generate(&self, pos: Vector3<i32>) -> Chunk<V> {
        const SIZE: i32 = super::chunk::CHUNK_SIZE as i32;
        let mut buffer = Vec::with_capacity(50*50*50);
        for z in 0..SIZE {
            for y in 0..SIZE {
                for x in 0..SIZE {
                    let x = ((SIZE*pos.x) as f64 + x as f64) / (SIZE as f64);
                    let y = (pos.y*SIZE) as f64 + y as f64;
                    let z = ((SIZE*pos.z) as f64 + z as f64) / (SIZE as f64);
                    let mut total = 0.0;
                    
                    for octave in 0..self.octaves-1 {
                        let x = x * self.lacunarity.powf(octave as f64);
                        let z = z * self.lacunarity.powf(octave as f64);
                        total += self.height * self.persistance.powf(octave as f64) * self.noise.get([x, z]);
                    }

                    let bpos = Vector3::new(x, y, z);

                    buffer.push((self.gen_func)(bpos, total));
                }
            }
        }

        Chunk::new(pos.x, pos.y, pos.z, buffer)
    }
}
