use gl::{self, types::*};
use gl_api::{
    context::{Context, TEXTURE_DROP_LIST},
    shader::uniform::{Uniform, UniformLocation},
};
use image::{ColorType, ImageBuffer, Pixel};
use std::ops::Deref;

pub struct RawTexture {
    crate id: u32,
    crate texture_type: TextureType,
}

impl RawTexture {
    pub fn new(_ctx: &Context, texture_type: TextureType) -> Self {
        let mut id = 0;
        gl_call!(assert CreateTextures(texture_type as u32, 1, &mut id));

        RawTexture { id, texture_type }
    }
}

impl Drop for RawTexture {
    fn drop(&mut self) {
        TEXTURE_DROP_LIST.lock().unwrap().push(self.id);
    }
}

#[repr(u32)]
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub enum TextureType {
    Texture1D = gl::TEXTURE_1D,
    Texture2D = gl::TEXTURE_2D,
    Texture3D = gl::TEXTURE_3D,
    Texture1DArray = gl::TEXTURE_1D_ARRAY,
    Texture2DArray = gl::TEXTURE_2D_ARRAY,
    TextureRectangle = gl::TEXTURE_RECTANGLE,
    TextureCubeMap = gl::TEXTURE_CUBE_MAP,
    TextureCubeMapArray = gl::TEXTURE_CUBE_MAP_ARRAY,
    TextureBuffer = gl::TEXTURE_BUFFER,
    Texture2DMultisample = gl::TEXTURE_2D_MULTISAMPLE,
    Texture2DMultisampleArray = gl::TEXTURE_2D_MULTISAMPLE_ARRAY,
}

/// Texture filter used when sampling a scaled-down texture
#[repr(u32)]
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub enum MinFilter {
    Nearest = gl::NEAREST,
    Linear = gl::LINEAR,
    NearestMipmapNearest = gl::NEAREST_MIPMAP_NEAREST,
    LinearMipmapNearest = gl::LINEAR_MIPMAP_NEAREST,
    NearestMipmapLinear = gl::NEAREST_MIPMAP_LINEAR,
    LinearMipmapLinear = gl::LINEAR_MIPMAP_LINEAR,
}

/// Texture filter used when sampling a scaled-up texture
#[repr(u32)]
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub enum MagFilter {
    Nearest = gl::NEAREST,
    Linear = gl::LINEAR,
}

/// What to do with tex coords outside the range [(0, 0), (1, 1)]
#[repr(u32)]
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub enum WrapMode {
    ClampToEdge = gl::CLAMP_TO_EDGE,
    ClampToBorder = gl::CLAMP_TO_BORDER,
    MirroredRepeat = gl::MIRRORED_REPEAT,
    Repeat = gl::REPEAT,
    MirrorClampToEdge = gl::MIRROR_CLAMP_TO_EDGE,
}

crate fn gl_format<P: Pixel>() -> Option<(u32, u32)> {
    let color_type = P::color_type();

    let format = match color_type {
        ColorType::Gray(_) => gl::RED,
        ColorType::RGB(_) => gl::RGB,
        ColorType::RGBA(_) => gl::RGBA,
        _ => return None,
    };

    let depth = match color_type {
        ColorType::Gray(d) | ColorType::RGB(d) | ColorType::RGBA(d) => d,
        _ => return None,
    };

    let ty = match depth {
        8 => gl::UNSIGNED_BYTE,
        16 => gl::UNSIGNED_SHORT,
        32 => gl::UNSIGNED_INT,
        _ => return None,
    };

    Some((format, ty))
}

crate fn load_texture_defaults(tex: &RawTexture) {
    gl_call!(assert TextureParameteri(tex.id, gl::TEXTURE_MIN_FILTER, gl::LINEAR as i32));
    gl_call!(assert TextureParameteri(tex.id, gl::TEXTURE_MAG_FILTER, gl::NEAREST as i32));
    gl_call!(assert TextureParameteri(tex.id, gl::TEXTURE_WRAP_S, gl::REPEAT as i32));
    gl_call!(assert TextureParameteri(tex.id, gl::TEXTURE_WRAP_T, gl::REPEAT as i32));
    gl_call!(assert TextureParameteri(tex.id, gl::TEXTURE_WRAP_R, gl::REPEAT as i32));
}

pub struct Texture2d {
    raw: RawTexture,
}

impl Texture2d {
    pub fn from_image<P, C>(ctx: &Context, image: &ImageBuffer<P, C>) -> Self
    where
        P: Pixel + 'static,
        P::Subpixel: 'static,
        C: Deref<Target = [P::Subpixel]>,
    {
        let img = Self::with_dimensions(ctx, image.width() as usize, image.height() as usize);
        img.upload_texture(ctx, image);
        img
    }

    pub fn with_dimensions(ctx: &Context, width: usize, height: usize) -> Self {
        let raw = RawTexture::new(ctx, TextureType::Texture2D);

        gl_call!(assert TextureStorage2D(raw.id, 1, gl::RGBA8, width as i32, height as i32));
        load_texture_defaults(&raw);

        Texture2d { raw }
    }

    pub fn upload_texture<P, C>(&self, ctx: &Context, image: &ImageBuffer<P, C>)
    where
        P: Pixel + 'static,
        P::Subpixel: 'static,
        C: Deref<Target = [P::Subpixel]>,
    {
        if let Some((format, ty)) = gl_format::<P>() {
            gl_call!(assert TextureSubImage2D(
                self.raw.id,
                0,
                0,
                0,
                image.width() as i32,
                image.height() as i32,
                format,
                ty,
                image.as_ptr() as *const _
            ));
        } else {
            unimplemented!();
        }
    }
}

impl Uniform for Texture2d {
    #[inline(always)]
    fn set_uniform(&self, ctx: &Context, location: UniformLocation) {
        gl_call!(assert BindTexture(gl::TEXTURE_2D, self.raw.id));
        gl_call!(assert ActiveTexture(gl::TEXTURE0));
        gl_call!(assert Uniform1i(location, 0));
    }
}
