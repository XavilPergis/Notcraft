use nalgebra::{vector, Point3, Vector3};
use std::cmp::Ordering;

#[inline(always)]
pub fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a * (1.0 - t) + b * t
}

#[inline(always)]
pub fn lerp_vec(a: Vector3<f32>, b: Vector3<f32>, t: f32) -> Vector3<f32> {
    vector![lerp(a.x, b.x, t), lerp(a.y, b.y, t), lerp(a.z, b.z, t)]
}

/// Version of `min` that only requires `PartialOrd`
#[inline(always)]
pub fn min<S: PartialOrd + Copy>(lhs: S, rhs: S) -> S {
    match lhs.partial_cmp(&rhs) {
        Some(Ordering::Less) | Some(Ordering::Equal) | None => lhs,
        _ => rhs,
    }
}

/// Version of `max` that only requires `PartialOrd`
#[inline(always)]
pub fn max<S: PartialOrd + Copy>(lhs: S, rhs: S) -> S {
    match lhs.partial_cmp(&rhs) {
        Some(Ordering::Greater) | Some(Ordering::Equal) | None => lhs,
        _ => rhs,
    }
}

/// Limits the range of `x` to be within `[a, b]`
#[inline(always)]
pub fn clamp<T: PartialOrd + Copy>(x: T, a: T, b: T) -> T {
    if x < a {
        a
    } else if x > b {
        b
    } else {
        x
    }
}

#[inline(always)]
pub fn is_within<T: PartialOrd + Copy>(t: T, min: T, max: T) -> bool {
    t >= min && t <= max
}

#[inline(always)]
pub fn is_between<T: PartialOrd + Copy>(t: T, min: T, max: T) -> bool {
    t > min && t < max
}

/// x / y, round towards negative inf
#[inline(always)]
pub fn floor_div(x: i32, y: i32) -> i32 {
    let result = x / y;
    let remainder = x % y;
    if remainder < 0 {
        result - 1
    } else {
        result
    }
}

/// Tests if `pos` is within `r` units from `center`
pub fn in_range(pos: Point3<i32>, center: Point3<i32>, radii: Vector3<i32>) -> bool {
    pos.x <= center.x + radii.x
        && pos.x >= center.x - radii.x
        && pos.y <= center.y + radii.y
        && pos.y >= center.y - radii.y
        && pos.z <= center.z + radii.z
        && pos.z >= center.z - radii.z
}

/// Mathematical mod function
#[inline(always)]
pub fn modulo(a: f32, b: f32) -> f32 {
    (a % b + b) % b
}

pub fn read_file<P: AsRef<std::path::Path>>(path: P) -> std::io::Result<String> {
    use std::{fs::File, io::Read};

    let mut file = File::open(path)?;
    let mut buffer = String::new();
    file.read_to_string(&mut buffer)?;

    Ok(buffer)
}

pub struct Defer<F: FnOnce()>(pub Option<F>);
impl<F: FnOnce()> Drop for Defer<F> {
    fn drop(&mut self) {
        (self.0.take().unwrap())();
    }
}

#[macro_export]
macro_rules! defer {
    ($($code:tt)*) => {
        let _defer = $crate::util::Defer(Some(|| drop({ $($code)* })));
    };
}

pub use defer;
