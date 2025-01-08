use std::{cmp::Ordering, ops};

use gcd::Gcd;
use serde::{Deserialize, Serialize};

/// Represents a time value as a beat fraction.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Timespan {
    n: i32,
    d: u8,
}

impl Timespan {
    pub const ZERO: Timespan = Timespan { n: 0, d: 1 };

    pub fn new(n: i32, d: u8) -> Self {
        let gcd = (n.abs() as u32).gcd(d as u32);
        Self {
            n: n / gcd as i32,
            d: d / gcd as u8,
        }
    }

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

    pub fn num(&self) -> i32 {
        self.n
    }

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
        self.as_f64().partial_cmp(&other.as_f64())
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
        Self {
            n: -self.n,
            d: self.d,
        }
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
        let gcd = (n.abs() as u64).gcd(d as u64);
        let d = d / gcd as u16;

        if d <= u8::MAX as u16 {
            let n = n as i32 / gcd as i32;
            Self::new(n, d as u8)
        } else {
            Self::approximate(self.as_f64() * rhs.as_f64())
        }
    }
}

impl Into::<f64> for Timespan {
    fn into(self) -> f64 {
        self.n as f64 / self.d as f64
    }
}

impl Into::<f32> for Timespan {
    fn into(self) -> f32 {
        self.n as f32 / self.d as f32
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