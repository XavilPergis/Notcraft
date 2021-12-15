// use crate::util;
use anyhow::Result;
use glium::{
    backend::Facade, program::SourceCode, texture::TextureCreationError, Program,
    ProgramCreationError,
};
use image::{GenericImageView, ImageError, RgbaImage};
use std::{
    collections::{HashMap, HashSet},
    ffi::OsStr,
    io::ErrorKind,
    path::Path,
    sync::Arc,
};

macro_rules! err_from {
    ($sup:ident => $sub:path = $variant:ident) => {
        impl From<$sub> for $sup {
            fn from(sub: $sub) -> Self {
                $sup::$variant(sub)
            }
        }
    };
}

#[derive(Debug)]
pub enum TextureLoadError {
    Io(std::io::Error),
    Image(ImageError),
    Texture(TextureCreationError),
    MismatchedDimensions(HashSet<(u32, u32)>),
}

impl std::error::Error for TextureLoadError {}
impl std::fmt::Display for TextureLoadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "texture load error: ")?;
        match self {
            TextureLoadError::Io(err) => write!(f, "io: {}", err)?,
            TextureLoadError::Image(err) => write!(f, "image: {}", err)?,
            TextureLoadError::Texture(err) => write!(f, "texture: {}", err)?,
            TextureLoadError::MismatchedDimensions(dims) => {
                write!(f, "mismatched dimensions: ")?;
                for (x, y) in dims {
                    write!(f, "({}, {}), ", x, y)?;
                }
            }
        }
        Ok(())
    }
}

err_from! { TextureLoadError => ImageError = Image }
err_from! { TextureLoadError => std::io::Error = Io }
err_from! { TextureLoadError => TextureCreationError = Texture }

#[derive(Clone, Debug)]
pub struct BlockTextures {
    pub width: u32,
    pub height: u32,
    pub unknown_texture: Arc<RgbaImage>,
    pub block_textures: HashMap<String, Arc<RgbaImage>>,
}

struct BlockTextureLoadContext<'env> {
    base_path: &'env Path,
    found_dimensions: HashSet<(u32, u32)>,
}

impl<'env> BlockTextureLoadContext<'env> {
    fn new(base_path: &'env Path) -> Self {
        Self {
            base_path,
            found_dimensions: Default::default(),
        }
    }

    fn load(&mut self, path: &str) -> Result<Option<RgbaImage>, TextureLoadError> {
        let texture_path = self.base_path.join(format!("{}.png", path));
        log::debug!("loading block texture from {}", texture_path.display());
        let image = match image::open(&texture_path) {
            Ok(image) => image,
            Err(ImageError::IoError(err)) if err.kind() == ErrorKind::NotFound => {
                log::warn!(
                    "block texture '{}' was not found in {}!",
                    path,
                    texture_path.display()
                );
                return Ok(None);
            }
            Err(other) => return Err(other.into()),
        };
        self.found_dimensions.insert(image.dimensions());
        Ok(Some(image.to_rgba()))
    }

    fn dimensions(&self) -> Option<(u32, u32)> {
        if self.found_dimensions.len() == 1 {
            self.found_dimensions.iter().copied().next()
        } else {
            None
        }
    }
}

pub fn load_block_textures<'a, P, I>(
    base_path: P,
    names: I,
) -> Result<BlockTextures, TextureLoadError>
where
    P: AsRef<Path>,
    I: IntoIterator<Item = &'a str>,
{
    let mut ctx = BlockTextureLoadContext::new(base_path.as_ref());

    let names = names.into_iter();

    let unknown_texture = Arc::new(ctx.load("unknown")?.unwrap());

    let mut block_textures = HashMap::new();
    for entry in names {
        let texture = ctx
            .load(entry)?
            .map(Arc::new)
            .unwrap_or_else(|| Arc::clone(&unknown_texture));
        block_textures.insert(entry.to_owned(), texture);
    }

    match ctx.dimensions() {
        Some((width, height)) => Ok(BlockTextures {
            width,
            height,
            unknown_texture,
            block_textures,
        }),
        None => Err(TextureLoadError::MismatchedDimensions(ctx.found_dimensions)),
    }
}

#[derive(Debug)]
pub enum ShaderLoadError {
    Io(std::io::Error),
    Program(ProgramCreationError),
    MissingFragment,
    MissingVertex,
}

impl std::error::Error for ShaderLoadError {}

impl std::fmt::Display for ShaderLoadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "shader load error: ")?;
        match self {
            ShaderLoadError::Io(err) => write!(f, "{}", err),
            ShaderLoadError::Program(err) => write!(f, "{}", err),
            ShaderLoadError::MissingFragment => write!(f, "missing fragment stage"),
            ShaderLoadError::MissingVertex => write!(f, "missing vertex stage"),
        }
    }
}

err_from! { ShaderLoadError => std::io::Error = Io }
err_from! { ShaderLoadError => ProgramCreationError = Program }

pub fn load_shader<P: AsRef<Path>, F: Facade>(
    ctx: &F,
    path: P,
) -> Result<Program, ShaderLoadError> {
    let dir = path.as_ref().read_dir()?;

    let mut vertex = None;
    let mut fragment = None;
    let mut geometry = None;
    let mut tess_eval = None;
    let mut tess_control = None;

    for file in dir {
        let file = file?;
        if file.file_type()?.is_file() {
            let path = file.path();

            match path.extension().and_then(OsStr::to_str) {
                Some("vert") => vertex = Some(std::fs::read_to_string(&path)?),
                Some("tesc") => tess_control = Some(std::fs::read_to_string(&path)?),
                Some("tese") => tess_eval = Some(std::fs::read_to_string(&path)?),
                Some("geom") => geometry = Some(std::fs::read_to_string(&path)?),
                Some("frag") => fragment = Some(std::fs::read_to_string(&path)?),

                _ => (),
            }
        }
    }

    match (vertex, fragment) {
        (None, _) => Err(ShaderLoadError::MissingVertex),
        (_, None) => Err(ShaderLoadError::MissingFragment),
        (Some(vert), Some(frag)) => Ok(Program::new(ctx, SourceCode {
            vertex_shader: &vert,
            tessellation_control_shader: tess_control.as_ref().map(|src| &**src),
            tessellation_evaluation_shader: tess_eval.as_ref().map(|src| &**src),
            geometry_shader: geometry.as_ref().map(|src| &**src),
            fragment_shader: &frag,
        })?),
    }
}
