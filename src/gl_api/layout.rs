use cgmath::{Vector2, Vector3, Vector4};
use gl::types::*;
use smallvec::SmallVec;

pub struct AttributeLayout {
    pub attrib_type: GLenum,
    pub attrib_size: GLint,
    pub attrib_offset: GLint,
}
pub type LayoutVec = SmallVec<[AttributeLayout; 8]>;

pub unsafe trait GlLayout {
    fn layout() -> LayoutVec;
}

pub unsafe trait SimpleLayout {
    fn size() -> GLint;
    fn ty() -> GLenum;
}

macro_rules! small_vec {
    ($($item:expr),*) => ({
        let mut vec = $crate::smallvec::SmallVec::new();
        $(vec.push($item);)*
        vec
    })
}

unsafe impl<T: SimpleLayout> GlLayout for T {
    fn layout() -> LayoutVec {
        small_vec![AttributeLayout {
            attrib_size: T::size(),
            attrib_type: T::ty(),
            attrib_offset: 0
        }]
    }
}

macro_rules! offset_of {
    ($father:ty, $($field:tt)+) => ({
        #[allow(unused_unsafe)]
        let root: $father = unsafe { $crate::std::mem::uninitialized() };

        let base = &root as *const _ as usize;

        // Future error: borrow of packed field requires unsafe function or block (error E0133)
        #[allow(unused_unsafe)]
        let member =  unsafe { &root.$($field)* as *const _ as usize };

        $crate::std::mem::forget(root);

        member - base
    });
}

macro_rules! vertex {
    (vertex $name:ident {
        $($attrib:ident: $attrib_type:ty,)*
    }) => {
        #[derive(Copy, Clone, Debug)]
        #[repr(C)]
        pub struct $name {
            $($attrib: $attrib_type),*
        }

        unsafe impl $crate::gl_api::layout::GlLayout for $name {
            fn layout() -> $crate::gl_api::layout::LayoutVec {
                small_vec![
                    $($crate::gl_api::layout::AttributeLayout {
                        attrib_type: <$attrib_type as $crate::gl_api::layout::SimpleLayout>::ty(),
                        attrib_size: <$attrib_type as $crate::gl_api::layout::SimpleLayout>::size(),
                        attrib_offset: offset_of!($name, $attrib) as i32,
                    }),*
                ]
            }
        }
    }
}

macro_rules! layout_simple {
    ($type:ty: $gl_type:ident $amount:expr) => {
        unsafe impl SimpleLayout for $type {
            fn size() -> GLint { $amount }
            fn ty() -> GLenum { ::gl::$gl_type }
        }
    }
}

layout_simple!(f32: FLOAT 1);
layout_simple!((f32,): FLOAT 1);
layout_simple!((f32, f32): FLOAT 2);
layout_simple!((f32, f32, f32): FLOAT 3);
layout_simple!((f32, f32, f32, f32): FLOAT 4);
layout_simple!([f32; 1]: FLOAT 1);
layout_simple!([f32; 2]: FLOAT 2);
layout_simple!([f32; 3]: FLOAT 3);
layout_simple!([f32; 4]: FLOAT 4);
layout_simple!(Vector2<f32>: FLOAT 2);
layout_simple!(Vector3<f32>: FLOAT 3);
layout_simple!(Vector4<f32>: FLOAT 4);

layout_simple!(f64: DOUBLE 1);
layout_simple!((f64,): DOUBLE 1);
layout_simple!((f64, f64): DOUBLE 2);
layout_simple!((f64, f64, f64): DOUBLE 3);
layout_simple!((f64, f64, f64, f64): DOUBLE 4);
layout_simple!([f64; 1]: DOUBLE 1);
layout_simple!([f64; 2]: DOUBLE 2);
layout_simple!([f64; 3]: DOUBLE 3);
layout_simple!([f64; 4]: DOUBLE 4);
layout_simple!(Vector2<f64>: DOUBLE 2);
layout_simple!(Vector3<f64>: DOUBLE 3);
layout_simple!(Vector4<f64>: DOUBLE 4);

layout_simple!(i32: INT 1);
layout_simple!((i32,): INT 1);
layout_simple!((i32, i32): INT 2);
layout_simple!((i32, i32, i32): INT 3);
layout_simple!((i32, i32, i32, i32): INT 4);
layout_simple!([i32; 1]: INT 1);
layout_simple!([i32; 2]: INT 2);
layout_simple!([i32; 3]: INT 3);
layout_simple!([i32; 4]: INT 4);
layout_simple!(Vector2<i32>: INT 2);
layout_simple!(Vector3<i32>: INT 3);
layout_simple!(Vector4<i32>: INT 4);

layout_simple!(u32: UNSIGNED_INT 1);
layout_simple!((u32,): UNSIGNED_INT 1);
layout_simple!((u32, u32): UNSIGNED_INT 2);
layout_simple!((u32, u32, u32): UNSIGNED_INT 3);
layout_simple!((u32, u32, u32, u32): UNSIGNED_INT 4);
layout_simple!([u32; 1]: UNSIGNED_INT 1);
layout_simple!([u32; 2]: UNSIGNED_INT 2);
layout_simple!([u32; 3]: UNSIGNED_INT 3);
layout_simple!([u32; 4]: UNSIGNED_INT 4);
layout_simple!(Vector2<u32>: UNSIGNED_INT 2);
layout_simple!(Vector3<u32>: UNSIGNED_INT 3);
layout_simple!(Vector4<u32>: UNSIGNED_INT 4);

layout_simple!(i16: SHORT 1);
layout_simple!((i16,): SHORT 1);
layout_simple!((i16, i16): SHORT 2);
layout_simple!((i16, i16, i16): SHORT 3);
layout_simple!((i16, i16, i16, i16): SHORT 4);
layout_simple!([i16; 1]: SHORT 1);
layout_simple!([i16; 2]: SHORT 2);
layout_simple!([i16; 3]: SHORT 3);
layout_simple!([i16; 4]: SHORT 4);
layout_simple!(Vector2<i16>: SHORT 2);
layout_simple!(Vector3<i16>: SHORT 3);
layout_simple!(Vector4<i16>: SHORT 4);

layout_simple!(u16: UNSIGNED_SHORT 1);
layout_simple!((u16,): UNSIGNED_SHORT 1);
layout_simple!((u16, u16): UNSIGNED_SHORT 2);
layout_simple!((u16, u16, u16): UNSIGNED_SHORT 3);
layout_simple!((u16, u16, u16, u16): UNSIGNED_SHORT 4);
layout_simple!([u16; 1]: UNSIGNED_SHORT 1);
layout_simple!([u16; 2]: UNSIGNED_SHORT 2);
layout_simple!([u16; 3]: UNSIGNED_SHORT 3);
layout_simple!([u16; 4]: UNSIGNED_SHORT 4);
layout_simple!(Vector2<u16>: UNSIGNED_SHORT 2);
layout_simple!(Vector3<u16>: UNSIGNED_SHORT 3);
layout_simple!(Vector4<u16>: UNSIGNED_SHORT 4);

layout_simple!(i8: BYTE 1);
layout_simple!((i8,): BYTE 1);
layout_simple!((i8, i8): BYTE 2);
layout_simple!((i8, i8, i8): BYTE 3);
layout_simple!((i8, i8, i8, i8): BYTE 4);
layout_simple!([i8; 1]: BYTE 1);
layout_simple!([i8; 2]: BYTE 2);
layout_simple!([i8; 3]: BYTE 3);
layout_simple!([i8; 4]: BYTE 4);
layout_simple!(Vector2<i8>: BYTE 2);
layout_simple!(Vector3<i8>: BYTE 3);
layout_simple!(Vector4<i8>: BYTE 4);

layout_simple!(u8: UNSIGNED_BYTE 1);
layout_simple!((u8,): UNSIGNED_BYTE 1);
layout_simple!((u8, u8): UNSIGNED_BYTE 2);
layout_simple!((u8, u8, u8): UNSIGNED_BYTE 3);
layout_simple!((u8, u8, u8, u8): UNSIGNED_BYTE 4);
layout_simple!([u8; 1]: UNSIGNED_BYTE 1);
layout_simple!([u8; 2]: UNSIGNED_BYTE 2);
layout_simple!([u8; 3]: UNSIGNED_BYTE 3);
layout_simple!([u8; 4]: UNSIGNED_BYTE 4);
layout_simple!(Vector2<u8>: UNSIGNED_BYTE 2);
layout_simple!(Vector3<u8>: UNSIGNED_BYTE 3);
layout_simple!(Vector4<u8>: UNSIGNED_BYTE 4);
