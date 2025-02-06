//! Representation and math of musical time intervals.

use std::{cmp::Ordering, ops};

use gcd::Gcd;
use serde::{de::{self, Visitor}, Deserialize, Deserializer, Serialize};

/// Represents a time interval as a beat fraction. Often, the interval is
/// measured from the start of the song. Operations that would overflow the
/// denominator instead saturate it and adjust the numerator to approximate
/// the result.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct Timespan {
    n: i32,
    d: u8,
}

impl Timespan {
    pub const ZERO: Timespan = Timespan { n: 0, d: 1 };

    pub fn new(n: i32, d: u8) -> Self {
        let gcd = n.unsigned_abs().gcd(d as u32);
        Self {
            n: n / gcd as i32,
            d: d / gcd as u8,
        }
    }

    /// Returns a rational approximation of a float. Always uses the highest
    /// possible denominator.
    pub fn approximate(f: f64) -> Self {
        let d = u8::MAX;
        let n = (f * d as f64).round() as i32;
        Self::new(n, d)
    }

    pub fn as_f32(&self) -> f32 {
        (*self).into()
    }

    pub fn as_f64(&self) -> f64 {
        (*self).into()
    }

    pub fn abs(&self) -> Self {
        Self { n: self.n.abs(), d: self.d }
    }

    /// Returns the numerator.
    pub fn num(&self) -> i32 {
        self.n
    }

    /// Returns the denominator.
    pub fn den(&self) -> u8 {
        self.d
    }
}

impl Default for Timespan {
    fn default() -> Self {
        Self::ZERO
    }
}

impl PartialOrd for Timespan {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Timespan {
    fn cmp(&self, other: &Self) -> Ordering {
        self.as_f64().total_cmp(&other.as_f64())
    }
}

impl ops::Neg for Timespan {
    type Output = Self;

    fn neg(self) -> Self::Output {
        Self { n: -self.n, d: self.d }
    }
}

impl ops::Add<Self> for Timespan {
    type Output = Self;

    fn add(self, rhs: Self) -> Self {
        let gcd = self.d.gcd(rhs.d);
        let d = (self.d as u16 * rhs.d as u16) / gcd as u16;
        if d <= u8::MAX as u16 {
            let n = (self.n * (rhs.d / gcd) as i32)
                + (rhs.n * (self.d / gcd) as i32);
            Self::new(n, d as u8)
        } else {
            Self::approximate(self.as_f64() + rhs.as_f64())
        }
    }
}

impl ops::AddAssign<Self> for Timespan {
    fn add_assign(&mut self, rhs: Self) {
        *self = *self + rhs;
    }
}

impl ops::Sub<Self> for Timespan {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self::Output {
        self + (-rhs)
    }
}

impl ops::Mul<Self> for Timespan {
    type Output = Self;

    fn mul(self, rhs: Self) -> Self::Output {
        let n = self.n as i64 * rhs.n as i64;
        let d = self.d as u16 * rhs.d as u16;
        let gcd = n.unsigned_abs().gcd(d as u64);
        let d = d / gcd as u16;

        if d <= u8::MAX as u16 {
            let n = n as i32 / gcd as i32;
            Self::new(n, d as u8)
        } else {
            Self::approximate(self.as_f64() * rhs.as_f64())
        }
    }
}

impl ops::Div<Self> for Timespan {
    type Output = Self;

    fn div(self, rhs: Self) -> Self::Output {
        let n = self.n as i64 * rhs.d as i64;
        let d = self.d as i64 * rhs.n as i64;
        let gcd = n.unsigned_abs().gcd(d as u64);
        let d = d / gcd as i64;

        if d <= u8::MAX as i64 {
            let n = n as i32 / gcd as i32;
            Self::new(n, d as u8)
        } else {
            Self::approximate(self.as_f64() * rhs.as_f64())
        }
    }
}

impl From::<Timespan> for f64 {
    fn from(value: Timespan) -> Self {
        value.n as f64 / value.d as f64
    }
}

impl From::<Timespan> for f32 {
    fn from(value: Timespan) -> Self {
        value.n as f32 / value.d as f32
    }
}

// custom implementation to load from legacy save files
impl<'de> Deserialize<'de> for Timespan {
    fn deserialize<D>(d: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>
    {
        d.deserialize_any(TimespanVisitor)
    }
}

struct TimespanVisitor;

const LEGACY_DENOM: u64 = 5040;

impl<'de> Visitor<'de> for TimespanVisitor {
    type Value = Timespan;

    fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(formatter, "integer or struct Timespan")
    }

    fn visit_u64<E>(self, v: u64) -> Result<Self::Value, E>
    where
        E: serde::de::Error
    {
        let gcd = v.gcd(LEGACY_DENOM);
        let d = LEGACY_DENOM / gcd;
        Ok(if d <= u8::MAX as u64 {
            Timespan::new((v / gcd) as i32, d as u8)
        } else {
            Timespan::approximate(v as f64 / LEGACY_DENOM as f64)
        })
    }

    fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
    where
        A: serde::de::SeqAccess<'de>
    {
        let n = seq.next_element()?
            .ok_or_else(|| de::Error::invalid_length(0, &self))?;
        let d = seq.next_element()?
            .ok_or_else(|| de::Error::invalid_length(1, &self))?;
        Ok(Timespan { n, d })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new() {
        assert_eq!(Timespan::new(1, 1), Timespan { n: 1, d: 1 });
        assert_eq!(Timespan::new(2, 2), Timespan { n: 1, d: 1 });
        assert_eq!(Timespan::new(10, 6), Timespan { n: 5, d: 3 });
    }

    #[test]
    fn test_add() {
        let t = Timespan::new(1, 1);
        assert_eq!(t + t, Timespan::new(2, 1));
        assert_eq!(t + -t, Timespan::new(0, 1));
        assert_eq!(t + Timespan::new(1, 2), Timespan::new(3, 2));
        assert_eq!(Timespan::new(14, 13) + Timespan::new(18, 17),
            Timespan::new(472, 221));

        let t = Timespan::new(1, 4);
        assert_eq!(Timespan::ZERO + t, t);
        assert_eq!(t + Timespan::new(3, 4), Timespan::new(1, 1));
    }

    #[test]
    fn test_add_overflow() {
        assert_eq!(Timespan::new(20, 19) + Timespan::new(24, 23), Timespan::new(535, 255))
    }

    #[test]
    fn test_sub() {
        let t = Timespan::new(1, 1);
        assert_eq!(t - t, Timespan::ZERO);
        assert_eq!(t - Timespan::new(2, 1), Timespan::new(-1, 1));
    }

    #[test]
    fn test_mul() {
        let t = Timespan::new(1, 1);
        assert_eq!(t * t, t);
        assert_eq!(t * Timespan::ZERO, Timespan::ZERO);
        assert_eq!(Timespan::new(7, 6) * Timespan::new(4, 3), Timespan::new(14, 9));
        assert_eq!(Timespan::new(14, 13) * Timespan::new(18, 17),
            Timespan::new(252, 221));
    }

    #[test]
    fn test_mul_overflow() {
        assert_eq!(Timespan::new(20, 19) * Timespan::new(24, 23), Timespan::new(280, 255))
    }
}