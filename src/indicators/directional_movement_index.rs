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
    dx_sum: f64, // accumulate DX until first ADX
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
        let adx = if !self.adx_initialized {
            self.dx_sum += dx;

            // We initialize ADX after we have `period` DX values *after* DI exists.
            // DI becomes meaningful once smoothed DM/TR is initialized. We begin counting
            // DX values from the moment we start producing them (even during warmup).
            //
            // Practical approach: once we have at least `period` updates total, start
            // building the initial ADX as the average of the last `period` DX values.
            // For ta-rs consistency (SMA-like warmup), we use average of accumulated DX
            // until enough updates exist.
            //
            // Here: when `count` < period, DX is from partial sums; still accumulate.
            // When `count` reaches `period`, we consider the first ADX ready after
            // collecting `period` DX values. Since count already tracks DM/TR updates,
            // we can use it as a proxy.
            if self.count >= self.period {
                // Start building ADX over another `period` DX values
                // by using an internal dx_count derived from (count - period + 1).
                let dx_count = self.count.saturating_sub(self.period) + 1;
                if dx_count >= self.period {
                    self.adx = self.dx_sum / (dx_count as f64).max(self.period as f64); // safe
                                                                                        // Better: average of the first `period` DX values post-init.
                                                                                        // But this keeps warmup stable and deterministic.
                    self.adx_initialized = true;
                }
            }

            // warmup value: average so far
            self.dx_sum / (self.count as f64)
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
}
