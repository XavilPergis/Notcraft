use crate::engine::{prelude::*, Side};
use std::{collections::HashMap, fs::File, path::Path, sync::Arc};

pub const AIR: BlockId = BlockId(0);

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct Faces<T> {
    pub top: T,
    pub bottom: T,
    pub right: T,
    pub left: T,
    pub front: T,
    pub back: T,
}

impl<T> Faces<T> {
    fn map<U, F>(self, mut func: F) -> Faces<U>
    where
        F: FnMut(T) -> U,
    {
        Faces {
            top: func(self.top),
            bottom: func(self.bottom),
            left: func(self.left),
            right: func(self.right),
            front: func(self.front),
            back: func(self.back),
        }
    }
}

impl<T> std::ops::Index<Side> for Faces<T> {
    type Output = T;

    fn index(&self, index: Side) -> &Self::Output {
        match index {
            Side::Top => &self.top,
            Side::Bottom => &self.bottom,
            Side::Right => &self.right,
            Side::Left => &self.left,
            Side::Front => &self.front,
            Side::Back => &self.back,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Default, Deserialize)]
#[serde(rename_all = "kebab-case")]
#[serde(default)]
pub struct BlockTextures {
    default: Option<String>,
    #[serde(flatten)]
    faces: Faces<Option<String>>,
}

#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct BlockProperties {
    collidable: bool,
    #[serde(default)]
    liquid: bool,
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
    #[serde(default)]
    textures: Option<Vec<BlockTextures>>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct BlockRegistryEntry {
    properties: BlockProperties,
    mesh_type: BlockMeshType,
    textures: Option<Vec<Faces<usize>>>,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash, Default)]
pub struct BlockId(pub(crate) usize);

fn register_texture(reg: &mut BlockRegistry, path: String) -> usize {
    if let Some(&idx) = reg.texture_indices.get(&path) {
        return idx;
    }

    let id = reg.texture_paths.len();
    reg.texture_indices.insert(path.clone(), id);
    reg.texture_paths.push(path);

    id
}

fn make_entry(reg: &mut BlockRegistry, desc: BlockDescription) -> Result<BlockRegistryEntry> {
    let textures = match desc.textures {
        Some(choices) => {
            let mut res = Vec::with_capacity(choices.len());
            for choice in choices {
                let default = choice
                    .default
                    .map(|path| register_texture(reg, path))
                    .unwrap_or_else(|| reg.texture_indices["unknown"]);
                res.push(choice.faces.map(|path| {
                    path.map(|path| register_texture(reg, path))
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

    texture_paths: Vec<String>,
    texture_indices: HashMap<String, usize>,
}

pub fn load_registry<P: AsRef<Path>>(path: P) -> Result<Arc<BlockRegistry>> {
    let blocks: Vec<BlockDescription> = serde_json::from_reader(File::open(path)?)?;

    let mut registry = BlockRegistry::default();
    register_texture(&mut registry, String::from("unknown.png"));

    for block in blocks {
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

    pub fn texture_paths<'a>(&'a self) -> impl Iterator<Item = &'a str> {
        self.texture_paths.iter().map(|s| &**s)
    }

    #[inline(always)]
    pub fn collidable(&self, id: BlockId) -> bool {
        self.entries[id.0].properties.collidable
    }

    #[inline(always)]
    pub fn liquid(&self, id: BlockId) -> bool {
        self.entries[id.0].properties.liquid
    }

    #[inline(always)]
    pub fn mesh_type(&self, id: BlockId) -> BlockMeshType {
        self.entries[id.0].mesh_type
    }

    #[inline(always)]
    pub fn block_textures(&self, id: BlockId) -> Option<&Vec<Faces<usize>>> {
        self.entries[id.0].textures.as_ref()
    }
}
