// use gl;

// use crate::program::uniform::{Uniform, UniformLocation};
// use crate::texture::{load_texture_defaults, RawTexture, TextureType};
// use crate::Context;

// use std::ops::Deref;

// // /target must be one of GL_TEXTURE_1D, GL_TEXTURE_2D, GL_TEXTURE_3D,
// // GL_TEXTURE_1D_ARRAY, GL_TEXTURE_2D_ARRAY, GL_TEXTURE_RECTANGLE,
// // GL_TEXTURE_CUBE_MAP, GL_TEXTURE_CUBE_MAP_ARRAY, GL_TEXTURE_BUFFER,
// // GL_TEXTURE_2D_MULTISAMPLE or GL_TEXTURE_2D_MULTISAMPLE_ARRAY.

// pub struct TextureArray2d {
//     pub(crate) raw: RawTexture,
// }

// // use image::{ColorType, ImageBuffer, Pixel};

// // format
// // Specifies the format of the pixel data. The following symbolic values are
// // accepted: GL_RED, GL_RG, GL_RGB, GL_BGR, GL_RGBA, GL_DEPTH_COMPONENT, and
// // GL_STENCIL_INDEX.

// // type
// // Specifies the data type of the pixel data. The following symbolic values are
// // accepted: GL_UNSIGNED_BYTE, GL_BYTE, GL_UNSIGNED_SHORT, GL_SHORT,
// // GL_UNSIGNED_INT, GL_INT, GL_FLOAT, GL_UNSIGNED_BYTE_3_3_2,
// // GL_UNSIGNED_BYTE_2_3_3_REV, GL_UNSIGNED_SHORT_5_6_5,
// // GL_UNSIGNED_SHORT_5_6_5_REV, GL_UNSIGNED_SHORT_4_4_4_4,
// // GL_UNSIGNED_SHORT_4_4_4_4_REV, GL_UNSIGNED_SHORT_5_5_5_1,
// // GL_UNSIGNED_SHORT_1_5_5_5_REV, GL_UNSIGNED_INT_8_8_8_8,
// // GL_UNSIGNED_INT_8_8_8_8_REV, GL_UNSIGNED_INT_10_10_10_2, and
// // GL_UNSIGNED_INT_2_10_10_10_REV.

// fn gl_format<P: Pixel>() -> Option<(u32, u32)> {
//     let color_type = P::color_type();

//     let format = match color_type {
//         ColorType::Gray(_) => gl::RED,
//         ColorType::RGB(_) => gl::RGB,
//         ColorType::RGBA(_) => gl::RGBA,
//         _ => return None,
//     };

//     let depth = match color_type {
//         ColorType::Gray(d) | ColorType::RGB(d) | ColorType::RGBA(d) => d,
//         _ => return None,
//     };

//     let ty = match depth {
//         8 => gl::UNSIGNED_BYTE,
//         16 => gl::UNSIGNED_SHORT,
//         32 => gl::UNSIGNED_INT,
//         _ => return None,
//     };

//     Some((format, ty))
// }

// fn sub_image_slice<P, C>(
//     raw: &RawTexture,
//     image: &ImageBuffer<P, C>,
//     layer: usize,
//     format: u32,
//     ty: u32,
// ) where
//     P: Pixel + 'static,
//     P::Subpixel: 'static,
//     C: Deref<Target = [P::Subpixel]>,
// {
//     gl_call!(assert TextureSubImage3D(
//         raw.id, // Texture object
//         0, // mipmap level
//         0, // X offset
//         0, // Y offset
//         layer as i32, // Z offset
//         image.width() as i32, // width
//         image.height() as i32, // height
//         1, // depth
//         format,
//         ty,
//         image.as_ptr() as *const _
//     ));
// }

// impl TextureArray2d {
//     pub fn new(ctx: &Context, width: usize, height: usize, layers: usize) -> Self {
//         let raw = RawTexture::new(ctx, TextureType::Texture2DArray);

//         gl_call!(assert TextureStorage3D(raw.id, 1, gl::RGBA8, width as i32, height as i32, layers as i32));
//         load_texture_defaults(&raw);

//         TextureArray2d { raw }
//     }

//     pub fn upload_textures<P, C, I>(&self, _ctx: &Context, iter: I)
//     where
//         I: IntoIterator<Item = ImageBuffer<P, C>>,
//         P: Pixel + 'static,
//         P::Subpixel: 'static,
//         C: Deref<Target = [P::Subpixel]>,
//     {
//         for (layer, image) in iter.into_iter().enumerate() {
//             debug!(
//                 "copying image into layer {} for 2d texture array #{}",
//                 layer, self.raw.id
//             );

//             if let Some((format, ty)) = gl_format::<P>() {
//                 sub_image_slice(&self.raw, &image, layer, format, ty);
//             } else {
//                 // TODO: convert to rgb or whatever and upload that
//                 unimplemented!()
//             }
//         }
//     }
// }

// impl Uniform for TextureArray2d {
//     #[inline(always)]
//     fn set_uniform(&self, ctx: &Context, location: UniformLocation) {
//         gl_call!(assert BindTexture(gl::TEXTURE_2D_ARRAY, self.raw.id));
//         gl_call!(assert ActiveTexture(gl::TEXTURE0));
//         gl_call!(assert Uniform1i(location, 0));
//     }
// }
