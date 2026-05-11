use std::fmt;

use crate::errors::{Result, TaError};
use crate::{Close, Next, Period, Reset};
#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

/// Wilder's smoothing average, also known as the Running Moving Average (RMA).
///
/// This is the smoothing method used by J. Welles Wilder in his original RSI, ATR,
/// and ADX indicators. It is equivalent to an exponential moving average with
/// smoothing factor `α = 1/period` (instead of the standard EMA's `α = 2/(period+1)`).
///
/// # Formula
///
/// RMA<sub>t</sub> = α × input + (1 - α) × RMA<sub>t-1</sub>
///
/// Where:
///
/// * α = 1 / period
/// * The first value is seeded directly: RMA<sub>0</sub> = input<sub>0</sub>
///
/// # Parameters
///
/// * _period_ - number of periods (integer greater than 0)
///
/// # Example
///
/// ```
/// use ta::indicators::WilderSmoothing;
/// use ta::Next;
///
/// let mut rma = WilderSmoothing::new(3).unwrap();
/// assert_eq!(rma.next(2.0), 2.0);
/// let v = rma.next(5.0);
/// assert!((v - 3.0).abs() < 1e-10);  // 2.0 + (5.0 - 2.0)/3 = 3.0
/// ```
///
/// # Links
///
/// * [Wilder's Smoothing (Wikipedia)](https://en.wikipedia.org/wiki/Moving_average#Modified_moving_average)
///
#[doc(alias = "RMA")]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Debug, Clone)]
pub struct WilderSmoothing {
    period: usize,
    k: f64,
    current: f64,
    is_new: bool,
}

impl WilderSmoothing {
    pub fn new(period: usize) -> Result<Self> {
        match period {
            0 => Err(TaError::InvalidParameter),
            _ => Ok(Self {
                period,
                k: 1.0 / period as f64,
                current: 0.0,
                is_new: true,
            }),
        }
    }
}

impl Period for WilderSmoothing {
    fn period(&self) -> usize {
        self.period
    }
}

impl Next<f64> for WilderSmoothing {
    type Output = f64;

    fn next(&mut self, input: f64) -> Self::Output {
        if self.is_new {
            self.is_new = false;
            self.current = input;
        } else {
            self.current = self.k * input + (1.0 - self.k) * self.current;
        }
        self.current
    }
}

impl<T: Close> Next<&T> for WilderSmoothing {
    type Output = f64;

    fn next(&mut self, input: &T) -> Self::Output {
        self.next(input.close())
    }
}

impl Reset for WilderSmoothing {
    fn reset(&mut self) {
        self.current = 0.0;
        self.is_new = true;
    }
}

impl Default for WilderSmoothing {
    fn default() -> Self {
        Self::new(14).unwrap()
    }
}

impl fmt::Display for WilderSmoothing {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "RMA({})", self.period)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_helper::*;

    test_indicator!(WilderSmoothing);

    #[test]
    fn test_new() {
        assert!(WilderSmoothing::new(0).is_err());
        assert!(WilderSmoothing::new(1).is_ok());
    }

    #[test]
    fn test_next() {
        let mut rma = WilderSmoothing::new(3).unwrap();
        // k = 1/3
        assert_eq!(rma.next(2.0), 2.0); // seed
        // 2.0 + (5.0 - 2.0) * (1/3) = 3.0
        let v2 = rma.next(5.0);
        assert!((v2 - 3.0).abs() < 1e-10, "got {v2}");
        // 3.0 + (1.0 - 3.0) * (1/3) = 2.333...
        let v3 = rma.next(1.0);
        assert!((v3 - 7.0 / 3.0).abs() < 1e-10, "got {v3}");
    }

    #[test]
    fn test_next_bar() {
        let mut rma = WilderSmoothing::new(3).unwrap();
        let bar1 = Bar::new().close(2);
        let bar2 = Bar::new().close(5);
        assert_eq!(rma.next(&bar1), 2.0);
        let v2 = rma.next(&bar2);
        assert!((v2 - 3.0).abs() < 1e-10);
    }

    #[test]
    fn test_reset() {
        let mut rma = WilderSmoothing::new(5).unwrap();

        assert_eq!(rma.next(4.0), 4.0);
        rma.next(10.0);
        rma.next(15.0);
        rma.next(20.0);
        assert_ne!(rma.next(4.0), 4.0);

        rma.reset();
        assert_eq!(rma.next(4.0), 4.0);
    }

    #[test]
    fn test_default() {
        WilderSmoothing::default();
    }

    #[test]
    fn test_display() {
        let rma = WilderSmoothing::new(7).unwrap();
        assert_eq!(format!("{}", rma), "RMA(7)");
    }

    #[test]
    fn test_k_is_one_over_period() {
        // For period=14, k should be 1/14
        let mut rma = WilderSmoothing::new(14).unwrap();
        rma.next(100.0); // seed
        let v = rma.next(114.0);
        // 100 + (114 - 100) * (1/14) = 101.0
        assert!((v - 101.0).abs() < 1e-10, "got {v}");
    }
}
