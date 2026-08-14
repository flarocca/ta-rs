use crate::errors::{Result, TaError};
use crate::indicators::TrueRange;
use crate::{Close, High, Low, Next, Period, Reset};
use std::fmt;

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct DirectionalMovementIndexOutput {
    pub plus_di: f64,
    pub minus_di: f64,
    pub adx: f64,
}

/// Directional Movement Index (DMI) with Average Directional Index (ADX).
///
/// The Directional Movement Index, introduced by J. Welles Wilder, is a trend analysis
/// framework designed to quantify:
///
/// * **Directional bias** — whether upward or downward movement dominates
/// * **Trend strength** — how strong that directional movement is
///
/// The indicator produces three values:
///
/// * **+DI** (Positive Directional Indicator) — measures upward directional pressure
/// * **−DI** (Negative Directional Indicator) — measures downward directional pressure
/// * **ADX** (Average Directional Index) — measures trend strength independent of direction
///
/// DMI is based on price *expansion* (changes in highs and lows), not closing prices.
/// All components are normalized by volatility using the True Range and smoothed
/// using Wilder’s smoothing method.
///
/// # Formula
///
/// Let:
///
/// * _H<sub>t</sub>_ = current high
/// * _L<sub>t</sub>_ = current low
/// * _C<sub>t−1</sub>_ = previous close
///
/// **Directional Movement**
///
/// ```text
/// UpMove   = H_t − H_{t−1}
/// DownMove = L_{t−1} − L_t
///
/// +DM = UpMove   if UpMove > DownMove and UpMove > 0, else 0
/// −DM = DownMove if DownMove > UpMove and DownMove > 0, else 0
/// ```
///
/// Only one of +DM or −DM can be non-zero for a given period.
///
/// **True Range**
///
/// The True Range (TR) is used to normalize directional movement by volatility:
///
/// ```text
/// TR = max(
///     H_t − L_t,
///     |H_t − C_{t−1}|,
///     |L_t − C_{t−1}|
/// )
/// ```
///
/// This implementation reuses the existing `TrueRange` indicator.
///
/// **Wilder Smoothing**
///
/// Directional Movement and True Range are smoothed using Wilder’s recursive smoothing:
///
/// ```text
/// Smoothed_t = Smoothed_{t−1} − (Smoothed_{t−1} / period) + Value_t
/// ```
///
/// **Directional Indicators**
///
/// ```text
/// +DI = 100 × (Smoothed +DM / Smoothed TR)
/// −DI = 100 × (Smoothed −DM / Smoothed TR)
/// ```
///
/// **Directional Index (DX)**
///
/// ```text
/// DX = 100 × |+DI − −DI| / (+DI + −DI)
/// ```
///
/// **Average Directional Index (ADX)**
///
/// ADX is the Wilder-smoothed DX and represents trend strength only
/// (it does not indicate trend direction).
///
/// # Interpretation
///
/// * **+DI > −DI** indicates dominant upward directional pressure
/// * **−DI > +DI** indicates dominant downward directional pressure
/// * **Rising ADX** indicates increasing trend strength
/// * **Falling ADX** indicates weakening trend strength
///
/// ADX can rise in both uptrends and downtrends.
///
/// # Parameters
///
/// * _period_ — smoothing period (integer greater than 0, typically 14)
///
/// # Example
///
/// ```
/// use ta::indicators::DirectionalMovementIndex;
/// use ta::{Next, DataItem};
///
/// let mut dmi = DirectionalMovementIndex::new(3).unwrap();
///
/// let di1 = DataItem::builder()
///     .high(10.0)
///     .low(9.0)
///     .close(9.5)
///     .open(9.2)
///     .volume(1.0)
///     .build().unwrap();
///
/// let di2 = DataItem::builder()
///     .high(11.0)
///     .low(9.8)
///     .close(10.6)
///     .open(10.0)
///     .volume(1.0)
///     .build().unwrap();
///
/// let _out1 = dmi.next(&di1);
/// let out2 = dmi.next(&di2);
///
/// assert!(out2.plus_di.is_finite());
/// assert!(out2.minus_di.is_finite());
/// assert!(out2.adx.is_finite());
/// ```
///
/// # Links
/// * [Directional Movement Index, Wikipedia](https://en.wikipedia.org/wiki/Average_directional_movement_index)
#[doc(alias = "DMI")]
#[doc(alias = "ADX")]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Debug, Clone)]
pub struct DirectionalMovementIndex {
    period: usize,

    tr: TrueRange,

    prev_high: f64,
    prev_low: f64,
    has_prev: bool,

    // counts how many DM/TR updates we've processed (not bars)
    count: usize,

    // Wilder smoothed values (after initialization)
    sm_tr: f64,
    sm_plus_dm: f64,
    sm_minus_dm: f64,
    sm_initialized: bool,

    // ADX warmup and smoothing
    dx_sum: f64, // accumulate DX until first ADX (post-warmup only)
    /// Number of DX values accumulated into `dx_sum` since
    /// `sm_initialized` became true. Once this reaches `period`,
    /// `adx` is initialized as `dx_sum / period` and Wilder smoothing
    /// takes over.
    ///
    /// Tagged `serde(default)` so persisted state written before this
    /// field existed can be deserialized without losing the rest of
    /// the indicator state.
    #[cfg_attr(feature = "serde", serde(default))]
    dx_count: usize,
    adx: f64,
    adx_initialized: bool,
}

impl DirectionalMovementIndex {
    pub fn new(period: usize) -> Result<Self> {
        match period {
            0 => Err(TaError::InvalidParameter),
            _ => Ok(Self {
                period,
                tr: TrueRange::new(),
                prev_high: 0.0,
                prev_low: 0.0,
                has_prev: false,
                count: 0,
                sm_tr: 0.0,
                sm_plus_dm: 0.0,
                sm_minus_dm: 0.0,
                sm_initialized: false,
                dx_sum: 0.0,
                dx_count: 0,
                adx: 0.0,
                adx_initialized: false,
            }),
        }
    }
}

impl Period for DirectionalMovementIndex {
    fn period(&self) -> usize {
        self.period
    }
}

impl<T: High + Low + Close> Next<&T> for DirectionalMovementIndex {
    type Output = DirectionalMovementIndexOutput;

    fn next(&mut self, input: &T) -> Self::Output {
        let tr = self.tr.next(input);

        let high = input.high();
        let low = input.low();

        if !self.has_prev {
            self.prev_high = high;
            self.prev_low = low;
            self.has_prev = true;

            return DirectionalMovementIndexOutput::default();
        }

        let up_move = high - self.prev_high;
        let down_move = self.prev_low - low;

        let plus_dm = if up_move > down_move && up_move > 0.0 {
            up_move
        } else {
            0.0
        };
        let minus_dm = if down_move > up_move && down_move > 0.0 {
            down_move
        } else {
            0.0
        };

        self.prev_high = high;
        self.prev_low = low;

        // Wilder smoothing for TR, +DM, -DM
        if !self.sm_initialized {
            // accumulate the first `period` updates
            self.sm_tr += tr;
            self.sm_plus_dm += plus_dm;
            self.sm_minus_dm += minus_dm;

            self.count += 1;

            if self.count >= self.period {
                self.sm_initialized = true;
            }
        } else {
            let p = self.period as f64;
            self.sm_tr = self.sm_tr - (self.sm_tr / p) + tr;
            self.sm_plus_dm = self.sm_plus_dm - (self.sm_plus_dm / p) + plus_dm;
            self.sm_minus_dm = self.sm_minus_dm - (self.sm_minus_dm / p) + minus_dm;
        }

        let plus_di = if self.sm_tr > 0.0 {
            100.0 * (self.sm_plus_dm / self.sm_tr)
        } else {
            0.0
        };
        let minus_di = if self.sm_tr > 0.0 {
            100.0 * (self.sm_minus_dm / self.sm_tr)
        } else {
            0.0
        };

        let denom = plus_di + minus_di;
        let dx = if denom > 0.0 {
            100.0 * ((plus_di - minus_di).abs() / denom)
        } else {
            0.0
        };

        // ADX
        //
        // Wilder's ADX is the running smoothed average of DX, but it only
        // starts once smoothed DM/TR are themselves initialized — DX values
        // produced during the DM/TR warmup come from partial sums and would
        // bias the initial ADX. So:
        //
        //   1. While `sm_initialized` is false, skip the DX accumulator
        //      entirely and emit 0.0 (no meaningful ADX exists yet).
        //   2. Once `sm_initialized` becomes true, accumulate the next
        //      `period` DX values into `dx_sum` / `dx_count`. During this
        //      second warmup we emit the running average so callers see a
        //      non-zero value that converges toward the eventual ADX.
        //   3. At `dx_count == period`, freeze the initial ADX as
        //      `dx_sum / period` and switch to Wilder smoothing on every
        //      subsequent call.
        //
        // Historical bug: the prior version of this branch incremented
        // `self.count` only inside the DM/TR warmup branch, so once warmup
        // finished `count` froze at `period` and the `dx_count >= period`
        // gate never fired. ADX stayed in the "warmup" branch forever,
        // returning `dx_sum / period` with `dx_sum` growing unboundedly —
        // producing values in the thousands on long series.
        let adx = if !self.adx_initialized {
            if self.sm_initialized {
                self.dx_sum += dx;
                self.dx_count += 1;

                if self.dx_count >= self.period {
                    self.adx = self.dx_sum / (self.period as f64);
                    self.adx_initialized = true;
                }

                // Pre-init warmup output: running average of the DX values
                // collected so far. Caller can use this as a coarse hint
                // but should treat values before bar `2 * period` as
                // un-converged.
                self.dx_sum / (self.dx_count as f64)
            } else {
                0.0
            }
        } else {
            let p = self.period as f64;
            self.adx = (self.adx * (p - 1.0) + dx) / p;
            self.adx
        };

        DirectionalMovementIndexOutput {
            plus_di,
            minus_di,
            adx,
        }
    }
}

impl Reset for DirectionalMovementIndex {
    fn reset(&mut self) {
        self.tr.reset();

        self.prev_high = 0.0;
        self.prev_low = 0.0;
        self.has_prev = false;

        self.count = 0;

        self.sm_tr = 0.0;
        self.sm_plus_dm = 0.0;
        self.sm_minus_dm = 0.0;
        self.sm_initialized = false;

        self.dx_sum = 0.0;
        self.dx_count = 0;
        self.adx = 0.0;
        self.adx_initialized = false;
    }
}

impl Default for DirectionalMovementIndex {
    fn default() -> Self {
        Self::new(14).unwrap()
    }
}

impl fmt::Display for DirectionalMovementIndex {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "DMI({})", self.period)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_helper::*;

    fn bar(h: f64, l: f64, c: f64) -> Bar {
        Bar::new().high(h).low(l).close(c)
    }

    fn assert_close(a: f64, b: f64, eps: f64) {
        assert!(
            (a - b).abs() <= eps,
            "expected |{} - {}| <= {}, got {}",
            a,
            b,
            eps,
            (a - b).abs()
        );
    }

    fn assert_output_validity(o: DirectionalMovementIndexOutput) {
        assert!(o.plus_di.is_finite());
        assert!(o.minus_di.is_finite());
        assert!(o.adx.is_finite());

        // DMI/ADX are non-negative by construction
        assert!(o.plus_di >= 0.0);
        assert!(o.minus_di >= 0.0);
        assert!(o.adx >= 0.0);

        // In typical data DI/ADX stay within [0, 100]
        // (Small warmup/edge cases can still remain within these bounds with this implementation.)
        assert!(o.plus_di <= 100.0);
        assert!(o.minus_di <= 100.0);
        assert!(o.adx <= 100.0);
    }

    #[test]
    fn test_new() {
        assert!(DirectionalMovementIndex::new(0).is_err());
        assert!(DirectionalMovementIndex::new(1).is_ok());
    }

    #[test]
    fn test_flat_market_is_zero() {
        let mut dmi = DirectionalMovementIndex::new(14).unwrap();
        let b = bar(10.0, 10.0, 10.0);

        let o0 = dmi.next(&b);
        assert_eq!(o0, DirectionalMovementIndexOutput::default());

        for _ in 0..50 {
            let o = dmi.next(&b);
            assert_eq!(
                o,
                DirectionalMovementIndexOutput {
                    plus_di: 0.0,
                    minus_di: 0.0,
                    adx: 0.0
                }
            );
        }
    }

    #[test]
    fn test_uptrend_plus_di_dominates() {
        let mut dmi = DirectionalMovementIndex::new(5).unwrap();

        let uptrend = vec![
            bar(10.0, 9.0, 9.5),
            bar(11.0, 9.8, 10.6),
            bar(12.0, 10.5, 11.7),
            bar(13.0, 11.2, 12.8),
            bar(14.0, 12.0, 13.9),
        ];

        let mut last = DirectionalMovementIndexOutput::default();

        for b in &uptrend {
            last = dmi.next(b);
            assert_output_validity(last);
        }

        assert!(
            last.plus_di > last.minus_di,
            "expected +DI > -DI in uptrend, got {:?}",
            last
        );
    }

    #[test]
    fn test_downtrend_minus_di_dominates() {
        let mut dmi = DirectionalMovementIndex::new(5).unwrap();

        let downtrend = vec![
            bar(13.5, 11.5, 12.0),
            bar(12.7, 10.8, 11.1),
            bar(11.8, 10.0, 10.2),
            bar(11.0, 9.2, 9.4),
            bar(10.2, 8.6, 8.9),
        ];

        let mut last = DirectionalMovementIndexOutput::default();

        for b in &downtrend {
            last = dmi.next(b);
            assert_output_validity(last);
        }

        assert!(
            last.minus_di > last.plus_di,
            "expected -DI > +DI in downtrend, got {:?}",
            last
        );
    }

    #[test]
    fn test_reset_reproducible() {
        let mut dmi = DirectionalMovementIndex::new(3).unwrap();

        let data = vec![
            bar(30.0, 28.0, 29.0),
            bar(31.0, 28.5, 30.0),
            bar(32.0, 29.0, 31.0),
            bar(31.5, 28.8, 29.5),
            bar(33.0, 29.5, 32.0),
            bar(32.0, 29.0, 30.0),
            bar(34.0, 30.0, 33.0),
        ];

        let outs1: Vec<DirectionalMovementIndexOutput> = data.iter().map(|b| dmi.next(b)).collect();

        dmi.reset();

        let outs2: Vec<DirectionalMovementIndexOutput> = data.iter().map(|b| dmi.next(b)).collect();

        let eps = 1e-12;
        for (a, b) in outs1.into_iter().zip(outs2.into_iter()) {
            assert_close(a.plus_di, b.plus_di, eps);
            assert_close(a.minus_di, b.minus_di, eps);
            assert_close(a.adx, b.adx, eps);
        }
    }

    #[test]
    fn test_default_and_display() {
        let dmi = DirectionalMovementIndex::default();
        let _ = format!("{}", dmi);
    }

    /// Regression test for the historical "ADX grows unboundedly on long
    /// series" bug. With the old code, `count` froze at `period` after
    /// warmup, the ADX-init gate never fired, and `dx_sum / period` kept
    /// climbing forever. On a 1000-bar series with period=14 the output
    /// would routinely reach the hundreds or thousands.
    ///
    /// After the fix:
    ///   * Bars 1..=period-1 produce ADX == 0 (no DI available yet).
    ///   * Bars period..=2*period-1 produce a non-zero in-warmup average
    ///     of accumulated DX values, all within [0, 100].
    ///   * From bar 2*period onward Wilder smoothing applies and ADX stays
    ///     bounded in [0, 100] forever.
    #[test]
    fn test_adx_stays_in_range_on_long_series() {
        // Deterministic synthetic OHLC series that produces non-trivial
        // directional movement (alternating trend regimes). 1000 bars is
        // long enough to expose any unbounded growth — the old code would
        // already be over 100 by ~bar 30.
        let mut dmi = DirectionalMovementIndex::new(14).unwrap();
        let mut last_close = 100.0;
        let mut max_adx = 0.0_f64;
        let mut min_adx = f64::INFINITY;

        for i in 0..1000_i32 {
            // Mix of trend + chop so DX varies across the range.
            let phase = (i as f64) * 0.05;
            let trend = (phase.sin() * 5.0) + ((phase * 0.3).cos() * 3.0);
            let close = last_close + trend;
            let high = close.max(last_close) + 0.5;
            let low = close.min(last_close) - 0.5;
            let bar = bar(high, low, close);

            let out = dmi.next(&bar);
            assert_output_validity(out);

            if i >= 28 {
                // After 2*period bars the ADX is fully initialized and must
                // stay in [0, 100] forever.
                assert!(
                    (0.0..=100.0).contains(&out.adx),
                    "bar {i}: adx={} out of range",
                    out.adx
                );
                max_adx = max_adx.max(out.adx);
                min_adx = min_adx.min(out.adx);
            }
            last_close = close;
        }

        // Sanity: on a series with real movement the ADX should be
        // non-zero somewhere — guards against a regression that pins it
        // at 0.
        assert!(
            max_adx > 1.0,
            "expected some non-trivial ADX, got max={max_adx}"
        );
        // And it must never have spiked far past 100 either.
        assert!(
            max_adx <= 100.0,
            "ADX exceeded 100 (unbounded-growth bug?): max={max_adx}"
        );
    }

    /// While `sm_initialized` is false, the indicator has no meaningful
    /// notion of directional strength. We promise ADX == 0 over that
    /// window — the fix replaces the historical "partial sum / count"
    /// noise with a clean zero so callers can reliably skip warmup.
    #[test]
    fn test_adx_is_zero_during_dm_tr_warmup() {
        let mut dmi = DirectionalMovementIndex::new(14).unwrap();
        let bars: Vec<Bar> = (0..14)
            .map(|i| bar(10.0 + i as f64, 9.0 + i as f64, 9.5 + i as f64))
            .collect();
        for (i, b) in bars.iter().enumerate() {
            let out = dmi.next(b);
            assert_eq!(
                out.adx, 0.0,
                "bar {i}: expected adx=0 during DM/TR warmup, got {}",
                out.adx
            );
        }
    }
}
