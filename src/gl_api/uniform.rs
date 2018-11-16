use cgmath::{Matrix2, Matrix3, Matrix4, Point1, Point2, Point3, Vector2, Vector3, Vector4};

pub type UniformLocation = ::gl::types::GLint;

pub trait Uniform {
    fn set_uniform(&self, uniform_location: UniformLocation);
}

impl<'a> Uniform for &'a [f32] {
    fn set_uniform(&self, location: UniformLocation) {
        unsafe {
            ::gl::Uniform1fv(location, self.len() as i32, self.as_ptr());
            let error = ::gl::GetError();
            if error != 0 {
                panic!("[f32] OpenGL Returned error {}", error);
            }
        }
    }
}
impl Uniform for [(f32, f32)] {
    fn set_uniform(&self, location: UniformLocation) {
        unsafe {
            ::gl::Uniform2fv(location, self.len() as i32, self.as_ptr() as *const f32);
            let error = ::gl::GetError();
            if error != 0 {
                panic!("OpenGL Returned error {}", error);
            }
        }
    }
}
impl<'a> Uniform for &'a [Vector3<f32>] {
    fn set_uniform(&self, location: UniformLocation) {
        unsafe {
            ::gl::Uniform3fv(location, self.len() as i32, self.as_ptr() as *const f32);
            let error = ::gl::GetError();
            if error != 0 {
                panic!("[Vector3<f32>] OpenGL Returned error {}", error);
            }
        }
    }
}

#[allow(unused_macros)]
macro_rules! uniform {
    // Macro cleanliness means that we can't use `self` in the macro invocation scope
    // without first introducing it into scope there (slightly unfortunate)
    ($self:ident, $type:ty => $func:ident($($expr:expr),*)) => (
        impl Uniform for $type {
            #[inline(always)]
            fn set_uniform(&$self, location: UniformLocation) {
                use ::gl;
                let _res = unsafe { gl::$func(location, $($expr,)*) };
                // println!("{} {:?}", stringify!($type), res);
                let error = unsafe { gl::GetError() };
                if error != 0 { panic!("OpenGL Returned error {}", error); }
            }
        }
    )
}

#[allow(unused_macros)]
macro_rules! uniform_vector {
    // Macro cleanliness means that we can't use `self` in the macro invocation scope
    // without first introducing it into scope there (slightly unfortunate)
    ($self:ident, $type:ty => $func:ident($($expr:expr),*)) => (
        impl Uniform for $type {
            #[inline(always)]
            fn set_uniform(&$self, location: UniformLocation) {
                use ::gl;
                let _res = unsafe { gl::$func(location, $($expr,)*) };
                // println!("{} {:?}", stringify!($type), res);
                let error = unsafe { gl::GetError() };
                if error != 0 { panic!("OpenGL Returned error {}", error); }
            }
        }
    )
}

uniform!(self, f32 => Uniform1f(*self));
uniform!(self, [f32; 1] => Uniform1f(self[0]));
uniform!(self, [f32; 2] => Uniform2f(self[0], self[1]));
uniform!(self, [f32; 3] => Uniform3f(self[0], self[1], self[2]));
uniform!(self, [f32; 4] => Uniform4f(self[0], self[1], self[2], self[3]));
uniform!(self, (f32,) => Uniform1f(self.0));
uniform!(self, (f32, f32) => Uniform2f(self.0, self.1));
uniform!(self, (f32, f32, f32) => Uniform3f(self.0, self.1, self.2));
uniform!(self, (f32, f32, f32, f32) => Uniform4f(self.0, self.1, self.2, self.3));
uniform!(self, Vector2<f32> => Uniform2f(self.x, self.y));
uniform!(self, Vector3<f32> => Uniform3f(self.x, self.y, self.z));
uniform!(self, Vector4<f32> => Uniform4f(self.x, self.y, self.z, self.w));
uniform!(self, Point1<f32> => Uniform1f(self.x));
uniform!(self, Point2<f32> => Uniform2f(self.x, self.y));
uniform!(self, Point3<f32> => Uniform3f(self.x, self.y, self.z));

uniform!(self, f64 => Uniform1d(*self));
uniform!(self, [f64; 1] => Uniform1d(self[0]));
uniform!(self, [f64; 2] => Uniform2d(self[0], self[1]));
uniform!(self, [f64; 3] => Uniform3d(self[0], self[1], self[2]));
uniform!(self, [f64; 4] => Uniform4d(self[0], self[1], self[2], self[3]));
uniform!(self, (f64,) => Uniform1d(self.0));
uniform!(self, (f64, f64) => Uniform2d(self.0, self.1));
uniform!(self, (f64, f64, f64) => Uniform3d(self.0, self.1, self.2));
uniform!(self, (f64, f64, f64, f64) => Uniform4d(self.0, self.1, self.2, self.3));
uniform!(self, Vector2<f64> => Uniform2d(self.x, self.y));
uniform!(self, Vector3<f64> => Uniform3d(self.x, self.y, self.z));
uniform!(self, Vector4<f64> => Uniform4d(self.x, self.y, self.z, self.w));
uniform!(self, Point1<f64> => Uniform1d(self.x));
uniform!(self, Point2<f64> => Uniform2d(self.x, self.y));
uniform!(self, Point3<f64> => Uniform3d(self.x, self.y, self.z));

uniform!(self, i32 => Uniform1i(*self));
uniform!(self, [i32; 1] => Uniform1i(self[0]));
uniform!(self, [i32; 2] => Uniform2i(self[0], self[1]));
uniform!(self, [i32; 3] => Uniform3i(self[0], self[1], self[2]));
uniform!(self, [i32; 4] => Uniform4i(self[0], self[1], self[2], self[3]));
uniform!(self, (i32,) => Uniform1i(self.0));
uniform!(self, (i32, i32) => Uniform2i(self.0, self.1));
uniform!(self, (i32, i32, i32) => Uniform3i(self.0, self.1, self.2));
uniform!(self, (i32, i32, i32, i32) => Uniform4i(self.0, self.1, self.2, self.3));
uniform!(self, Vector2<i32> => Uniform2i(self.x, self.y));
uniform!(self, Vector3<i32> => Uniform3i(self.x, self.y, self.z));
uniform!(self, Vector4<i32> => Uniform4i(self.x, self.y, self.z, self.w));
uniform!(self, Point1<i32> => Uniform1i(self.x));
uniform!(self, Point2<i32> => Uniform2i(self.x, self.y));
uniform!(self, Point3<i32> => Uniform3i(self.x, self.y, self.z));

uniform!(self, u32 => Uniform1ui(*self));
uniform!(self, [u32; 1] => Uniform1ui(self[0]));
uniform!(self, [u32; 2] => Uniform2ui(self[0], self[1]));
uniform!(self, [u32; 3] => Uniform3ui(self[0], self[1], self[2]));
uniform!(self, [u32; 4] => Uniform4ui(self[0], self[1], self[2], self[3]));
uniform!(self, (u32,) => Uniform1ui(self.0));
uniform!(self, (u32, u32) => Uniform2ui(self.0, self.1));
uniform!(self, (u32, u32, u32) => Uniform3ui(self.0, self.1, self.2));
uniform!(self, (u32, u32, u32, u32) => Uniform4ui(self.0, self.1, self.2, self.3));
uniform!(self, Vector2<u32> => Uniform2ui(self.x, self.y));
uniform!(self, Vector3<u32> => Uniform3ui(self.x, self.y, self.z));
uniform!(self, Vector4<u32> => Uniform4ui(self.x, self.y, self.z, self.w));
uniform!(self, Point1<u32> => Uniform1ui(self.x));
uniform!(self, Point2<u32> => Uniform2ui(self.x, self.y));
uniform!(self, Point3<u32> => Uniform3ui(self.x, self.y, self.z));

use cgmath::Matrix;

uniform!(self, Matrix4<f32> => UniformMatrix4fv(1, gl::FALSE, self.as_ptr() as *const f32));
uniform!(self, Matrix4<f64> => UniformMatrix4dv(1, gl::FALSE, self.as_ptr() as *const f64));
uniform!(self, Matrix3<f32> => UniformMatrix3fv(1, gl::FALSE, self.as_ptr() as *const f32));
uniform!(self, Matrix3<f64> => UniformMatrix3dv(1, gl::FALSE, self.as_ptr() as *const f64));
uniform!(self, Matrix2<f32> => UniformMatrix4fv(1, gl::FALSE, self.as_ptr() as *const f32));
uniform!(self, Matrix2<f64> => UniformMatrix4dv(1, gl::FALSE, self.as_ptr() as *const f64));

impl<'a, U: Uniform> Uniform for &'a U {
    fn set_uniform(&self, location: UniformLocation) {
        (*self).set_uniform(location);
    }
}
