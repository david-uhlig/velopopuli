//! Heart-rate pacing against a simulated heart and rider.

use std::collections::VecDeque;

use velopopuli_core::hrpace::{HrPacer, PaceConfig, PaceInput, PaceOutput, PaceState, PaceWarning};

fn cfg() -> PaceConfig {
    PaceConfig { low: 135.0, high: 145.0, min_w: 100.0, max_w: 262.5, warmup_s: 600.0, ftp: 250.0, max_hr: 190.0 }
}

/// A heart that answers power after a dead time with a first-order lag, plus optional slow drift.
struct Heart {
    rest: f64,
    k: f64,
    tau: f64,
    dead_s: usize,
    drift_per_min: f64,
    drift_from: f64,
    hr: f64,
    queue: VecDeque<f64>,
}

impl Heart {
    fn new(tau: f64) -> Self {
        Heart { rest: 60.0, k: 0.4, tau, dead_s: 15, drift_per_min: 0.0, drift_from: 600.0, hr: 65.0, queue: VecDeque::new() }
    }

    /// One second with the rider producing `power`; returns the true heart rate.
    fn tick(&mut self, t: f64, power: f64) -> f64 {
        self.queue.push_back(power);
        if self.queue.len() > self.dead_s + 1 {
            self.queue.pop_front();
        }
        let target = self.rest + self.k * self.queue[0] + self.drift_per_min * (t - self.drift_from).max(0.0) / 60.0;
        self.hr += (target - self.hr) * (1.0 - (-1.0 / self.tau).exp());
        self.hr
    }
}

/// Repeatable strap noise of up to ±1 bpm.
struct Noise(u64);

impl Noise {
    fn next(&mut self) -> f64 {
        self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        (self.0 >> 11) as f64 / (1u64 << 53) as f64 * 2.0 - 1.0
    }
}

enum HrSignal {
    Heart,
    Missing,
    Value(f64),
}

/// What the rider and strap do in one second; scripts change the defaults.
struct Tick {
    power: f64,
    cadence: Option<f64>,
    paused: bool,
    hr: HrSignal,
}

struct Sample {
    t: f64,
    hr: f64,
    out: PaceOutput,
}

/// Rides `secs` seconds at 1 Hz. By default the rider holds the commanded watts at 90 rpm.
fn ride(mut cfg: PaceConfig, mut heart: Heart, warm_up: bool, start_w: f64, secs: usize, mut script: impl FnMut(f64, &mut PaceConfig, &mut Tick)) -> Vec<Sample> {
    let mut pacer = HrPacer::new(&cfg, start_w, warm_up);
    let mut watts = if warm_up { 0.5 * cfg.ftp } else { start_w.min(cfg.max_w).max(cfg.min_w) };
    let mut noise = Noise(12345);
    let mut log = Vec::with_capacity(secs);
    for i in 0..secs {
        let t = i as f64;
        let mut tick = Tick { power: watts, cadence: Some(90.0), paused: false, hr: HrSignal::Heart };
        script(t, &mut cfg, &mut tick);
        let true_hr = heart.tick(t, tick.power);
        let n = noise.next();
        let hr = match tick.hr {
            HrSignal::Heart => Some(true_hr + n),
            HrSignal::Missing => None,
            HrSignal::Value(v) => Some(v),
        };
        let out = pacer.step(&cfg, PaceInput { t, hr, power: tick.power, cadence: tick.cadence, paused: tick.paused });
        watts = out.watts;
        log.push(Sample { t, hr: true_hr, out });
    }
    log
}

fn settled(_: f64, _: &mut PaceConfig, _: &mut Tick) {}

/// Sample summary for assertion messages.
fn at(s: &Sample) -> String {
    format!("t={} hr={:.1} watts={:.1} state={:?} warning={:?}", s.t, s.hr, s.out.watts, s.out.state, s.out.warning)
}

/// Asserts that watts never rise between consecutive samples in `log[a..b]`.
fn no_increase(log: &[Sample], a: usize, b: usize) {
    for w in log[a..b].windows(2) {
        assert!(w[1].out.watts <= w[0].out.watts + 1e-9, "watts rose: {}", at(&w[1]));
    }
}

/// Largest minus smallest watts in `log`.
fn watts_span(log: &[Sample]) -> f64 {
    let hi = log.iter().map(|s| s.out.watts).fold(f64::NEG_INFINITY, f64::max);
    let lo = log.iter().map(|s| s.out.watts).fold(f64::INFINITY, f64::min);
    hi - lo
}

#[test]
fn warm_up_reaches_the_band_without_overshooting() {
    for tau in [30.0, 60.0, 90.0] {
        let log = ride(cfg(), Heart::new(tau), true, 0.0, 1800, settled);
        assert_eq!(log[0].out.watts, 125.0, "tau {tau}: {}", at(&log[0]));
        assert_eq!(log[0].out.state, PaceState::WarmUp, "tau {tau}: {}", at(&log[0]));
        for s in log.iter().filter(|s| s.out.state == PaceState::WarmUp) {
            assert!(s.out.watts <= 175.0 + 1e-9, "tau {tau}: warm-up overshot: {}", at(s));
        }
        let first = log.iter().find(|s| s.out.state != PaceState::WarmUp).unwrap();
        assert!(first.t <= 601.0, "tau {tau}: still warming up: {}", at(first));
        assert!(log[..=900].iter().any(|s| s.hr >= 135.0), "tau {tau}: no hr >= 135 by t=900: {}", at(&log[900]));
        let hottest = log.iter().max_by(|a, b| a.hr.total_cmp(&b.hr)).unwrap();
        assert!(hottest.hr <= 148.0, "tau {tau}: overshoot: {}", at(hottest));
    }
}

#[test]
fn holds_the_band_once_settled() {
    for tau in [30.0, 60.0, 90.0] {
        let log = ride(cfg(), Heart::new(tau), true, 0.0, 2700, settled);
        for s in &log[1500..2700] {
            assert!((135.0..=145.0).contains(&s.hr), "tau {tau}: out of band: {}", at(s));
            assert_eq!(s.out.warning, None, "tau {tau}: {}", at(s));
            assert!(matches!(s.out.state, PaceState::Holding | PaceState::Raising | PaceState::Easing), "tau {tau}: {}", at(s));
        }
        let span = watts_span(&log[1500..2700]);
        assert!(span <= 10.0 + 1e-9, "tau {tau}: watts span {span:.1} over 1500..2700: {}", at(&log[2699]));
    }
}

#[test]
fn cardiac_drift_lowers_power_not_heart_rate() {
    for tau in [30.0, 60.0, 90.0] {
        let log = ride(cfg(), Heart { drift_per_min: 0.25, ..Heart::new(tau) }, true, 0.0, 5100, settled);
        for s in &log[1500..] {
            assert!((135.0..=145.0).contains(&s.hr), "tau {tau}: drift left the band: {}", at(s));
        }
        assert!(log[5099].out.watts <= log[1500].out.watts - 20.0, "tau {tau}: power did not fall with drift: {} (start {})", at(&log[5099]), at(&log[1500]));
    }
}

#[test]
fn stops_at_the_floor_and_warns() {
    let heart = Heart { rest: 150.0, k: 0.1, hr: 150.0, ..Heart::new(60.0) };
    let log = ride(cfg(), heart, false, 200.0, 1000, |t, _, tick| {
        if (900.0..930.0).contains(&t) {
            tick.hr = HrSignal::Value(150.0);
        } else if (930.0..1000.0).contains(&t) {
            tick.hr = HrSignal::Value(135.0);
        }
    });
    for s in &log {
        assert!(s.out.watts >= 100.0 - 1e-9, "below the floor: {}", at(s));
    }
    let tf = log.iter().position(|s| s.out.watts <= 100.0 + 1e-9).expect("never reached the floor");
    assert!(tf < 300, "reached the floor too late: {}", at(&log[tf]));
    assert_ne!(log[tf + 170].out.warning, Some(PaceWarning::HighAtFloor), "warned too early: {}", at(&log[tf + 170]));
    assert_eq!(log[tf + 190].out.warning, Some(PaceWarning::HighAtFloor), "no floor warning: {}", at(&log[tf + 190]));
    assert_eq!(log[999].out.warning, None, "{}", at(&log[999]));
}

#[test]
fn stops_at_the_ceiling_and_warns() {
    let log = ride(cfg(), Heart { k: 0.2, ..Heart::new(60.0) }, false, 150.0, 1000, settled);
    for s in &log {
        assert!(s.out.watts <= 262.5 + 1e-9, "above the ceiling: {}", at(s));
    }
    let tc = log.iter().position(|s| s.out.watts >= 262.5 - 1e-9).expect("never reached the ceiling");
    assert!(tc < 600, "reached the ceiling too late: {}", at(&log[tc]));
    assert_eq!(log[tc + 170].out.warning, None, "warned too early: {}", at(&log[tc + 170]));
    assert_eq!(log[tc + 190].out.warning, Some(PaceWarning::LowAtCeiling), "no ceiling warning: {}", at(&log[tc + 190]));
}

#[test]
fn cadence_collapse_stops_raises_and_backs_off() {
    let log = ride(cfg(), Heart::new(60.0), true, 0.0, 1800, |t, _, tick| {
        if (1500.0..1620.0).contains(&t) {
            tick.cadence = Some(50.0);
        }
    });
    no_increase(&log, 1505, 1620);
    for s in &log[1505..1620] {
        assert_eq!(s.out.state, PaceState::Struggling, "{}", at(s));
    }
    assert!(log[1619].out.watts <= 0.8 * log[1499].out.watts, "did not back off: {} (before {})", at(&log[1619]), at(&log[1499]));
    for s in &log {
        assert!(s.out.watts >= 100.0 - 1e-9, "below the floor: {}", at(s));
    }
    assert_ne!(log[1621].out.state, PaceState::Struggling, "{}", at(&log[1621]));
}

#[test]
fn power_shortfall_counts_as_struggling_without_cadence() {
    let log = ride(cfg(), Heart::new(60.0), true, 0.0, 1700, |t, _, tick| {
        tick.cadence = None;
        if (1500.0..1545.0).contains(&t) {
            tick.power = tick.power.min(120.0);
        }
    });
    for s in &log[1512..=1540] {
        assert_eq!(s.out.state, PaceState::Struggling, "{}", at(s));
    }
    no_increase(&log, 1510, 1545);
    assert!(log[1540].out.watts < 0.8 * log[1499].out.watts, "did not back off: {} (before {})", at(&log[1540]), at(&log[1499]));
}

#[test]
fn heart_rate_dropout_holds_then_hands_over() {
    let log = ride(cfg(), Heart::new(60.0), true, 0.0, 2100, |t, _, tick| {
        if (1500.0..1560.0).contains(&t) {
            tick.hr = HrSignal::Missing;
        }
    });
    let w = log[1499].out.watts;
    for s in &log[1500..1569] {
        assert_eq!(s.out.watts, w, "watts moved during dropout: {}", at(s));
    }
    assert_ne!(log[1520].out.state, PaceState::NoHr, "{}", at(&log[1520]));
    assert_eq!(log[1520].out.warning, None, "{}", at(&log[1520]));
    for s in &log[1531..=1565] {
        assert_eq!(s.out.state, PaceState::NoHr, "{}", at(s));
        assert_eq!(s.out.warning, Some(PaceWarning::NoHr), "{}", at(s));
    }
    for s in &log[1572..] {
        assert_ne!(s.out.state, PaceState::NoHr, "{}", at(s));
        assert_ne!(s.out.warning, Some(PaceWarning::NoHr), "{}", at(s));
    }
    assert!((135.0..=145.0).contains(&log[2099].hr), "{}", at(&log[2099]));
}

#[test]
fn implausible_readings_are_ignored() {
    let log = ride(cfg(), Heart::new(60.0), true, 0.0, 1700, |t, _, tick| {
        tick.hr = match t as usize {
            1500 => HrSignal::Value(220.0),
            1501 => HrSignal::Value(0.0),
            1502 => HrSignal::Value(f64::NAN),
            1510 => HrSignal::Value(175.0),
            _ => HrSignal::Heart,
        };
    });
    for i in [1500, 1501, 1502] {
        assert_eq!(log[i].out.hr_smooth, log[1499].out.hr_smooth, "{}", at(&log[i]));
        assert_eq!(log[i].out.watts, log[1499].out.watts, "{}", at(&log[i]));
    }
    assert_eq!(log[1510].out.hr_smooth, log[1509].out.hr_smooth, "{}", at(&log[1510]));
    for s in &log[1500..1700] {
        assert_ne!(s.out.state, PaceState::NoHr, "{}", at(s));
        assert_eq!(s.out.warning, None, "{}", at(s));
    }
}

#[test]
fn pause_freezes_and_resume_waits_before_raising() {
    let log = ride(cfg(), Heart::new(60.0), true, 0.0, 2400, |t, _, tick| {
        if (1500.0..1560.0).contains(&t) {
            tick.paused = true;
            tick.power = 0.0;
            tick.cadence = None;
        }
    });
    let before = log[1499].out;
    for s in &log[1500..1560] {
        assert_eq!(s.out.watts, before.watts, "watts moved while paused: {}", at(s));
        assert_eq!(s.out.state, before.state, "state changed while paused: {}", at(s));
    }
    no_increase(&log, 1559, 1590);
    assert!(log[1590..1800].windows(2).any(|w| w[1].out.watts > w[0].out.watts + 1e-9), "no raise after the resume lock: {}", at(&log[1799]));
    for s in &log[2160..2400] {
        assert!((135.0..=145.0).contains(&s.hr), "{}", at(s));
    }
}

#[test]
fn stopping_pedalling_freezes_like_a_pause() {
    let log = ride(cfg(), Heart::new(60.0), true, 0.0, 1800, |t, _, tick| {
        if (1500.0..1540.0).contains(&t) {
            tick.power = 0.0;
            tick.cadence = Some(0.0);
        }
    });
    for s in &log[1503..1540] {
        assert_eq!(s.out.watts, log[1503].out.watts, "watts moved while stopped: {}", at(s));
    }
    no_increase(&log, 1539, 1570);
}

#[test]
fn follows_a_band_change() {
    for tau in [30.0, 60.0, 90.0] {
        for (low, high) in [(145.0, 155.0), (125.0, 135.0)] {
            let log = ride(cfg(), Heart::new(tau), true, 0.0, 2400, |t, c, _| {
                if t >= 1800.0 {
                    c.low = low;
                    c.high = high;
                }
            });
            assert!(log[1800..=2040].iter().any(|s| (low..=high).contains(&s.hr)), "tau {tau} band {low}-{high}: never entered by t=2040: {}", at(&log[2040]));
            for s in &log[2100..2400] {
                assert!((low..=high).contains(&s.hr), "tau {tau} band {low}-{high}: {}", at(s));
            }
        }
    }
}

#[test]
fn states_and_warnings_serialise_in_camel_case() {
    assert_eq!(serde_json::to_string(&PaceState::WarmUp).unwrap(), "\"warmUp\"");
    assert_eq!(serde_json::to_string(&PaceState::NoHr).unwrap(), "\"noHr\"");
    assert_eq!(serde_json::to_string(&PaceWarning::HighAtFloor).unwrap(), "\"highAtFloor\"");
    let out = PaceOutput { watts: 200.0, state: PaceState::Holding, warning: None, hr_smooth: Some(140.0), trend_bpm_min: 0.5 };
    let json = serde_json::to_string(&out).unwrap();
    assert!(json.contains("\"hrSmooth\""), "{json}");
    assert!(json.contains("\"trendBpmMin\""), "{json}");
}
