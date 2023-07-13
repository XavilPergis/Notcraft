use crate::util;

#[derive(Clone, Debug, PartialEq, Default)]
pub struct Spline {
    points: Vec<SplinePoint>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SplinePoint {
    pub start: f32,
    pub height: f32,
}

impl Spline {
    pub fn with_point(mut self, point: SplinePoint) -> Self {
        match self
            .points
            .binary_search_by(|cur| PartialOrd::partial_cmp(&cur.start, &point.start).unwrap())
        {
            Ok(idx) => self.points.insert(idx + 1, point),
            Err(idx) => self.points.insert(idx, point),
        }
        self
    }

    pub fn sample(&self, value: f32) -> f32 {
        match self
            .points
            .binary_search_by(|cur| PartialOrd::partial_cmp(&cur.start, &value).unwrap())
        {
            // out of bounds of this sampler; just define everything outside to be the values of the
            // respective endpoints.
            Err(0) => self.points[0].height,
            Err(idx) if idx == self.points.len() => self.points[idx - 1].height,

            Ok(idx) => self.points[idx].height,
            Err(idx) => {
                assert!(self.points[idx - 1].start <= value);
                assert!(self.points[idx].start >= value);
                util::remap(
                    self.points[idx - 1].start,
                    self.points[idx].start,
                    self.points[idx - 1].height,
                    self.points[idx].height,
                    value,
                )
            }
        }
    }
}
