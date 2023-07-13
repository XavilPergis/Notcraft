use crate::{aabb::Aabb, world::BlockPos};
use bevy_app::{AppExit, EventWriter};
use bevy_ecs::prelude::In;
use nalgebra::{point, vector, Point3, Vector3};
use std::{cmp::Ordering, fmt::Display};

#[inline(always)]
pub fn invlerp(a: f32, b: f32, n: f32) -> f32 {
    // you can get this by solving for `t` in the equation for `lerp`
    (n - a) / (b - a)
}

#[inline(always)]
pub fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a * (1.0 - t) + b * t
}

#[inline(always)]
pub fn remap(input_start: f32, input_end: f32, output_start: f32, output_end: f32, n: f32) -> f32 {
    lerp(output_start, output_end, invlerp(input_start, input_end, n))
}

#[cfg(test)]
mod tests {
    use approx::assert_relative_eq;

    use super::*;

    #[test]
    fn test_remap() {
        assert_relative_eq!(remap(0.0, 1.0, 0.0, 1.0, 0.5), 0.5);
        assert_relative_eq!(remap(0.0, 2.0, 0.0, 1.0, 0.5), 0.25);
        assert_relative_eq!(remap(0.0, 1.0, 0.0, 2.0, 0.5), 1.0);
        assert_relative_eq!(remap(0.0, 2.0, 0.0, 2.0, 0.5), 0.5);
    }
}

#[inline(always)]
pub fn lerp_vec(a: Vector3<f32>, b: Vector3<f32>, t: f32) -> Vector3<f32> {
    vector![lerp(a.x, b.x, t), lerp(a.y, b.y, t), lerp(a.z, b.z, t)]
}

#[inline(always)]
pub fn lerp_point(a: Point3<f32>, b: Point3<f32>, t: f32) -> Point3<f32> {
    point![lerp(a.x, b.x, t), lerp(a.y, b.y, t), lerp(a.z, b.z, t)]
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
pub fn clamp<T: PartialOrd + Copy>(a: T, b: T, x: T) -> T {
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

#[derive(Debug)]
pub struct ChannelPair<T> {
    pub rx: crossbeam_channel::Receiver<T>,
    pub tx: crossbeam_channel::Sender<T>,
}

impl<T> Default for ChannelPair<T> {
    fn default() -> Self {
        let (tx, rx) = crossbeam_channel::unbounded();
        Self { rx, tx }
    }
}

impl<T> ChannelPair<T> {
    pub fn sender(&self) -> crossbeam_channel::Sender<T> {
        self.tx.clone()
    }
}

pub fn block_aabb(block: BlockPos) -> Aabb {
    let pos = point![block.x as f32, block.y as f32, block.z as f32];
    Aabb {
        min: pos,
        max: pos + vector![1.0, 1.0, 1.0],
    }
}

pub fn handle_error_internal<T, E>(In(res): In<Result<T, E>>, mut exit: EventWriter<AppExit>)
where
    E: Display,
{
    match res {
        Ok(_) => {}
        Err(err) => {
            log::error!("{}", err);
            exit.send(AppExit);
        }
    }
}

#[macro_export]
macro_rules! try_system {
    ($sys:expr) => {
        $sys.system()
            .chain($crate::util::handle_error_internal.system())
    };
}

pub use try_system;
