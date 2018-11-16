use gl;
use gl::types::*;
use gl_api::uniform::{Uniform, UniformLocation};
use image::ImageError;
use image::{self, DynamicImage, ImageBuffer, Pixel};
use std::cell::Cell;
use std::ops::Deref;
use std::path::Path;

pub type TextureResult<T> = Result<T, TextureError>;
#[derive(Debug)]
pub enum TextureError {
    Image(ImageError),
    TextureTooLarge(u32, u32),
}

impl From<ImageError> for TextureError {
    fn from(err: ImageError) -> Self {
        TextureError::Image(err)
    }
}

#[repr(u32)]
pub enum MinimizationFilter {
    Nearest = gl::NEAREST,
    Linear = gl::LINEAR,
    NearestMipmapNearest = gl::NEAREST_MIPMAP_NEAREST,
    LinearMipmapNearest = gl::LINEAR_MIPMAP_NEAREST,
    NearestMipmapLinear = gl::NEAREST_MIPMAP_LINEAR,
    LinearMipmapLinear = gl::LINEAR_MIPMAP_LINEAR,
}

#[repr(u32)]
pub enum MagnificationFilter {
    Nearest = gl::NEAREST,
    Linear = gl::LINEAR,
}

#[repr(u32)]
pub enum WrapMode {
    ClampToEdge = gl::CLAMP_TO_EDGE,
    ClampToBorder = gl::CLAMP_TO_BORDER,
    MirroredRepeat = gl::MIRRORED_REPEAT,
    Repeat = gl::REPEAT,
    MirrorClampToEdge = gl::MIRROR_CLAMP_TO_EDGE,
}

pub enum TextureAxis {
    S,
    T,
    R,
}

pub trait Texture {
    fn texture_wrap_behavior(&self, axis: TextureAxis, mode: WrapMode);
    fn min_filter(&self, mode: MinimizationFilter);
    fn mag_filter(&self, mode: MagnificationFilter);
}

pub struct Texture2D {
    id: GLuint,
    texture_slot: Cell<GLenum>,
}

impl Texture2D {
    pub fn new() -> Self {
        let mut id = 0;

        gl_call!(assert GenTextures(1, &mut id));

        Texture2D {
            id,
            texture_slot: Cell::new(0),
        }
    }

    pub fn source_from_image<P: AsRef<Path>>(&self, path: P) -> TextureResult<()> {
        self.bind();
        let image = image::open(path)?;

        #[allow(dead_code)]
        fn tex_image<P, C>(format: GLenum, buffer: &ImageBuffer<P, C>) -> TextureResult<()>
        where
            P: Pixel + 'static,
            P::Subpixel: 'static,
            C: Deref<Target = [P::Subpixel]>,
        {
            let (width, height) = buffer.dimensions();

            if width > gl::MAX_TEXTURE_SIZE || height > gl::MAX_TEXTURE_SIZE {
                return Err(TextureError::TextureTooLarge(width, height));
            }

            Ok(gl_call!(assert TexImage2D(
                gl::TEXTURE_2D,
                0,
                gl::RGBA8 as GLint,
                width as i32,
                height as i32,
                0,
                format,
                gl::UNSIGNED_BYTE,
                buffer.as_ptr() as *const _
            )))
        };

        match image {
            DynamicImage::ImageRgb8(image) => tex_image(gl::RGB, &image)?,
            DynamicImage::ImageRgba8(image) => tex_image(gl::RGBA, &image)?,
            _ => unimplemented!("luma images are not supported."),
        }

        Ok(())
    }

    fn generate_mipmap(&self) {
        self.bind();

        gl_call!(assert GenerateMipmap(gl::TEXTURE_2D));
    }

    pub fn bind(&self) {
        gl_call!(assert BindTexture(gl::TEXTURE_2D, self.id));
    }

    pub fn set_texture_bank(&self, slot: usize) {
        assert!(slot <= gl::MAX_COMBINED_TEXTURE_IMAGE_UNITS as usize);

        self.bind();
        self.texture_slot.set(slot as GLenum);
        gl_call!(assert ActiveTexture(gl::TEXTURE0 + slot as GLenum));
    }
}

impl Texture for Texture2D {
    fn texture_wrap_behavior(&self, axis: TextureAxis, mode: WrapMode) {
        self.bind();
        let axis = match axis {
            TextureAxis::S => gl::TEXTURE_WRAP_S,
            TextureAxis::T => gl::TEXTURE_WRAP_T,
            TextureAxis::R => gl::TEXTURE_WRAP_R,
        };

        gl_call!(assert TexParameteri(gl::TEXTURE_2D, axis, mode as i32));
    }

    fn min_filter(&self, mode: MinimizationFilter) {
        self.bind();

        // Generate mipmaps if the minimization filter uses mipmaps
        match mode {
            MinimizationFilter::LinearMipmapLinear
            | MinimizationFilter::LinearMipmapNearest
            | MinimizationFilter::NearestMipmapLinear
            | MinimizationFilter::NearestMipmapNearest => self.generate_mipmap(),
            _ => (),
        }

        gl_call!(TexParameteri(
            gl::TEXTURE_2D,
            gl::TEXTURE_MIN_FILTER,
            mode as i32
        ))
        .unwrap();
    }

    fn mag_filter(&self, mode: MagnificationFilter) {
        self.bind();

        gl_call!(assert TexParameteri(
            gl::TEXTURE_2D,
            gl::TEXTURE_MAG_FILTER,
            mode as i32
        ));
    }
}

impl Drop for Texture2D {
    fn drop(&mut self) {
        gl_call!(debug DeleteTextures(1, &self.id));
    }
}

impl Uniform for Texture2D {
    #[inline(always)]
    fn set_uniform(&self, location: UniformLocation) {
        gl_call!(assert Uniform1i(location, self.texture_slot.get() as i32));
    }
}
