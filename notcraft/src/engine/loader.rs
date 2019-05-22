// use crate::util;
use glium::{
    backend::Facade, program::SourceCode, texture::TextureCreationError, Program,
    ProgramCreationError,
};
use image::{GenericImageView, ImageError, RgbImage, RgbaImage};
use std::{collections::HashSet, ffi::OsStr, path::Path};

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

err_from! { TextureLoadError => ImageError = Image }
err_from! { TextureLoadError => std::io::Error = Io }
err_from! { TextureLoadError => TextureCreationError = Texture }

#[derive(Clone, Debug)]
pub struct BlockTextureMaps {
    pub albedo: RgbaImage,
    pub normal: Option<RgbImage>,
    pub extra: Option<RgbImage>,
}

pub fn load_block_textures<'a, P, I>(
    base_path: P,
    names: I,
) -> Result<(u32, u32, Vec<BlockTextureMaps>), TextureLoadError>
where
    P: AsRef<Path>,
    I: IntoIterator<Item = &'a str>,
    I::IntoIter: ExactSizeIterator,
{
    let base_path = base_path.as_ref();
    let names = names.into_iter();
    let mut textures = Vec::with_capacity(names.len());

    let mut dims = HashSet::new();

    for entry in names {
        let albedo_path = base_path.join(format!("{}.png", entry));
        let normal_path = base_path.join(format!("{}_n.png", entry));
        let extra_path = base_path.join(format!("{}_s.png", entry));

        log::debug!(
            "Loading `{name}` - `{base_path}`",
            name = entry,
            base_path = albedo_path.display()
        );

        let albedo = image::open(albedo_path)?;
        dims.insert(albedo.dimensions());

        let normal = if normal_path.exists() {
            let normal = image::open(normal_path)?;
            dims.insert(normal.dimensions());
            Some(normal)
        } else {
            None
        };
        let extra = if extra_path.exists() {
            let extra = image::open(extra_path)?;
            dims.insert(extra.dimensions());
            Some(extra)
        } else {
            None
        };

        textures.push(BlockTextureMaps {
            albedo: albedo.to_rgba(),
            normal: normal.map(|img| img.to_rgb()),
            extra: extra.map(|img| img.to_rgb()),
        });
    }

    log::debug!("Texture sizes: {:?}", dims);

    if dims.len() > 1 {
        Err(TextureLoadError::MismatchedDimensions(dims))
    } else {
        let (w, h) = dims.iter().cloned().next().unwrap_or_default();
        Ok((w, h, textures))
    }
}

#[derive(Debug)]
pub enum ShaderLoadError {
    Io(std::io::Error),
    Program(ProgramCreationError),
    MissingFragment,
    MissingVertex,
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
