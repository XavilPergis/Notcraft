use cgmath::{Matrix2, Matrix3, Matrix4, Point1, Point2, Point3, Vector2, Vector3, Vector4};
use gl_api::context::Context;

pub type UniformLocation = ::gl::types::GLint;

pub trait Uniform {
    fn set_uniform(&self, ctx: &Context, uniform_location: UniformLocation);
}
macro_rules! uniform_matrix {
    ($self:ident, $type:ty => $func:ident($($expr:expr),*)) => (
        impl Uniform for $type {
            #[inline(always)]
            fn set_uniform(&$self, _ctx: &Context, location: UniformLocation) {
                gl_call!(assert $func(location, $($expr),*));
            }
        }
    )
}

#[allow(unused_macros)]
macro_rules! uniform {
    // Macro cleanliness means that we can't use `self` in the macro invocation scope
    // without first introducing it into scope there (slightly unfortunate)
    ($self:ident, $type:ty => $vec_func:ident, $func:ident($($expr:expr),*)) => (
        impl Uniform for $type {
            #[inline(always)]
            fn set_uniform(&$self, _ctx: &Context, location: UniformLocation) {
                gl_call!(assert $func(location, $($expr),*));
            }
        }

        impl Uniform for [$type] {
            #[inline(always)]
            fn set_uniform(&self, _ctx: &Context, location: UniformLocation) {
                gl_call!(assert $vec_func(location, self.len() as i32, self.as_ptr() as *const _));
            }
        }
    )
}

// void glUniform1fv(	GLint location,
//  	GLsizei count,
//  	const GLfloat *value);

uniform!(self, f32                  => Uniform1fv, Uniform1f(*self));
uniform!(self, [f32; 1]             => Uniform1fv, Uniform1f(self[0]));
uniform!(self, [f32; 2]             => Uniform2fv, Uniform2f(self[0], self[1]));
uniform!(self, [f32; 3]             => Uniform3fv, Uniform3f(self[0], self[1], self[2]));
uniform!(self, [f32; 4]             => Uniform4fv, Uniform4f(self[0], self[1], self[2], self[3]));
uniform!(self, (f32,)               => Uniform1fv, Uniform1f(self.0));
uniform!(self, (f32, f32)           => Uniform2fv, Uniform2f(self.0, self.1));
uniform!(self, (f32, f32, f32)      => Uniform3fv, Uniform3f(self.0, self.1, self.2));
uniform!(self, (f32, f32, f32, f32) => Uniform4fv, Uniform4f(self.0, self.1, self.2, self.3));
uniform!(self, Vector2<f32>         => Uniform2fv, Uniform2f(self.x, self.y));
uniform!(self, Vector3<f32>         => Uniform3fv, Uniform3f(self.x, self.y, self.z));
uniform!(self, Vector4<f32>         => Uniform4fv, Uniform4f(self.x, self.y, self.z, self.w));
uniform!(self, Point1<f32>          => Uniform1fv, Uniform1f(self.x));
uniform!(self, Point2<f32>          => Uniform2fv, Uniform2f(self.x, self.y));
uniform!(self, Point3<f32>          => Uniform3fv, Uniform3f(self.x, self.y, self.z));

uniform!(self, f64                  => Uniform1dv, Uniform1d(*self));
uniform!(self, [f64; 1]             => Uniform1dv, Uniform1d(self[0]));
uniform!(self, [f64; 2]             => Uniform2dv, Uniform2d(self[0], self[1]));
uniform!(self, [f64; 3]             => Uniform3dv, Uniform3d(self[0], self[1], self[2]));
uniform!(self, [f64; 4]             => Uniform4dv, Uniform4d(self[0], self[1], self[2], self[3]));
uniform!(self, (f64,)               => Uniform1dv, Uniform1d(self.0));
uniform!(self, (f64, f64)           => Uniform2dv, Uniform2d(self.0, self.1));
uniform!(self, (f64, f64, f64)      => Uniform3dv, Uniform3d(self.0, self.1, self.2));
uniform!(self, (f64, f64, f64, f64) => Uniform4dv, Uniform4d(self.0, self.1, self.2, self.3));
uniform!(self, Vector2<f64>         => Uniform2dv, Uniform2d(self.x, self.y));
uniform!(self, Vector3<f64>         => Uniform3dv, Uniform3d(self.x, self.y, self.z));
uniform!(self, Vector4<f64>         => Uniform4dv, Uniform4d(self.x, self.y, self.z, self.w));
uniform!(self, Point1<f64>          => Uniform1dv, Uniform1d(self.x));
uniform!(self, Point2<f64>          => Uniform2dv, Uniform2d(self.x, self.y));
uniform!(self, Point3<f64>          => Uniform3dv, Uniform3d(self.x, self.y, self.z));

uniform!(self, i32                  => Uniform1iv, Uniform1i(*self));
uniform!(self, [i32; 1]             => Uniform1iv, Uniform1i(self[0]));
uniform!(self, [i32; 2]             => Uniform2iv, Uniform2i(self[0], self[1]));
uniform!(self, [i32; 3]             => Uniform3iv, Uniform3i(self[0], self[1], self[2]));
uniform!(self, [i32; 4]             => Uniform4iv, Uniform4i(self[0], self[1], self[2], self[3]));
uniform!(self, (i32,)               => Uniform1iv, Uniform1i(self.0));
uniform!(self, (i32, i32)           => Uniform2iv, Uniform2i(self.0, self.1));
uniform!(self, (i32, i32, i32)      => Uniform3iv, Uniform3i(self.0, self.1, self.2));
uniform!(self, (i32, i32, i32, i32) => Uniform4iv, Uniform4i(self.0, self.1, self.2, self.3));
uniform!(self, Vector2<i32>         => Uniform2iv, Uniform2i(self.x, self.y));
uniform!(self, Vector3<i32>         => Uniform3iv, Uniform3i(self.x, self.y, self.z));
uniform!(self, Vector4<i32>         => Uniform4iv, Uniform4i(self.x, self.y, self.z, self.w));
uniform!(self, Point1<i32>          => Uniform1iv, Uniform1i(self.x));
uniform!(self, Point2<i32>          => Uniform2iv, Uniform2i(self.x, self.y));
uniform!(self, Point3<i32>          => Uniform3iv, Uniform3i(self.x, self.y, self.z));

uniform!(self, u32                  => Uniform1uiv, Uniform1ui(*self));
uniform!(self, [u32; 1]             => Uniform1uiv, Uniform1ui(self[0]));
uniform!(self, [u32; 2]             => Uniform2uiv, Uniform2ui(self[0], self[1]));
uniform!(self, [u32; 3]             => Uniform3uiv, Uniform3ui(self[0], self[1], self[2]));
uniform!(self, [u32; 4]             => Uniform4uiv, Uniform4ui(self[0], self[1], self[2], self[3]));
uniform!(self, (u32,)               => Uniform1uiv, Uniform1ui(self.0));
uniform!(self, (u32, u32)           => Uniform2uiv, Uniform2ui(self.0, self.1));
uniform!(self, (u32, u32, u32)      => Uniform3uiv, Uniform3ui(self.0, self.1, self.2));
uniform!(self, (u32, u32, u32, u32) => Uniform4uiv, Uniform4ui(self.0, self.1, self.2, self.3));
uniform!(self, Vector2<u32>         => Uniform2uiv, Uniform2ui(self.x, self.y));
uniform!(self, Vector3<u32>         => Uniform3uiv, Uniform3ui(self.x, self.y, self.z));
uniform!(self, Vector4<u32>         => Uniform4uiv, Uniform4ui(self.x, self.y, self.z, self.w));
uniform!(self, Point1<u32>          => Uniform1uiv, Uniform1ui(self.x));
uniform!(self, Point2<u32>          => Uniform2uiv, Uniform2ui(self.x, self.y));
uniform!(self, Point3<u32>          => Uniform3uiv, Uniform3ui(self.x, self.y, self.z));

use cgmath::Matrix;

uniform_matrix!(self, Matrix4<f32> => UniformMatrix4fv(1, gl::FALSE, self.as_ptr() as *const f32));
uniform_matrix!(self, Matrix4<f64> => UniformMatrix4dv(1, gl::FALSE, self.as_ptr() as *const f64));
uniform_matrix!(self, Matrix3<f32> => UniformMatrix3fv(1, gl::FALSE, self.as_ptr() as *const f32));
uniform_matrix!(self, Matrix3<f64> => UniformMatrix3dv(1, gl::FALSE, self.as_ptr() as *const f64));
uniform_matrix!(self, Matrix2<f32> => UniformMatrix4fv(1, gl::FALSE, self.as_ptr() as *const f32));
uniform_matrix!(self, Matrix2<f64> => UniformMatrix4dv(1, gl::FALSE, self.as_ptr() as *const f64));

impl<'a, U: Uniform> Uniform for &'a U {
    fn set_uniform(&self, ctx: &Context, location: UniformLocation) {
        (*self).set_uniform(ctx, location);
    }
}
