use image::ImageError;
use std::cell::Cell;
use std::ops::Deref;
use std::path::Path;
use image::{self, ImageBuffer, DynamicImage, Pixel};
use gl::types::*;
use gl;
use gl_api::uniform::{Uniform, UniformLocation};

pub type TextureResult<T> = Result<T, TextureError>;
#[derive(Debug)]
pub enum TextureError {
    Image(ImageError),
    TextureTooLarge(u32, u32),
}

impl From<ImageError> for TextureError {
    fn from(err: ImageError) -> Self { TextureError::Image(err) }
}

pub struct Texture {
    id: GLuint,
    texture_slot: Cell<GLenum>,
}

impl Texture {
    pub fn new<P: AsRef<Path>>(path: P) -> TextureResult<Self> {
        unsafe {
            let image = image::open(path)?;

            let mut id = 0;
            gl_call!(GenTextures(1, &mut id)).unwrap();

            gl::ActiveTexture(gl::TEXTURE0);
            gl::BindTexture(gl::TEXTURE_2D, id);
            gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_MIN_FILTER, gl::NEAREST as GLint);
            gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_MAG_FILTER, gl::NEAREST as GLint);
            gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_WRAP_S, gl::CLAMP_TO_EDGE as GLint);
            gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_WRAP_T, gl::CLAMP_TO_EDGE as GLint);

            #[allow(dead_code)]
            fn tex_image<P, C>(format: GLenum, buffer: &ImageBuffer<P, C>) -> TextureResult<()>
            where P: Pixel + 'static,
                  P::Subpixel: 'static,
                  C: Deref<Target=[P::Subpixel]> {
                let (width, height) = buffer.dimensions();

                if width > gl::MAX_TEXTURE_SIZE || height > gl::MAX_TEXTURE_SIZE {
                    return Err(TextureError::TextureTooLarge(width, height));
                }
                unsafe {
                    Ok(gl_call!(TexImage2D(gl::TEXTURE_2D, 0, gl::RGBA8 as GLint,
                                   width as i32, height as i32, 0, format,
                                   gl::UNSIGNED_BYTE, buffer.as_ptr() as *const _)).unwrap())
                }
            };

            match image {
                DynamicImage::ImageRgb8(image) => tex_image(gl::RGB, &image)?,
                DynamicImage::ImageRgba8(image) => tex_image(gl::RGBA, &image)?,
                _ => unimplemented!("luma images are not supported."),
            }

            Ok(Texture { id, texture_slot: Cell::new(0) })
        }
    }

    pub fn bind(&self) {
        unsafe {
            gl_call!(BindTexture(gl::TEXTURE_2D, self.id)).unwrap();
        }
    }

    pub fn set_texture_bank(&self, slot: usize) {
        assert!(slot <= gl::MAX_COMBINED_TEXTURE_IMAGE_UNITS as usize);
        unsafe {
            self.bind();
            self.texture_slot.set(slot as GLenum);
            gl_call!(ActiveTexture(gl::TEXTURE0 + slot as GLenum)).unwrap();
        }
    }
}

impl Drop for Texture {
    fn drop(&mut self) {
        unsafe {
            gl_call!(DeleteTextures(1, &self.id)).unwrap();
        }
    }
}

impl Uniform for Texture {
    #[inline(always)]
    fn set_uniform(&self, location: UniformLocation) {
        unsafe { gl_call!(Uniform1i(location, self.texture_slot.get() as i32)).unwrap(); }
    }
}
