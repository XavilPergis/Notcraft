use cgmath::*;
use crate::{
    buffer::Buffer,
    vertex_array::{Binder, VertexArray},
    Cons, Nil,
};
use gl;
use std::mem;

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

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub struct AttributeFormat {
    pub(crate) ty: Type,
    pub(crate) dim: Dim,
    pub(crate) normalized: bool,
    pub(crate) is_integer: bool,

    // If this data does not come from an interleaved buffer, then stride is the size of the type
    // and offset is 0
    pub(crate) stride: usize,
    pub(crate) offset: usize,
}

impl AttributeFormat {
    fn resize(self, size: usize, offset: usize) -> Self {
        AttributeFormat {
            stride: size,
            offset: self.offset + offset,
            ..self
        }
    }
}

pub unsafe trait InterleavedAttribute: Copy + Sized + 'static {
    fn attribute_format<F>(func: F)
    where
        F: FnMut(AttributeFormat);
}

macro_rules! attr_primitive {
    ($type:ty, $ty:expr, $dim:expr, $norm:expr, $int:expr) => {
        unsafe impl InterleavedAttribute for $type {
            fn attribute_format<F>(mut func: F)
            where
                F: FnMut(AttributeFormat),
            {
                func(AttributeFormat {
                    ty: $ty,
                    dim: $dim,
                    normalized: $norm,
                    is_integer: $int,

                    stride: mem::size_of::<Self>(),
                    offset: 0,
                });
            }
        }
    };

    (group $type:ty, $ty:expr, $norm:expr, $int:expr) => {
        attr_primitive!($type, $ty, Dim::One, $norm, $int);

        attr_primitive!([$type; 1], $ty, Dim::One, $norm, $int);
        attr_primitive!([$type; 2], $ty, Dim::Two, $norm, $int);
        attr_primitive!([$type; 3], $ty, Dim::Three, $norm, $int);
        attr_primitive!([$type; 4], $ty, Dim::Four, $norm, $int);

        attr_primitive!(Vector1<$type>, $ty, Dim::One, $norm, $int);
        attr_primitive!(Vector2<$type>, $ty, Dim::Two, $norm, $int);
        attr_primitive!(Vector3<$type>, $ty, Dim::Three, $norm, $int);
        attr_primitive!(Vector4<$type>, $ty, Dim::Four, $norm, $int);

        attr_primitive!(Point1<$type>, $ty, Dim::One, $norm, $int);
        attr_primitive!(Point2<$type>, $ty, Dim::Two, $norm, $int);
        attr_primitive!(Point3<$type>, $ty, Dim::Three, $norm, $int);
    };
}

/// Special wrapper meant for integer data. Integer data of this type will be
/// converted to a float in the shader and normalized, so that unsigned types
/// get mapped to `[0, 1]` and signed types get mapped to `[-1, 1]`.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Default)]
pub struct Normalized<T>(T);

/// Special wrapper meant for integer data. Integer data of this type will be
/// converted directly to a float in the shader.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Default)]
pub struct AsFloat<T>(T);

attr_primitive!(group f32, Type::F32, false, false);
attr_primitive!(group f64, Type::F64, false, false);

attr_primitive!(group i32, Type::I32, false, true);
attr_primitive!(group i16, Type::I16, false, true);
attr_primitive!(group i8, Type::I8, false, true);
attr_primitive!(group Normalized<i32>, Type::I32, true, false);
attr_primitive!(group Normalized<i16>, Type::I16, true, false);
attr_primitive!(group Normalized<i8>, Type::I8, true, false);
attr_primitive!(group AsFloat<i32>, Type::I32, false, false);
attr_primitive!(group AsFloat<i16>, Type::I16, false, false);
attr_primitive!(group AsFloat<i8>, Type::I8, false, false);

attr_primitive!(group u32, Type::U32, false, true);
attr_primitive!(group u16, Type::U16, false, true);
attr_primitive!(group u8, Type::U8, false, true);
attr_primitive!(group Normalized<u32>, Type::U32, true, false);
attr_primitive!(group Normalized<u16>, Type::U16, true, false);
attr_primitive!(group Normalized<u8>, Type::U8, true, false);
attr_primitive!(group AsFloat<u32>, Type::U32, false, false);
attr_primitive!(group AsFloat<u16>, Type::U16, false, false);
attr_primitive!(group AsFloat<u8>, Type::U8, false, false);

macro_rules! offset_of {
    ($father:ty, $($field:tt)+) => ({
        #[allow(unused_unsafe)]
        let root: $father = unsafe { mem::uninitialized() };

        let base = &root as *const _ as usize;
        let member = &root.$($field)* as *const _ as usize;
        mem::forget(root);

        member - base
    });
}

macro_rules! attr_tuple {
    ($($ty:ident: $field:tt),*) => {
        unsafe impl<$($ty),*> InterleavedAttribute for ($($ty),*) where $($ty: InterleavedAttribute),* {
            fn attribute_format<Func>(mut func: Func)
            where
                Func: FnMut(AttributeFormat),
            {
                let stride = mem::size_of::<Self>();

                $(
                    let offset = offset_of!(Self, $field);
                    $ty::attribute_format(|attr| func(attr.resize(stride, offset)));
                )*
            }
        }
    };
}

attr_tuple![A:0, B:1];
attr_tuple![A:0, B:1, C:2];
attr_tuple![A:0, B:1, C:2, D:3];
attr_tuple![A:0, B:1, C:2, D:3, E:4];
attr_tuple![A:0, B:1, C:2, D:3, E:4, F:5];
attr_tuple![A:0, B:1, C:2, D:3, E:4, F:5, G:6];
attr_tuple![A:0, B:1, C:2, D:3, E:4, F:5, G:6, H:7];
attr_tuple![A:0, B:1, C:2, D:3, E:4, F:5, G:6, H:7, I:8];
attr_tuple![A:0, B:1, C:2, D:3, E:4, F:5, G:6, H:7, I:8, J:9];
attr_tuple![A:0, B:1, C:2, D:3, E:4, F:5, G:6, H:7, I:8, J:9, K:10];
attr_tuple![A:0, B:1, C:2, D:3, E:4, F:5, G:6, H:7, I:8, J:9, K:10, L:11];
attr_tuple![A:0, B:1, C:2, D:3, E:4, F:5, G:6, H:7, I:8, J:9, K:10, L:11, M:12];
attr_tuple![A:0, B:1, C:2, D:3, E:4, F:5, G:6, H:7, I:8, J:9, K:10, L:11, M:12, N:13];
attr_tuple![A:0, B:1, C:2, D:3, E:4, F:5, G:6, H:7, I:8, J:9, K:10, L:11, M:12, N:13, O:14];
attr_tuple![A:0, B:1, C:2, D:3, E:4, F:5, G:6, H:7, I:8, J:9, K:10, L:11, M:12, N:13, O:14, P:15];
attr_tuple![A:0, B:1, C:2, D:3, E:4, F:5, G:6, H:7, I:8, J:9, K:10, L:11, M:12, N:13, O:14, P:15, Q:16];
attr_tuple![A:0, B:1, C:2, D:3, E:4, F:5, G:6, H:7, I:8, J:9, K:10, L:11, M:12, N:13, O:14, P:15, Q:16, R:17];
attr_tuple![A:0, B:1, C:2, D:3, E:4, F:5, G:6, H:7, I:8, J:9, K:10, L:11, M:12, N:13, O:14, P:15, Q:16, R:17, S:18];
attr_tuple![A:0, B:1, C:2, D:3, E:4, F:5, G:6, H:7, I:8, J:9, K:10, L:11, M:12, N:13, O:14, P:15, Q:16, R:17, S:18, T:19];
attr_tuple![A:0, B:1, C:2, D:3, E:4, F:5, G:6, H:7, I:8, J:9, K:10, L:11, M:12, N:13, O:14, P:15, Q:16, R:17, S:18, T:19, U:20];
attr_tuple![A:0, B:1, C:2, D:3, E:4, F:5, G:6, H:7, I:8, J:9, K:10, L:11, M:12, N:13, O:14, P:15, Q:16, R:17, S:18, T:19, U:20, V:21];
attr_tuple![A:0, B:1, C:2, D:3, E:4, F:5, G:6, H:7, I:8, J:9, K:10, L:11, M:12, N:13, O:14, P:15, Q:16, R:17, S:18, T:19, U:20, V:21, W:22];
attr_tuple![A:0, B:1, C:2, D:3, E:4, F:5, G:6, H:7, I:8, J:9, K:10, L:11, M:12, N:13, O:14, P:15, Q:16, R:17, S:18, T:19, U:20, V:21, W:22, X:23];
attr_tuple![A:0, B:1, C:2, D:3, E:4, F:5, G:6, H:7, I:8, J:9, K:10, L:11, M:12, N:13, O:14, P:15, Q:16, R:17, S:18, T:19, U:20, V:21, W:22, X:23, Y:24];
attr_tuple![A:0, B:1, C:2, D:3, E:4, F:5, G:6, H:7, I:8, J:9, K:10, L:11, M:12, N:13, O:14, P:15, Q:16, R:17, S:18, T:19, U:20, V:21, W:22, X:23, Y:24, Z:25];

pub unsafe trait DataSource<'v>: Sized {
    type Buffers: 'v;

    fn setup_sources(vao: &mut VertexArray);
    fn apply_sources(buffers: Self::Buffers, vao: &mut Binder<'_>) -> Option<usize>;
}

unsafe impl<'v> DataSource<'v> for Nil {
    type Buffers = Nil;

    fn setup_sources(_vao: &mut VertexArray) {}

    fn apply_sources(_buffers: Self::Buffers, _binder: &mut Binder<'_>) -> Option<usize> {
        None
    }
}

unsafe impl<'v, T, L> DataSource<'v> for Cons<T, L>
where
    T: InterleavedAttribute,
    L: DataSource<'v>,
{
    type Buffers = Cons<&'v Buffer<T>, L::Buffers>;

    fn setup_sources(vao: &mut VertexArray) {
        vao.push_binding::<T>();
        L::setup_sources(vao);
    }

    fn apply_sources(buffers: Self::Buffers, binder: &mut Binder<'_>) -> Option<usize> {
        binder.bind_buffer(buffers.0);
        Some(
            buffers
                .0
                .len()
                .min(L::apply_sources(buffers.1, binder).unwrap_or(std::usize::MAX)),
        )
    }
}
