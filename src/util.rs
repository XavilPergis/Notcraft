use engine::{ChunkPos, WorldPos};
use cgmath::{Point3, Vector3};
use std::cmp::Ordering;

/// Version of `min` that only requires `PartialOrd`
pub fn min<S: PartialOrd + Copy>(lhs: S, rhs: S) -> S {
    match lhs.partial_cmp(&rhs) {
        Some(Ordering::Less) | Some(Ordering::Equal) | None => lhs,
        _ => rhs,
    }
}

/// Version of `max` that only requires `PartialOrd`
pub fn max<S: PartialOrd + Copy>(lhs: S, rhs: S) -> S {
    match lhs.partial_cmp(&rhs) {
        Some(Ordering::Greater) | Some(Ordering::Equal) | None => lhs,
        _ => rhs,
    }
}

/// Limits the range of `x` to be within `[a, b]`
pub fn clamp<T: PartialOrd + Copy>(x: T, a: T, b: T) -> T {
    if x < a { a }
    else if x > b { b }
    else { x }
}

/// x / y, round towards negative inf
pub fn floor_div(x: i32, y: i32) -> i32 {
    let result = x / y;
    let remainder = x % y;
    if remainder < 0 { result - 1 } else { result }
}

pub fn to_vector<S>(point: Point3<S>) -> Vector3<S> {
    Vector3::new(point.x, point.y, point.z)
}

pub fn to_point<S>(vec: Vector3<S>) -> Point3<S> {
    Point3::new(vec.x, vec.y, vec.z)
}

/// Get a chunk position from a world position
pub fn get_chunk_pos(pos: WorldPos) -> (ChunkPos, Vector3<i32>) {
    const SIZE: i32 = ::engine::chunk::CHUNK_SIZE as i32;
    let cx = ::util::floor_div(pos.x, SIZE);
    let cy = ::util::floor_div(pos.y, SIZE);
    let cz = ::util::floor_div(pos.z, SIZE);

    let cpos = Point3::new(cx, cy, cz);
    let bpos = pos - (SIZE*cpos);

    (cpos, bpos)
}

/// Tests if `pos` is within `r` units from `center`
pub fn in_range(pos: WorldPos, center: WorldPos, r: i32) -> bool {
    pos.x <= center.x + r && pos.x >= center.x - r &&
    pos.y <= center.y + r && pos.y >= center.y - r &&
    pos.z <= center.z + r && pos.z >= center.z - r
}