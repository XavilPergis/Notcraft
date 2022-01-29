use crate::{
    codec::{
        encode::{Encode, Encoder},
        NodeKind,
    },
    prelude::*,
    Faces,
};
use serde::Deserialize;
use std::{
    collections::HashMap,
    fs::File,
    path::{Path, PathBuf},
    sync::Arc,
};

pub const AIR: BlockId = BlockId(0);

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash, Default)]
pub struct TexturePoolId(usize);

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash, Default)]
pub struct TextureId(pub usize);

#[derive(Clone, Debug, PartialEq, Default, Deserialize)]
#[serde(rename_all = "kebab-case")]
#[serde(default)]
pub struct BlockTextureReference {
    /// fallback texture to use when a face is not specified. if neither a face
    /// nor a default is provided, the "unknown" texture is used.
    default: Option<String>,

    /// references into a pool of textures that represent texture variants that
    /// are selected randomly.
    #[serde(flatten)]
    faces: Faces<Option<String>>,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum CollisionType {
    None,
    Solid,
    Liquid,
}

impl CollisionType {
    /// Returns `true` if the collision type is [`Solid`].
    ///
    /// [`Solid`]: CollisionType::Solid
    pub fn is_solid(&self) -> bool {
        matches!(self, Self::Solid)
    }

    /// Returns `true` if the collision type is [`Liquid`].
    ///
    /// [`Liquid`]: CollisionType::Liquid
    pub fn is_liquid(&self) -> bool {
        matches!(self, Self::Liquid)
    }
}

#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct BlockProperties {
    collision_type: CollisionType,
    #[serde(default)]
    liquid: bool,
    #[serde(default)]
    wind_sway: bool,
    #[serde(default)]
    block_light: u16,
    #[serde(default)]
    light_transmissible: bool,
}

#[derive(Copy, Clone, Debug, PartialEq, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum BlockMeshType {
    None,
    FullCube,
    Cross,
}

impl Default for BlockMeshType {
    fn default() -> Self {
        Self::FullCube
    }
}

#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct BlockDescription {
    name: String,
    properties: BlockProperties,
    #[serde(default)]
    mesh_type: BlockMeshType,

    /// a list of texture variants to use for this block.
    ///
    /// note that there is a variant list here, as well as inside
    /// [`BlockTextureReference`]. the difference is that here, variants change
    /// the textures for the entire block, which inside the texture reference,
    /// variants change the textures for just that block face.
    #[serde(default)]
    texture_variants: Option<Vec<BlockTextureReference>>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct BlockRegistryEntry {
    properties: BlockProperties,
    mesh_type: BlockMeshType,
    textures: Option<Vec<Faces<TexturePoolId>>>,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash, Default)]
pub struct BlockId(pub(crate) usize);

impl<W: std::io::Write> Encode<W> for BlockId {
    const KIND: NodeKind = NodeKind::UnsignedVarInt;

    fn encode(&self, encoder: Encoder<W>) -> Result<()> {
        encoder.encode(&self.0)
    }
}

fn add_texture_to_pool(reg: &mut BlockRegistry, pool: TexturePoolId, path: &Path) -> TextureId {
    let pool = &mut reg.texture_pools[pool.0];

    let id = TextureId(reg.texture_paths.len());
    reg.texture_indices.insert(path.into(), id);
    reg.texture_paths.push(path.into());

    pool.push(id);
    id
}

fn register_texture_pool(reg: &mut BlockRegistry, name: &str) -> TexturePoolId {
    if let Some(&idx) = reg.texture_pool_indices.get(name) {
        return idx;
    }

    let id = TexturePoolId(reg.texture_pools.len());
    reg.texture_pool_indices.insert(name.into(), id);
    reg.texture_pools.push(vec![]);

    id
}

fn make_entry(reg: &mut BlockRegistry, desc: BlockDescription) -> Result<BlockRegistryEntry> {
    let textures = match desc.texture_variants {
        Some(variants) => {
            let mut res = Vec::with_capacity(variants.len());
            for variant in variants {
                let default = variant
                    .default
                    .map(|path| reg.texture_pool_indices[&path])
                    .unwrap_or_else(|| reg.texture_pool_indices["unknown"]);
                res.push(variant.faces.map(|path| {
                    path.map(|path| reg.texture_pool_indices[&path])
                        .unwrap_or(default)
                }));
            }
            Some(res)
        }
        None => None,
    };

    Ok(BlockRegistryEntry {
        properties: desc.properties,
        mesh_type: desc.mesh_type,
        textures,
    })
}

#[derive(Clone, Debug, PartialEq, Default)]
pub struct BlockRegistry {
    name_map: HashMap<String, BlockId>,
    entries: Vec<BlockRegistryEntry>,

    // the order here is important: the indices will becomes layers in a texture array that holds
    // the actual texture data.
    texture_paths: Vec<PathBuf>,
    // texture *paths* to texture ID
    texture_indices: HashMap<PathBuf, TextureId>,

    // texture pool *names* to texture ID
    texture_pools: Vec<Vec<TextureId>>,
    texture_pool_indices: HashMap<String, TexturePoolId>,
}

#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(rename_all = "kebab-case")]
struct RegistryManifest {
    textures: HashMap<String, Vec<String>>,
    blocks: Vec<BlockDescription>,
}

pub fn load_registry<P: AsRef<Path>>(path: P) -> Result<Arc<BlockRegistry>> {
    let manifest: RegistryManifest = serde_json::from_reader(File::open(path)?)?;
    let mut registry = BlockRegistry::default();

    let unknown_pool = register_texture_pool(&mut registry, "unknown");
    add_texture_to_pool(&mut registry, unknown_pool, Path::new("unknown.png"));

    for (pool_name, paths) in manifest.textures {
        let pool = register_texture_pool(&mut registry, &pool_name);
        for path in paths {
            add_texture_to_pool(&mut registry, pool, Path::new(&path));
        }
    }

    for block in manifest.blocks {
        let id = registry.entries.len();
        registry.name_map.insert(block.name.clone(), BlockId(id));
        let entry = make_entry(&mut registry, block)?;
        registry.entries.push(entry);
    }

    Ok(Arc::new(registry))
}

impl BlockRegistry {
    pub fn get_id(&self, name: &str) -> BlockId {
        self.name_map[name]
    }

    pub fn texture_paths<'a>(&'a self) -> impl Iterator<Item = &'a Path> {
        self.texture_paths.iter().map(|s| &**s)
    }

    #[inline(always)]
    pub fn collision_type(&self, id: BlockId) -> CollisionType {
        self.entries[id.0].properties.collision_type
    }

    #[inline(always)]
    pub fn liquid(&self, id: BlockId) -> bool {
        self.entries[id.0].properties.liquid
    }

    #[inline(always)]
    pub fn wind_sway(&self, id: BlockId) -> bool {
        self.entries[id.0].properties.wind_sway
    }

    #[inline(always)]
    pub fn block_light(&self, id: BlockId) -> u16 {
        self.entries[id.0].properties.block_light
    }

    #[inline(always)]
    pub fn light_transmissible(&self, id: BlockId) -> bool {
        self.entries[id.0].properties.light_transmissible
    }

    #[inline(always)]
    pub fn mesh_type(&self, id: BlockId) -> BlockMeshType {
        self.entries[id.0].mesh_type
    }

    #[inline(always)]
    pub fn block_textures(&self, id: BlockId) -> Option<&Vec<Faces<TexturePoolId>>> {
        self.entries[id.0].textures.as_ref()
    }

    #[inline(always)]
    pub fn pool_textures(&self, id: TexturePoolId) -> &[TextureId] {
        &self.texture_pools[id.0]
    }
}
