use nalgebra::{Point3, Vector3};
use std::cmp::Ordering;

// pub fn aspect_ratio(window: &GlWindow) -> Option<f32> {
//     window.get_inner_size().map(|size| {
//         let size: (f64, f64) =
// size.to_physical(window.get_hidpi_factor()).into();         size.0 as f32 /
// size.1 as f32     })
// }

// pub fn lerp_angle(a: Deg<f32>, b: Deg<f32>, t: f32) -> Deg<f32> {
//     Deg(a.0 * (1.0 - t) + b.0 * clamp(t, 0.0, 1.0))
// }

pub fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a * (1.0 - t) + b * t
}

pub fn lerp_vec(a: Vector3<f32>, b: Vector3<f32>, t: f32) -> Vector3<f32> {
    Vector3::new(lerp(a.x, b.x, t), lerp(a.y, b.y, t), lerp(a.z, b.z, t))
}

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
    if x < a {
        a
    } else if x > b {
        b
    } else {
        x
    }
}

/// x / y, round towards negative inf
pub fn floor_div(x: i32, y: i32) -> i32 {
    let result = x / y;
    let remainder = x % y;
    if remainder < 0 {
        result - 1
    } else {
        result
    }
}

// pub fn to_vector<S>(point: Point3<S>) -> Vector3<S> {
//     Vector3::new(point.x, point.y, point.z)
// }

// pub fn to_point<S>(vec: Vector3<S>) -> Point3<S> {
//     Point3::new(vec.x, vec.y, vec.z)
// }

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
