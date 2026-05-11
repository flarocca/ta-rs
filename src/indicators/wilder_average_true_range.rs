use std::fmt;

use crate::errors::Result;
use crate::indicators::{TrueRange, WilderSmoothing};
use crate::{Close, High, Low, Next, Period, Reset};

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

/// Wilder's Average True Range (ATR).
///
/// A variant of ATR that uses Wilder's smoothing (RMA, α = 1/period) instead of
/// the standard EMA (α = 2/(period+1)). This matches TradingView's built-in ATR
/// and J. Welles Wilder's original definition.
///
/// # Formula
///
/// ATR(period)<sub>t</sub> = RMA(period) of TR<sub>t</sub>
///
/// Where:
///
/// * _RMA(period)_ - [Wilder's smoothing](struct.WilderSmoothing.html) with smoothing period
/// * _TR<sub>t</sub>_ - [true range](struct.TrueRange.html) for period _t_
///
/// # Parameters
///
/// * _period_ - smoothing period (integer greater than 0)
///
/// # Example
///
/// ```
/// extern crate ta;
/// #[macro_use] extern crate assert_approx_eq;
///
/// use ta::{Next, DataItem};
/// use ta::indicators::WilderAverageTrueRange;
///
/// fn main() {
///     let mut atr = WilderAverageTrueRange::new(3).unwrap();
///
///     let bar1 = DataItem::builder()
///         .high(10.0).low(7.5).close(9.0).open(9.0).volume(1000.0)
///         .build().unwrap();
///     let bar2 = DataItem::builder()
///         .high(11.0).low(9.0).close(9.5).open(9.0).volume(1000.0)
///         .build().unwrap();
///
///     assert_approx_eq!(atr.next(&bar1), 2.5);
///     assert_approx_eq!(atr.next(&bar2), 2.3333, 1e-4);
/// }
/// ```
///
/// # Links
///
/// * [ATR (Wikipedia)](https://en.wikipedia.org/wiki/Average_true_range)
///
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Debug, Clone)]
pub struct WilderAverageTrueRange {
    true_range: TrueRange,
    rma: WilderSmoothing,
}

impl WilderAverageTrueRange {
    pub fn new(period: usize) -> Result<Self> {
        Ok(Self {
            true_range: TrueRange::new(),
            rma: WilderSmoothing::new(period)?,
        })
    }
}

impl Period for WilderAverageTrueRange {
    fn period(&self) -> usize {
        self.rma.period()
    }
}

impl Next<f64> for WilderAverageTrueRange {
    type Output = f64;

    fn next(&mut self, input: f64) -> Self::Output {
        self.rma.next(self.true_range.next(input))
    }
}

impl<T: High + Low + Close> Next<&T> for WilderAverageTrueRange {
    type Output = f64;

    fn next(&mut self, input: &T) -> Self::Output {
        self.rma.next(self.true_range.next(input))
    }
}

impl Reset for WilderAverageTrueRange {
    fn reset(&mut self) {
        self.true_range.reset();
        self.rma.reset();
    }
}

impl Default for WilderAverageTrueRange {
    fn default() -> Self {
        Self::new(14).unwrap()
    }
}

impl fmt::Display for WilderAverageTrueRange {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "WATR({})", self.rma.period())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_helper::*;

    test_indicator!(WilderAverageTrueRange);

    #[test]
    fn test_new() {
        assert!(WilderAverageTrueRange::new(0).is_err());
        assert!(WilderAverageTrueRange::new(1).is_ok());
    }

    #[test]
    fn test_next() {
        let mut atr = WilderAverageTrueRange::new(3).unwrap();

        let bar1 = Bar::new().high(10).low(7.5).close(9);
        let bar2 = Bar::new().high(11).low(9).close(9.5);
        let bar3 = Bar::new().high(9).low(5).close(8);

        assert_eq!(atr.next(&bar1), 2.5);
        // RMA: 2.5 + (2.0 - 2.5)/3 = 2.3333...
        let v2 = atr.next(&bar2);
        assert!((v2 - 7.0 / 3.0).abs() < 1e-10, "got {v2}");
        // RMA: 2.3333 + (4.5 - 2.3333)/3 = 3.0555...
        let v3 = atr.next(&bar3);
        assert!((v3 - 55.0 / 18.0).abs() < 1e-10, "got {v3}");
    }

    #[test]
    fn test_reset() {
        let mut atr = WilderAverageTrueRange::new(9).unwrap();

        let bar1 = Bar::new().high(10).low(7.5).close(9);
        let bar2 = Bar::new().high(11).low(9).close(9.5);

        atr.next(&bar1);
        atr.next(&bar2);

        atr.reset();
        let bar3 = Bar::new().high(60).low(15).close(51);
        assert_eq!(atr.next(&bar3), 45.0);
    }

    #[test]
    fn test_default() {
        WilderAverageTrueRange::default();
    }

    #[test]
    fn test_display() {
        let indicator = WilderAverageTrueRange::new(8).unwrap();
        assert_eq!(format!("{}", indicator), "WATR(8)");
    }
}
