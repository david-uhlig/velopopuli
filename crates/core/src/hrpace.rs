//! Heart-rate pacing: picks an ERG target once a second so the rider's heart
//! rate settles inside a chosen band. Heart rate answers power slowly and
//! late, so the controller acts on a short-term prediction and waits between steps.

use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

/// Readings below this are a strap that has lost contact, not a heart rate.
const HR_MIN_BPM: f64 = 30.0;
/// Readings this far above max HR are sensor noise.
const HR_ABOVE_MAX_BPM: f64 = 5.0;
/// A reading further than this from the smoothed value is a glitch.
const HR_MAX_JUMP_BPM: f64 = 20.0;
/// Time constant of the heart-rate smoothing filter.
const HR_TAU_S: f64 = 8.0;
/// Window over which the heart-rate trend is fitted.
const TREND_WINDOW_S: f64 = 45.0;
/// Shorter history than this gives a trend too noisy to act on.
const TREND_MIN_SPAN_S: f64 = 20.0;
/// Deadband margin at each edge of the band, capped at a quarter of the band width.
const MARGIN_MAX_BPM: f64 = 3.0;
/// How far ahead the trend is extrapolated when predicting heart rate.
const PREDICT_MIN: f64 = 1.0;
/// Step size per bpm of predicted error, as a fraction of FTP.
const STEP_PER_BPM: f64 = 0.0075;
const STEP_MIN: f64 = 0.01;
const STEP_MAX: f64 = 0.04;
/// Easing faster when the smoothed heart rate is already above the band.
const HOT_STEP_FACTOR: f64 = 2.0;
// 15 s let a fast heart (tau 30 s) oscillate: several steps landed before the first showed.
const RAISE_COOLDOWN_S: f64 = 30.0;
const EASE_COOLDOWN_S: f64 = 15.0;
/// Warm-up ramp, as fractions of FTP.
const WARM_FROM: f64 = 0.5;
const WARM_TO: f64 = 0.7;
/// Warm-up ends once the smoothed heart rate is this close to the band floor.
const WARM_EXIT_BPM: f64 = 3.0;
const CADENCE_LOW: f64 = 60.0;
const CADENCE_LOW_S: f64 = 5.0;
const CADENCE_OK: f64 = 70.0;
const POWER_SHORT: f64 = 0.75;
const POWER_SHORT_S: f64 = 10.0;
const POWER_OK: f64 = 0.9;
const STRUGGLE_CUT: f64 = 0.05;
const CUT_EVERY_S: f64 = 5.0;
const NEAR_MAX_BPM: f64 = 3.0;
const NEAR_MAX_CUT: f64 = 0.10;
/// How long the power floor or ceiling may hold before it is reported.
const LIMIT_WARN_S: f64 = 180.0;
const NO_HR_AFTER_S: f64 = 30.0;
const NO_HR_RESUME_S: f64 = 10.0;
/// Below this power the rider has stopped pedalling.
const STOPPED_W: f64 = 10.0;
const STOPPED_S: f64 = 3.0;
const RESUME_RAISE_LOCK_S: f64 = 30.0;
/// Tolerance for comparing watts with their limits.
const EDGE_W: f64 = 1e-6;
/// Pacing that starts this early in a ride starts with a warm-up.
const START_WINDOW_S: f64 = 60.0;
/// A heart rate this far below the band means the rider has cooled off: warm up again.
const WELL_BELOW_BPM: f64 = 15.0;
/// Seconds of recent power averaged when pacing starts mid-ride.
pub const START_POWER_S: usize = 10;

/// Settings for one pacing session. Heart-rate limits are in bpm, power in watts.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PaceConfig {
    pub low: f64,
    pub high: f64,
    pub min_w: f64,
    pub max_w: f64,
    pub warmup_s: f64,
    pub ftp: f64,
    pub max_hr: f64,
}

/// One second of input from the rider and sensors.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PaceInput {
    /// Seconds since the session started.
    pub t: f64,
    pub hr: Option<f64>,
    pub power: f64,
    pub cadence: Option<f64>,
    pub paused: bool,
}

/// What the controller is doing this second.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum PaceState {
    WarmUp,
    Holding,
    Raising,
    Easing,
    Struggling,
    NoHr,
}

/// A condition the rider should be told about.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum PaceWarning {
    HighAtFloor,
    LowAtCeiling,
    NearMaxHr,
    NoHr,
}

/// The controller's decision for one second.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PaceOutput {
    pub watts: f64,
    pub state: PaceState,
    pub warning: Option<PaceWarning>,
    pub hr_smooth: Option<f64>,
    pub trend_bpm_min: f64,
}

/// Keeps heart rate in a band by nudging an ERG target. Call `step` once a second.
#[derive(Debug, Clone)]
pub struct HrPacer {
    watts: f64,
    state: PaceState,
    warning: Option<PaceWarning>,
    last_t: Option<f64>,
    smooth: Option<f64>,
    history: VecDeque<(f64, f64)>,
    warming: bool,
    warm_s: f64,
    cooldown_s: f64,
    raise_lock_s: f64,
    cut_s: f64,
    low_power_s: f64,
    frozen: bool,
    lost_s: f64,
    no_hr: bool,
    good_s: f64,
    cad_low_s: f64,
    pow_low_s: f64,
    struggling: bool,
    near_max: bool,
    floor_s: f64,
    ceil_s: f64,
}

/// Returns the reading if it is usable: finite, in a human range, and not a jump from the smoothed value.
fn plausible(hr: Option<f64>, smooth: Option<f64>, max_hr: f64) -> Option<f64> {
    let v = hr?;
    let ok = v.is_finite() && v >= HR_MIN_BPM && v <= max_hr + HR_ABOVE_MAX_BPM && smooth.is_none_or(|s| (v - s).abs() <= HR_MAX_JUMP_BPM);
    ok.then_some(v)
}

/// One step of a first-order filter with time constant `HR_TAU_S`.
fn ema(prev: Option<f64>, v: f64, dt: f64) -> f64 {
    match prev {
        None => v,
        Some(p) => p + (v - p) * (1.0 - (-dt / HR_TAU_S).exp()),
    }
}

/// Least-squares slope of the points, in units per minute. Zero until the points span enough time.
fn slope_per_min(points: &VecDeque<(f64, f64)>) -> f64 {
    if points.len() < 2 {
        return 0.0;
    }
    let (t0, t1) = (points[0].0, points[points.len() - 1].0);
    if t1 - t0 < TREND_MIN_SPAN_S {
        return 0.0;
    }
    let n = points.len() as f64;
    let mt = points.iter().map(|p| p.0).sum::<f64>() / n;
    let mv = points.iter().map(|p| p.1).sum::<f64>() / n;
    let sxx: f64 = points.iter().map(|p| (p.0 - mt).powi(2)).sum();
    let sxy: f64 = points.iter().map(|p| (p.0 - mt) * (p.1 - mv)).sum();
    if sxx <= 0.0 {
        return 0.0;
    }
    sxy / sxx * 60.0
}

/// The band the controller aims for: the rider's band shrunk by a deadband at each edge.
fn inner_band(low: f64, high: f64) -> (f64, f64) {
    let m = MARGIN_MAX_BPM.min((high - low) / 4.0).max(0.0);
    (low + m, high - m)
}

/// Watts to move for a given heart-rate error, clamped to a fraction of FTP.
fn step_watts(ftp: f64, err_bpm: f64) -> f64 {
    (STEP_PER_BPM * ftp * err_bpm).max(STEP_MIN * ftp).min(STEP_MAX * ftp)
}

fn clamp_w(cfg: &PaceConfig, w: f64) -> f64 {
    w.min(cfg.max_w).max(cfg.min_w)
}

/// How to start pacing: with a warm-up at the start of a ride, when the heart rate is well
/// below the band or when the rider isn't pedalling; otherwise from the recent average power.
/// `recent_power` is the last few one-second samples. Returns `(start_w, warm_up)`.
pub fn pace_start(cfg: &PaceConfig, elapsed_s: f64, hr: Option<f64>, recent_power: &[f64]) -> (f64, bool) {
    let mean = if recent_power.is_empty() { 0.0 } else { recent_power.iter().sum::<f64>() / recent_power.len() as f64 };
    let warm_up = elapsed_s < START_WINDOW_S || hr.is_some_and(|h| h < cfg.low - WELL_BELOW_BPM) || mean < STOPPED_W;
    if warm_up {
        (0.0, true)
    } else {
        (mean, false)
    }
}

impl HrPacer {
    /// Starts a session. A warm-up ramps from half FTP and ignores `start_w`;
    /// otherwise the controller holds `start_w` until the heart rate says otherwise.
    pub fn new(cfg: &PaceConfig, start_w: f64, warm_up: bool) -> Self {
        Self {
            watts: clamp_w(cfg, if warm_up { WARM_FROM * cfg.ftp } else { start_w }),
            state: if warm_up { PaceState::WarmUp } else { PaceState::Holding },
            warning: None,
            last_t: None,
            smooth: None,
            history: VecDeque::new(),
            warming: warm_up,
            warm_s: 0.0,
            cooldown_s: 0.0,
            raise_lock_s: 0.0,
            cut_s: 0.0,
            low_power_s: 0.0,
            frozen: false,
            lost_s: 0.0,
            no_hr: false,
            good_s: 0.0,
            cad_low_s: 0.0,
            pow_low_s: 0.0,
            struggling: false,
            near_max: false,
            floor_s: 0.0,
            ceil_s: 0.0,
        }
    }

    /// Advances the controller by one input and returns the target to set now.
    pub fn step(&mut self, cfg: &PaceConfig, input: PaceInput) -> PaceOutput {
        let dt = match self.last_t {
            None => 0.0,
            Some(p) => (input.t - p).max(0.0),
        };
        self.last_t = Some(input.t);

        self.low_power_s = if input.power < STOPPED_W { self.low_power_s + dt } else { 0.0 };
        let frozen_now = input.paused || self.low_power_s >= STOPPED_S;

        let reading = plausible(input.hr, self.smooth, cfg.max_hr);
        if let Some(v) = reading {
            self.smooth = Some(ema(self.smooth, v, dt));
        }
        let fresh = reading.is_some();

        if frozen_now {
            self.frozen = true;
            return self.output(slope_per_min(&self.history));
        }

        self.cooldown_s = (self.cooldown_s - dt).max(0.0);
        self.raise_lock_s = (self.raise_lock_s - dt).max(0.0);
        self.cut_s = (self.cut_s - dt).max(0.0);
        if self.frozen {
            self.frozen = false;
            self.history.clear();
            self.raise_lock_s = RESUME_RAISE_LOCK_S;
            self.cad_low_s = 0.0;
            self.pow_low_s = 0.0;
        }

        self.watts = clamp_w(cfg, self.watts);
        let target = self.watts;

        if fresh {
            self.lost_s = 0.0;
        } else {
            self.lost_s += dt;
        }
        if self.no_hr {
            if fresh {
                self.good_s += dt;
            } else {
                self.good_s = 0.0;
                self.smooth = None;
            }
            if self.good_s >= NO_HR_RESUME_S {
                self.no_hr = false;
                self.good_s = 0.0;
                self.history.clear();
                self.raise_lock_s = RESUME_RAISE_LOCK_S;
            }
        } else if self.lost_s >= NO_HR_AFTER_S {
            self.no_hr = true;
            self.smooth = None;
            self.history.clear();
            self.good_s = 0.0;
            self.floor_s = 0.0;
            self.ceil_s = 0.0;
        }
        let accepted = fresh && !self.no_hr;
        let s = self.smooth.unwrap_or(0.0);
        if accepted {
            self.history.push_back((input.t, s));
        }
        while self.history.front().is_some_and(|&(t0, _)| t0 < input.t - TREND_WINDOW_S) {
            self.history.pop_front();
        }
        let trend = slope_per_min(&self.history);

        self.cad_low_s = if matches!(input.cadence, Some(c) if c < CADENCE_LOW) { self.cad_low_s + dt } else { 0.0 };
        self.pow_low_s = if input.power < POWER_SHORT * target { self.pow_low_s + dt } else { 0.0 };
        if !self.struggling {
            if self.cad_low_s >= CADENCE_LOW_S || self.pow_low_s >= POWER_SHORT_S {
                self.struggling = true;
                self.cut_s = 0.0;
                self.warming = false;
            }
        } else if input.cadence.is_none_or(|c| c >= CADENCE_OK) && input.power >= POWER_OK * target {
            self.struggling = false;
            self.cooldown_s = RAISE_COOLDOWN_S;
        }

        let hot = accepted && s >= cfg.max_hr - NEAR_MAX_BPM;
        if hot && !self.near_max {
            self.cut_s = 0.0;
        }
        self.near_max = hot;
        if hot {
            self.warming = false;
        }

        let state = if self.near_max {
            if self.cut_s <= 0.0 {
                self.watts = clamp_w(cfg, self.watts * (1.0 - NEAR_MAX_CUT));
                self.cut_s = CUT_EVERY_S;
            }
            PaceState::Easing
        } else if self.struggling {
            if self.cut_s <= 0.0 {
                self.watts = clamp_w(cfg, self.watts * (1.0 - STRUGGLE_CUT));
                self.cut_s = CUT_EVERY_S;
            }
            PaceState::Struggling
        } else if self.no_hr {
            PaceState::NoHr
        } else if !fresh {
            if self.warming {
                PaceState::WarmUp
            } else {
                PaceState::Holding
            }
        } else if self.warming {
            self.warm_s += dt;
            if s < cfg.low - WARM_EXIT_BPM && self.warm_s < cfg.warmup_s {
                self.watts = clamp_w(cfg, cfg.ftp * (WARM_FROM + (WARM_TO - WARM_FROM) * (self.warm_s / cfg.warmup_s).min(1.0)));
                PaceState::WarmUp
            } else {
                self.warming = false;
                self.control(cfg, s, trend)
            }
        } else {
            self.control(cfg, s, trend)
        };

        if accepted {
            self.floor_s = if self.watts <= cfg.min_w + EDGE_W && s > cfg.high { self.floor_s + dt } else { 0.0 };
            self.ceil_s = if self.watts >= cfg.max_w - EDGE_W && s < cfg.low { self.ceil_s + dt } else { 0.0 };
        }

        self.state = state;
        self.warning = if self.near_max { Some(PaceWarning::NearMaxHr) } else { self.steady_warning() };
        self.output(trend)
    }

    /// Seconds of warm-up still to go, or `None` once it is over (or there was none).
    pub fn warmup_left_s(&self, cfg: &PaceConfig) -> Option<f64> {
        self.warming.then(|| (cfg.warmup_s - self.warm_s).max(0.0))
    }

    /// Moves the target toward the band from the predicted heart rate, respecting cooldowns and the raise lock.
    fn control(&mut self, cfg: &PaceConfig, s: f64, trend: f64) -> PaceState {
        let (lo, hi) = inner_band(cfg.low, cfg.high);
        let predicted = s + trend * PREDICT_MIN;
        if predicted < lo {
            if self.raise_lock_s > 0.0 || self.watts >= cfg.max_w - EDGE_W {
                PaceState::Holding
            } else {
                if self.cooldown_s <= 0.0 {
                    self.watts = clamp_w(cfg, self.watts + step_watts(cfg.ftp, lo - predicted));
                    self.cooldown_s = RAISE_COOLDOWN_S;
                }
                PaceState::Raising
            }
        } else if predicted > hi {
            if self.watts <= cfg.min_w + EDGE_W {
                PaceState::Holding
            } else {
                let hot = if s > cfg.high { HOT_STEP_FACTOR } else { 1.0 };
                let w = step_watts(cfg.ftp, predicted - hi) * hot;
                if self.cooldown_s <= 0.0 {
                    self.watts = clamp_w(cfg, self.watts - w);
                    self.cooldown_s = EASE_COOLDOWN_S;
                }
                PaceState::Easing
            }
        } else {
            PaceState::Holding
        }
    }

    fn steady_warning(&self) -> Option<PaceWarning> {
        if self.no_hr {
            Some(PaceWarning::NoHr)
        } else if self.floor_s >= LIMIT_WARN_S {
            Some(PaceWarning::HighAtFloor)
        } else if self.ceil_s >= LIMIT_WARN_S {
            Some(PaceWarning::LowAtCeiling)
        } else {
            None
        }
    }

    fn output(&self, trend_bpm_min: f64) -> PaceOutput {
        PaceOutput { watts: self.watts, state: self.state, warning: self.warning, hr_smooth: self.smooth, trend_bpm_min }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg() -> PaceConfig {
        PaceConfig { low: 135.0, high: 145.0, min_w: 100.0, max_w: 262.5, warmup_s: 600.0, ftp: 250.0, max_hr: 190.0 }
    }

    #[test]
    fn plausible_readings_are_bounded_and_jump_checked() {
        assert_eq!(plausible(Some(29.0), None, 190.0), None);
        assert_eq!(plausible(Some(30.0), None, 190.0), Some(30.0));
        assert_eq!(plausible(Some(195.0), None, 190.0), Some(195.0));
        assert_eq!(plausible(Some(195.5), None, 190.0), None);
        assert_eq!(plausible(Some(f64::NAN), None, 190.0), None);
        assert_eq!(plausible(None, None, 190.0), None);
        assert_eq!(plausible(Some(160.0), Some(140.0), 190.0), Some(160.0));
        assert_eq!(plausible(Some(160.5), Some(140.0), 190.0), None);
        assert_eq!(plausible(Some(180.0), None, 190.0), Some(180.0));
    }

    #[test]
    fn ema_takes_the_first_reading_then_moves_by_elapsed_time() {
        assert_eq!(ema(None, 120.0, 1.0), 120.0);
        assert_eq!(ema(Some(100.0), 110.0, 0.0), 100.0);
        assert!((ema(Some(100.0), 110.0, 8.0) - (100.0 + 10.0 * (1.0 - (-1.0f64).exp()))).abs() < 1e-9);
    }

    #[test]
    fn trend_is_a_slope_per_minute_over_a_long_enough_span() {
        let mut points: VecDeque<(f64, f64)> = [(0.0, 100.0), (10.0, 105.0), (20.0, 110.0), (30.0, 115.0)].into_iter().collect();
        assert!((slope_per_min(&points) - 30.0).abs() < 1e-9);
        points.pop_back();
        assert!((slope_per_min(&points) - 30.0).abs() < 1e-9);
        let short: VecDeque<(f64, f64)> = [(0.0, 100.0), (10.0, 105.0)].into_iter().collect();
        assert_eq!(slope_per_min(&short), 0.0);
        let single: VecDeque<(f64, f64)> = [(0.0, 100.0)].into_iter().collect();
        assert_eq!(slope_per_min(&single), 0.0);
    }

    #[test]
    fn inner_band_shrinks_by_a_capped_margin() {
        assert_eq!(inner_band(135.0, 145.0), (137.5, 142.5));
        assert_eq!(inner_band(130.0, 150.0), (133.0, 147.0));
        assert_eq!(inner_band(140.0, 140.0), (140.0, 140.0));
    }

    #[test]
    fn steps_are_proportional_to_error_within_limits() {
        assert!((step_watts(250.0, 0.1) - 2.5).abs() < 1e-9);
        assert!((step_watts(250.0, 2.0) - 3.75).abs() < 1e-9);
        assert!((step_watts(250.0, 10.0) - 10.0).abs() < 1e-9);
    }

    #[test]
    fn new_starts_from_warm_up_or_from_the_given_watts() {
        let warm = HrPacer::new(&cfg(), 0.0, true);
        assert_eq!(warm.watts, 125.0);
        assert_eq!(warm.state, PaceState::WarmUp);
        let held = HrPacer::new(&cfg(), 500.0, false);
        assert_eq!(held.watts, 262.5);
        assert_eq!(held.state, PaceState::Holding);
    }

    #[test]
    fn pace_start_warms_up_early_cold_or_stopped() {
        assert_eq!(pace_start(&cfg(), 0.0, None, &[]), (0.0, true));
        assert_eq!(pace_start(&cfg(), 59.9, Some(140.0), &[200.0; 10]), (0.0, true));
        assert_eq!(pace_start(&cfg(), 600.0, Some(119.9), &[200.0; 10]), (0.0, true));
        assert_eq!(pace_start(&cfg(), 600.0, Some(140.0), &[5.0; 10]), (0.0, true));
        assert_eq!(pace_start(&cfg(), 600.0, Some(140.0), &[]), (0.0, true));
    }

    #[test]
    fn pace_start_mid_ride_holds_the_recent_average() {
        assert_eq!(pace_start(&cfg(), 600.0, Some(120.0), &[200.0; 10]), (200.0, false));
        assert_eq!(pace_start(&cfg(), 600.0, None, &[150.0, 160.0]), (155.0, false));
    }

    #[test]
    fn warm_up_time_left_counts_down_and_ends() {
        let mut p = HrPacer::new(&cfg(), 0.0, true);
        assert_eq!(p.warmup_left_s(&cfg()), Some(600.0));
        p.step(&cfg(), PaceInput { t: 0.0, hr: Some(100.0), power: 150.0, cadence: Some(90.0), paused: false });
        p.step(&cfg(), PaceInput { t: 1.0, hr: Some(100.0), power: 150.0, cadence: Some(90.0), paused: false });
        assert_eq!(p.warmup_left_s(&cfg()), Some(599.0));
        assert_eq!(HrPacer::new(&cfg(), 200.0, false).warmup_left_s(&cfg()), None);
    }
}
