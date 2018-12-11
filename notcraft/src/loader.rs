use crate::util;
use glium::{
    backend::Facade,
    program::SourceCode,
    texture::{RawImage2d, TextureCreationError},
    Program, ProgramCreationError, Texture2d,
};
use image::{ImageError, RgbaImage};
use std::{
    collections::HashMap,
    ffi::OsStr,
    path::{Path, PathBuf},
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
}

err_from! { TextureLoadError => ImageError = Image }
err_from! { TextureLoadError => std::io::Error = Io }
err_from! { TextureLoadError => TextureCreationError = Texture }

pub fn load_textures<P: AsRef<Path>, F: Facade>(
    ctx: &F,
    path: P,
) -> Result<HashMap<String, RgbaImage>, TextureLoadError> {
    let dir = path.as_ref().read_dir()?;
    let mut images = HashMap::new();

    for file in dir {
        let file = file?;

        if file.file_type()?.is_file() {
            let path = file.path();
            let image = image::open(&path)?.to_rgba();
            if let Some(stem) = path.file_stem() {
                let name = stem.to_string_lossy().into();
                log::debug!("Loaded `{}` as `{}`", path.display(), name);
                images.insert(name, image);
            }
        }
    }

    Ok(images)
}

// pub fn create_2d_texture<F: Facade>(
//     ctx: &F,
//     image: &RgbaImage,
// ) -> Result<Texture2d, TextureLoadError> {
//     let raw = RawImage2d::from_raw_rgba_reversed(&image.into_raw(),
// image.dimensions());     Ok(Texture2d::new(ctx, raw)?)
// }

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
                Some("vert") => vertex = Some(util::read_file(&path)?),
                Some("tesc") => tess_control = Some(util::read_file(&path)?),
                Some("tese") => tess_eval = Some(util::read_file(&path)?),
                Some("geom") => geometry = Some(util::read_file(&path)?),
                Some("frag") => fragment = Some(util::read_file(&path)?),

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
