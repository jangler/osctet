use std::ops;

use gcd::Gcd;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Beat {
    n: i32,
    d: u8,
}

impl Beat {
    fn new(n: i32, d: u8) -> Self {
        let gcd = (n.abs() as u32).gcd(d as u32);
        Self {
            n: n / gcd as i32,
            d: d / gcd as u8,
        }
    }
}

impl ops::Neg for Beat {
    type Output = Self;

    fn neg(self) -> Self::Output {
        Self {
            n: -self.n,
            d: self.d,
        }
    }
}

impl ops::Add<Self> for Beat {
    type Output = Self;

    fn add(self, rhs: Self) -> Self {
        let gcd = self.d.gcd(rhs.d);
        let d = (self.d as u16 * rhs.d as u16) / gcd as u16;
        if d <= u8::MAX as u16 {
            let n = (self.n * (self.d / gcd) as i32)
                + (rhs.n * (rhs.d / gcd) as i32);
            Self { n, d: d as u8 }
        } else {
            let d = u8::MAX;
            let a: f64 = self.into();
            let b: f64 = rhs.into();
            let n = ((a + b) * d as f64).round() as i32;
            Self::new(n, d)
        }
    }
}

impl Into::<f64> for Beat {
    fn into(self) -> f64 {
        self.n as f64 / self.d as f64
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new() {
        assert_eq!(Beat::new(1, 1), Beat { n: 1, d: 1 });
        assert_eq!(Beat::new(2, 2), Beat { n: 1, d: 1 });
        assert_eq!(Beat::new(10, 6), Beat { n: 5, d: 3 });
    }

    #[test]
    fn test_add() {
        let a = Beat::new(1, 1);
        assert_eq!(a + a, Beat::new(2, 1));
        assert_eq!(a + -a, Beat::new(0, 1));
        assert_eq!(a + Beat::new(1, 2), Beat::new(3, 2));
    }

    #[test]
    fn test_add_overflow() {
        assert_eq!(Beat::new(20, 19) + Beat::new(24, 23), Beat::new(535, 255))
    }
}