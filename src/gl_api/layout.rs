use gl;

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
#[repr(i32)]
pub enum Dim {
    One = 1,
    Two = 2,
    Three = 3,
    Four = 4,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
#[repr(u32)]
pub enum Type {
    F32 = gl::FLOAT,
    F64 = gl::DOUBLE,
    I32 = gl::INT,
    U32 = gl::UNSIGNED_INT,
    I16 = gl::SHORT,
    U16 = gl::UNSIGNED_SHORT,
    I8 = gl::BYTE,
    U8 = gl::UNSIGNED_BYTE,
}

impl Type {
    pub fn size(&self) -> usize {
        match *self {
            Type::F64 => 8,
            Type::F32 | Type::U32 | Type::I32 => 4,
            Type::U16 | Type::I16 => 2,
            Type::U8 | Type::I8 => 1,
        }
    }
}

impl Type {
    pub fn is_integer(&self) -> bool {
        match *self {
            Type::F32 | Type::F64 => false,
            _ => true,
        }
    }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub struct AttributeFormat {
    pub(crate) ty: Type,
    pub(crate) dim: Dim,
}

impl AttributeFormat {
    pub fn size(&self) -> usize {
        self.dim as usize * self.ty.size()
    }
}

pub unsafe trait Layout: 'static {
    fn layout() -> Vec<AttributeFormat>;
}

macro_rules! vertex {
    (vertex $name:ident {
        $($attrib:ident: $attrib_type:ty,)*
    }) => {
        #[derive(Copy, Clone, Debug)]
        #[repr(C)]
        pub struct $name {
            $(pub $attrib: $attrib_type),*
        }

        unsafe impl $crate::gl_api::layout::Layout for $name {
            fn layout() -> Vec<$crate::gl_api::layout::AttributeFormat> {
                let mut attribs = vec![];
                $(attribs.extend(&<$attrib_type as $crate::gl_api::layout::Layout>::layout());)*
                attribs
            }
        }
    }
}

macro_rules! impl_layout {
    ($type:ty: $gl_type:ident $amount:ident) => {
        unsafe impl Layout for $type {
            fn layout() -> Vec<AttributeFormat> {
                vec![AttributeFormat {
                    ty: Type::$gl_type,
                    dim: Dim::$amount,
                }]
            }
        }
    };
}

use cgmath::*;
macro_rules! impl_layout_type {
    ($ty:ty => $gl:ident) => {
        impl_layout!($ty: $gl One);
        impl_layout!(($ty,): $gl One);
        impl_layout!(($ty, $ty): $gl Two);
        impl_layout!(($ty, $ty, $ty): $gl Three);
        impl_layout!(($ty, $ty, $ty, $ty): $gl Four);
        impl_layout!([$ty; 1]: $gl One);
        impl_layout!([$ty; 2]: $gl Two);
        impl_layout!([$ty; 3]: $gl Three);
        impl_layout!([$ty; 4]: $gl Four);
        impl_layout!(Vector1<$ty>: $gl One);
        impl_layout!(Vector2<$ty>: $gl Two);
        impl_layout!(Vector3<$ty>: $gl Three);
        impl_layout!(Vector4<$ty>: $gl Four);
        impl_layout!(Point1<$ty>: $gl One);
        impl_layout!(Point2<$ty>: $gl Two);
        impl_layout!(Point3<$ty>: $gl Three);
    };
}

impl_layout_type!(f32 => F32);
impl_layout_type!(f64 => F64);
impl_layout_type!(i32 => I32);
impl_layout_type!(u32 => U32);
impl_layout_type!(i16 => I16);
impl_layout_type!(u16 => U16);
impl_layout_type!(i8  => I8 );
impl_layout_type!(u8  => U8 );
