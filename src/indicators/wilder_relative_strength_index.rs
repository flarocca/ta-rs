use std::fmt;

use crate::errors::Result;
use crate::indicators::WilderSmoothing;
use crate::{Close, Next, Period, Reset};
#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

/// Wilder's Relative Strength Index.
///
/// A variant of RSI that uses Wilder's smoothing (RMA, α = 1/period) instead of
/// the standard EMA (α = 2/(period+1)).  This matches TradingView's built-in RSI
/// and J. Welles Wilder's original definition.
///
/// The oscillator returns output in the range of 0..100.
///
/// # Formula
///
/// RSI<sub>t</sub> = RMA<sub>Ut</sub> * 100 / (RMA<sub>Ut</sub> + RMA<sub>Dt</sub>)
///
/// Where:
///
/// * RSI<sub>t</sub> - value of RSI indicator in a moment of time _t_
/// * RMA<sub>Ut</sub> - value of [Wilder's smoothing](struct.WilderSmoothing.html) of up periods in a moment of time _t_
/// * RMA<sub>Dt</sub> - value of [Wilder's smoothing](struct.WilderSmoothing.html) of down periods in a moment of time _t_
///
/// If current period has value higher than previous period, than:
///
/// U = p<sub>t</sub> - p<sub>t-1</sub>
///
/// D = 0
///
/// Otherwise:
///
/// U = 0
///
/// D = p<sub>t-1</sub> - p<sub>t</sub>
///
/// # Parameters
///
/// * _period_ - number of periods (integer greater than 0). Default value is 14.
///
/// # Example
///
/// ```
/// use ta::indicators::WilderRelativeStrengthIndex;
/// use ta::Next;
///
/// let mut rsi = WilderRelativeStrengthIndex::new(3).unwrap();
/// assert_eq!(rsi.next(10.0), 50.0);
/// assert_eq!(rsi.next(10.5).round(), 78.0);
/// assert_eq!(rsi.next(10.0).round(), 42.0);
/// assert_eq!(rsi.next(9.5).round(), 25.0);
/// ```
///
/// # Links
/// * [Relative strength index (Wikipedia)](https://en.wikipedia.org/wiki/Relative_strength_index)
/// * [RSI (Investopedia)](http://www.investopedia.com/terms/r/rsi.asp)
///
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Debug, Clone)]
pub struct WilderRelativeStrengthIndex {
    period: usize,
    up_rma: WilderSmoothing,
    down_rma: WilderSmoothing,
    prev_val: f64,
    is_new: bool,
}

impl WilderRelativeStrengthIndex {
    pub fn new(period: usize) -> Result<Self> {
        Ok(Self {
            period,
            up_rma: WilderSmoothing::new(period)?,
            down_rma: WilderSmoothing::new(period)?,
            prev_val: 0.0,
            is_new: true,
        })
    }
}

impl Period for WilderRelativeStrengthIndex {
    fn period(&self) -> usize {
        self.period
    }
}

impl Next<f64> for WilderRelativeStrengthIndex {
    type Output = f64;

    fn next(&mut self, input: f64) -> Self::Output {
        let mut up = 0.0;
        let mut down = 0.0;

        if self.is_new {
            self.is_new = false;
            // Initialize with some small seed numbers to avoid division by zero
            up = 0.1;
            down = 0.1;
        } else {
            if input > self.prev_val {
                up = input - self.prev_val;
            } else {
                down = self.prev_val - input;
            }
        }

        self.prev_val = input;
        let up_rma = self.up_rma.next(up);
        let down_rma = self.down_rma.next(down);
        100.0 * up_rma / (up_rma + down_rma)
    }
}

impl<T: Close> Next<&T> for WilderRelativeStrengthIndex {
    type Output = f64;

    fn next(&mut self, input: &T) -> Self::Output {
        self.next(input.close())
    }
}

impl Reset for WilderRelativeStrengthIndex {
    fn reset(&mut self) {
        self.is_new = true;
        self.prev_val = 0.0;
        self.up_rma.reset();
        self.down_rma.reset();
    }
}

impl Default for WilderRelativeStrengthIndex {
    fn default() -> Self {
        Self::new(14).unwrap()
    }
}

impl fmt::Display for WilderRelativeStrengthIndex {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "WRSI({})", self.period)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_helper::*;

    test_indicator!(WilderRelativeStrengthIndex);

    #[test]
    fn test_new() {
        assert!(WilderRelativeStrengthIndex::new(0).is_err());
        assert!(WilderRelativeStrengthIndex::new(1).is_ok());
    }

    #[test]
    fn test_next() {
        let mut rsi = WilderRelativeStrengthIndex::new(3).unwrap();
        assert_eq!(rsi.next(10.0), 50.0);
        assert_eq!(rsi.next(10.5).round(), 78.0);
        assert_eq!(rsi.next(10.0).round(), 42.0);
        assert_eq!(rsi.next(9.5).round(), 25.0);
    }

    #[test]
    fn test_next_bar() {
        let mut rsi = WilderRelativeStrengthIndex::new(3).unwrap();
        let bar1 = Bar::new().close(10);
        let bar2 = Bar::new().close(10.5);
        assert_eq!(rsi.next(&bar1), 50.0);
        assert_eq!(rsi.next(&bar2).round(), 78.0);
    }

    #[test]
    fn test_reset() {
        let mut rsi = WilderRelativeStrengthIndex::new(3).unwrap();
        assert_eq!(rsi.next(10.0), 50.0);
        assert_eq!(rsi.next(10.5).round(), 78.0);

        rsi.reset();
        assert_eq!(rsi.next(10.0).round(), 50.0);
        assert_eq!(rsi.next(10.5).round(), 78.0);
    }

    #[test]
    fn test_default() {
        WilderRelativeStrengthIndex::default();
    }

    #[test]
    fn test_display() {
        let rsi = WilderRelativeStrengthIndex::new(16).unwrap();
        assert_eq!(format!("{}", rsi), "WRSI(16)");
    }

    #[test]
    fn test_k_matches_wilder() {
        // For period=14, Wilder's k=1/14
        // Verify RSI converges differently than EMA-based RSI
        let mut rsi = WilderRelativeStrengthIndex::new(14).unwrap();
        rsi.next(100.0); // seed
        let v = rsi.next(114.0); // big up move
        // With RMA: up_rma = 0.1 + (14.0 - 0.1)/14 = 1.0928..
        //           down_rma = 0.1 + (0 - 0.1)/14 = 0.09285..
        // RSI = 100 * 1.0928 / (1.0928 + 0.09285) = 92.17
        assert!((v - 92.17).abs() < 0.1, "got {v}");
    }
}
