use std::fmt;

use crate::{Close, High, Low, Next, Reset, Volume};
#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

/// Volume Weighted Average Price (VWAP).
///
/// The average price a security has traded at, weighted by volume. Unlike a
/// moving average it has no period: it accumulates from an *anchor* and is
/// reset when a new session begins. Because it accumulates indefinitely, a VWAP
/// that is never reset converges to a constant and stops carrying information —
/// callers are expected to call [`Reset::reset`] at each session boundary.
///
/// This indicator deliberately has no notion of time. It cannot know when a
/// session starts, so anchoring is the caller's responsibility.
///
/// # Formula
///
/// ```text
///          Σ (price_i × volume_i)
/// VWAP =  ------------------------
///              Σ volume_i
/// ```
///
/// Two ways to supply the numerator:
///
/// * [`Next<&T>`] uses the typical price `(high + low + close) / 3` as an
///   estimate of the bar's average traded price. This is the conventional
///   approximation used when only OHLCV is available.
/// * [`VolumeWeightedAveragePrice::next_turnover`] takes the bar's actual
///   traded turnover directly. When a feed reports turnover — Binance's
///   `quote asset volume` is exactly `Σ(price × qty)` over the bar's trades —
///   this is the true VWAP rather than an estimate, and should be preferred.
///
/// Bars with zero or negative volume contribute nothing and leave the running
/// value unchanged, so an untraded bar cannot drag the average.
///
/// # Example
///
/// ```
/// use ta::indicators::VolumeWeightedAveragePrice;
/// use ta::{Next, DataItem};
///
/// let mut vwap = VolumeWeightedAveragePrice::new();
///
/// let di = DataItem::builder()
///             .high(3.0)
///             .low(1.0)
///             .close(2.0)
///             .open(1.5)
///             .volume(1000.0)
///             .build().unwrap();
///
/// // typical price = (3 + 1 + 2) / 3 = 2
/// assert_eq!(vwap.next(&di), 2.0);
/// ```
///
/// # Links
///
/// * [Volume-weighted average price, Wikipedia](https://en.wikipedia.org/wiki/Volume-weighted_average_price)
#[doc(alias = "VWAP")]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Debug, Clone)]
pub struct VolumeWeightedAveragePrice {
    cumulative_turnover: f64,
    cumulative_volume: f64,
    value: f64,
}

impl VolumeWeightedAveragePrice {
    pub fn new() -> Self {
        Self {
            cumulative_turnover: 0.0,
            cumulative_volume: 0.0,
            value: 0.0,
        }
    }

    /// Accumulate a bar from its actual traded turnover.
    ///
    /// `turnover` is `Σ(price × qty)` over the bar's trades, in quote currency;
    /// `volume` is the matching base-asset quantity. Prefer this over
    /// [`Next<&T>`] whenever the feed reports turnover, because it is the true
    /// volume-weighted price rather than a typical-price estimate.
    pub fn next_turnover(&mut self, turnover: f64, volume: f64) -> f64 {
        if volume > 0.0 && turnover.is_finite() && volume.is_finite() {
            self.cumulative_turnover += turnover;
            self.cumulative_volume += volume;
        }
        if self.cumulative_volume > 0.0 {
            self.value = self.cumulative_turnover / self.cumulative_volume;
        }
        self.value
    }

    /// The current VWAP, without accumulating anything.
    ///
    /// Zero before the first bar with volume has been seen.
    pub fn value(&self) -> f64 {
        self.value
    }

    /// Volume accumulated since the last reset.
    pub fn cumulative_volume(&self) -> f64 {
        self.cumulative_volume
    }
}

impl<T: High + Low + Close + Volume> Next<&T> for VolumeWeightedAveragePrice {
    type Output = f64;

    fn next(&mut self, input: &T) -> f64 {
        let typical_price = (input.high() + input.low() + input.close()) / 3.0;
        self.next_turnover(typical_price * input.volume(), input.volume())
    }
}

impl Default for VolumeWeightedAveragePrice {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for VolumeWeightedAveragePrice {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "VWAP")
    }
}

impl Reset for VolumeWeightedAveragePrice {
    fn reset(&mut self) {
        self.cumulative_turnover = 0.0;
        self.cumulative_volume = 0.0;
        self.value = 0.0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_helper::*;

    #[test]
    fn test_next_bar_uses_typical_price() {
        let mut vwap = VolumeWeightedAveragePrice::new();

        // typical = (3 + 1 + 2) / 3 = 2, volume 1000 -> 2000 / 1000 = 2
        let bar1 = Bar::new().high(3).low(1).close(2).volume(1000.0);
        assert_eq!(vwap.next(&bar1), 2.0);

        // typical = (6 + 3 + 3) / 3 = 4, volume 1000
        // -> (2000 + 4000) / 2000 = 3
        let bar2 = Bar::new().high(6).low(3).close(3).volume(1000.0);
        assert_eq!(vwap.next(&bar2), 3.0);
    }

    #[test]
    fn test_weights_by_volume_not_by_bar_count() {
        let mut vwap = VolumeWeightedAveragePrice::new();

        // A heavy bar at 10 and a light bar at 20 must land near 10, not at 15.
        vwap.next(&Bar::new().high(10).low(10).close(10).volume(9000.0));
        let value = vwap.next(&Bar::new().high(20).low(20).close(20).volume(1000.0));
        assert_eq!(value, 11.0);
    }

    #[test]
    fn test_next_turnover_is_exact() {
        let mut vwap = VolumeWeightedAveragePrice::new();

        // 100 units traded for 1_500 quote -> average price 15, which no
        // combination of high/low/close would produce by itself.
        assert_eq!(vwap.next_turnover(1_500.0, 100.0), 15.0);
        // another 100 units for 2_500 -> 4_000 / 200 = 20
        assert_eq!(vwap.next_turnover(2_500.0, 100.0), 20.0);
    }

    #[test]
    fn test_zero_volume_bar_is_ignored() {
        let mut vwap = VolumeWeightedAveragePrice::new();

        assert_eq!(vwap.next(&Bar::new().high(2).low(2).close(2).volume(500.0)), 2.0);
        // An untraded bar must not move the average, and must not divide by zero.
        assert_eq!(vwap.next(&Bar::new().high(9).low(9).close(9).volume(0.0)), 2.0);
        assert_eq!(vwap.cumulative_volume(), 500.0);
    }

    #[test]
    fn test_no_volume_at_all_stays_zero() {
        let mut vwap = VolumeWeightedAveragePrice::new();
        assert_eq!(vwap.next(&Bar::new().high(9).low(9).close(9).volume(0.0)), 0.0);
        assert_eq!(vwap.value(), 0.0);
    }

    #[test]
    fn test_reset() {
        let mut vwap = VolumeWeightedAveragePrice::new();

        let bar1 = Bar::new().high(3).low(1).close(2).volume(1000.0);
        let bar2 = Bar::new().high(6).low(3).close(3).volume(1000.0);

        assert_eq!(vwap.next(&bar1), 2.0);
        assert_eq!(vwap.next(&bar2), 3.0);

        // Reset is what makes VWAP a session indicator: afterwards it must
        // behave exactly as a freshly constructed one.
        vwap.reset();
        assert_eq!(vwap.value(), 0.0);
        assert_eq!(vwap.cumulative_volume(), 0.0);
        assert_eq!(vwap.next(&bar1), 2.0);
        assert_eq!(vwap.next(&bar2), 3.0);
    }

    /// VWAP is a session accumulator, so its state must survive a process
    /// restart: a deserialised instance has to continue the session, not
    /// restart it. Round-tripping must preserve the running sums, not merely
    /// the last emitted value.
    #[cfg(feature = "serde")]
    #[test]
    fn test_serde_round_trip_resumes_accumulation() {
        let mut live = VolumeWeightedAveragePrice::new();
        live.next_turnover(1_000.0, 100.0); // 10.0
        live.next_turnover(3_000.0, 100.0); // 20.0
        assert_eq!(live.value(), 20.0);

        let bytes = bincode::serialize(&live).expect("serialises");
        let mut restored: VolumeWeightedAveragePrice =
            bincode::deserialize(&bytes).expect("deserialises");

        // The running sums must come back, not just the last value.
        assert_eq!(restored.value(), live.value());
        assert_eq!(restored.cumulative_volume(), 200.0);

        // Feeding both the same next bar must keep them identical — this is
        // what proves the accumulator resumed rather than started fresh.
        let live_next = live.next_turnover(5_000.0, 100.0);
        let restored_next = restored.next_turnover(5_000.0, 100.0);
        assert_eq!(restored_next, live_next);
        assert_eq!(restored_next, 30.0, "(1000+3000+5000)/300");

        // A fresh instance would have reported 50.0 for that same bar.
        let mut fresh = VolumeWeightedAveragePrice::new();
        assert_eq!(fresh.next_turnover(5_000.0, 100.0), 50.0);
    }

    #[test]
    fn test_default() {
        VolumeWeightedAveragePrice::default();
    }

    #[test]
    fn test_display() {
        let vwap = VolumeWeightedAveragePrice::new();
        assert_eq!(format!("{}", vwap), "VWAP");
    }
}
