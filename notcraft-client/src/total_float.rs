#[derive(Copy, Clone, Debug, Default)]
#[repr(transparent)]
pub struct TotalFloat<T>(pub T);

macro_rules! impl_float_ord {
    ($float_type:ty, $int_type:ty) => {
        impl TotalFloat<$float_type> {
            pub fn as_orderable(value: $float_type) -> $int_type {
                const ZERO: $int_type = 0;
                const SHIFT: $int_type = (<$int_type>::BITS - 1) as $int_type;

                // from http://stereopsis.com/radix.html
                let bits = value.to_bits();

                // to fix up a floating point number for comparison, we have to invert the sign
                // bit, and invert all other bits if the sign bit was set (the number was
                // negative). we implement this here with a single XOR mask.

                // this converts a negative sign (sign bit set) to an all-bits-set mask,
                // covering our case where we invert all bits if the number was negative.
                let mut mask = ZERO.wrapping_sub(bits >> SHIFT);

                // while the negative case is covered, we still need to reverse the sign bit in
                // both cases, so we can OR in the sign bit's place in the mask. this has no
                // effect on the negative case that is already covered, but makes the positive
                // case correct.
                mask |= (1 << SHIFT);

                bits ^ mask
            }
        }

        impl Eq for TotalFloat<$float_type> {}
        impl PartialEq for TotalFloat<$float_type> {
            fn eq(&self, other: &Self) -> bool {
                self.0.to_bits() == other.0.to_bits()
            }
        }

        impl Ord for TotalFloat<$float_type> {
            fn cmp(&self, other: &Self) -> std::cmp::Ordering {
                Self::as_orderable(self.0).cmp(&Self::as_orderable(other.0))
            }
        }
        impl PartialOrd for TotalFloat<$float_type> {
            fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
                Self::as_orderable(self.0).partial_cmp(&Self::as_orderable(other.0))
            }
        }

        impl std::hash::Hash for TotalFloat<$float_type> {
            fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
                self.0.to_bits().hash(state);
            }
        }
    };
}

impl_float_ord!(f32, u32);
impl_float_ord!(f64, u64);

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        collections::hash_map::DefaultHasher,
        hash::{Hash, Hasher},
    };

    #[rustfmt::skip]
    const F32_EQ_SAMPLES: &[f32] = &[
        0.0, -0.0, 1.0, -1.0,
        f32::MIN,  f32::MAX,
        f32::INFINITY, f32::NEG_INFINITY,
        f32::NAN, -f32::NAN,
    ];

    #[rustfmt::skip]
    const F64_EQ_SAMPLES: &[f64] = &[
        0.0, -0.0, 1.0, -1.0,
        f64::MIN,  f64::MAX,
        f64::INFINITY, f64::NEG_INFINITY,
        f64::NAN, -f64::NAN,
    ];

    #[test]
    fn test_equality_f32() {
        for &number in F32_EQ_SAMPLES.iter() {
            assert_eq!(TotalFloat(number), TotalFloat(number));
        }
    }

    #[test]
    fn test_equality_f64() {
        for &number in F64_EQ_SAMPLES.iter() {
            assert_eq!(TotalFloat(number), TotalFloat(number));
        }
    }

    fn hash<H: Hash>(value: H) -> u64 {
        let mut hasher = DefaultHasher::new();
        value.hash(&mut hasher);
        hasher.finish()
    }

    #[test]
    fn test_hash_f32() {
        for &number in F32_EQ_SAMPLES.iter() {
            assert_eq!(hash(TotalFloat(number)), hash(TotalFloat(number)));
        }
    }

    #[test]
    fn test_hash_f64() {
        for &number in F64_EQ_SAMPLES.iter() {
            assert_eq!(hash(TotalFloat(number)), hash(TotalFloat(number)));
        }
    }
}
