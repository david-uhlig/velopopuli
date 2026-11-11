//! The demo rider's body: a made-up heart that answers effort late and slowly
//! and drifts upward on long efforts, and legs that can tire on a hard ERG
//! target. Kept here, away from the app's demo loop, so it can be tested.

use std::collections::VecDeque;

/// The made-up rider's FTP. The demo heart is one fixed person, whatever FTP the
/// real rider has set, so the demo behaves the same for everyone.
pub const DEMO_FTP: f64 = 200.0;
/// Heart rate with no effort; the demo heart also starts here.
pub const REST_BPM: f64 = 70.0;
/// Steady heart rate rises this much per watt: about 166 bpm at FTP.
const BPM_PER_W: f64 = 0.48;
/// The heart answers a change of effort only after this long.
const DELAY_S: f64 = 12.0;
/// Time constant of the heart's response once it starts.
const TAU_S: f64 = 30.0;
/// Cardiac drift: above half FTP the steady heart rate creeps up this much a minute…
const DRIFT_FROM_W: f64 = 0.5 * DEMO_FTP;
const DRIFT_PER_MIN: f64 = 0.3;
/// …up to this much in all…
const DRIFT_MAX_BPM: f64 = 15.0;
/// …and eases back with this time constant while the effort is easy.
const DRIFT_RECOVER_TAU_S: f64 = 300.0;
/// The heart goes no higher than this, so readings stay believable.
const TOP_BPM: f64 = 188.0;

/// The demo rider's heart. Call `step` with the rider's power as time passes.
#[derive(Debug, Clone)]
pub struct DemoHeart {
    t: f64,
    hr: f64,
    drift: f64,
    /// Recent `(time, watts)`, oldest first, covering the delay.
    power: VecDeque<(f64, f64)>,
}

impl DemoHeart {
    /// A heart beating at `hr` after a rest (no recent effort).
    pub fn new(hr: f64) -> Self {
        Self { t: 0.0, hr, drift: 0.0, power: VecDeque::from([(0.0, 0.0)]) }
    }

    /// Advances `dt` seconds with the rider producing `power` watts; returns the heart rate.
    pub fn step(&mut self, dt: f64, power: f64) -> f64 {
        let dt = if dt.is_finite() { dt.max(0.0) } else { 0.0 };
        let power = if power.is_finite() { power.max(0.0) } else { 0.0 };
        self.t += dt;
        self.power.push_back((self.t, power));
        // The front entry is the effort the heart is answering now: the newest one at least DELAY_S old.
        while self.power.len() > 1 && self.power[1].0 <= self.t - DELAY_S {
            self.power.pop_front();
        }
        let felt = self.power[0].1;
        if felt > DRIFT_FROM_W {
            self.drift = (self.drift + DRIFT_PER_MIN * dt / 60.0).min(DRIFT_MAX_BPM);
        } else {
            self.drift *= (-dt / DRIFT_RECOVER_TAU_S).exp();
        }
        let target = (REST_BPM + BPM_PER_W * felt + self.drift).min(TOP_BPM);
        self.hr += (target - self.hr) * (1.0 - (-dt / TAU_S).exp());
        self.hr
    }

    pub fn hr(&self) -> f64 {
        self.hr
    }

    /// How far cardiac drift has raised the steady heart rate, in bpm.
    pub fn drift(&self) -> f64 {
        self.drift
    }
}

/// The tired rider fades after this long on an ERG target above their limit…
const FADE_AFTER_S: f64 = 120.0;
/// …and recovers this many times faster than they tired, below it.
const RECOVER_RATE: f64 = 2.0;
/// A fading rider's cadence, below the pacer's 60 rpm "struggling" line.
pub const FADE_CADENCE: f64 = 52.0;
/// A fading rider's power, as a fraction of what they would otherwise ride.
pub const FADE_POWER: f64 = 0.85;

/// Legs that tire: an ERG target above `above_w` for long makes the rider fade
/// until they have fully recovered at an easier target. Call `step` as time passes.
#[derive(Debug, Clone)]
pub struct Exhaustion {
    above_w: f64,
    strain_s: f64,
    fading: bool,
}

impl Exhaustion {
    pub fn new(above_w: f64) -> Self {
        Self { above_w, strain_s: 0.0, fading: false }
    }

    /// Advances `dt` seconds under the ERG target `erg` (`None`: no ERG target). Returns whether the rider is fading.
    pub fn step(&mut self, dt: f64, erg: Option<f64>) -> bool {
        let dt = if dt.is_finite() { dt.max(0.0) } else { 0.0 };
        if erg.is_some_and(|w| w > self.above_w) {
            self.strain_s += dt;
        } else {
            self.strain_s = (self.strain_s - RECOVER_RATE * dt).max(0.0);
        }
        if self.strain_s >= FADE_AFTER_S {
            self.fading = true;
        } else if self.strain_s <= 0.0 {
            self.fading = false;
        }
        self.fading
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DT: f64 = 0.25;

    /// Runs `secs` seconds at `watts` in quarter-second steps; returns the last heart rate.
    fn ride(h: &mut DemoHeart, secs: f64, watts: f64) -> f64 {
        for _ in 0..(secs / DT).round() as usize {
            h.step(DT, watts);
        }
        h.hr()
    }

    #[test]
    fn effort_shows_only_after_the_delay() {
        let mut h = DemoHeart::new(REST_BPM);
        assert_eq!(ride(&mut h, 60.0, 0.0), REST_BPM);
        assert_eq!(ride(&mut h, 11.75, 200.0), REST_BPM);
        assert!(ride(&mut h, 1.0, 200.0) > REST_BPM);
    }

    #[test]
    fn heart_rate_follows_with_a_thirty_second_lag() {
        let mut h = DemoHeart::new(REST_BPM);
        ride(&mut h, 60.0, 0.0);
        // 100 W is exactly half the demo FTP, so no drift: the steady heart rate is 70 + 48 = 118.
        let hr = ride(&mut h, 42.0, 100.0);
        assert!((hr - (REST_BPM + 48.0 * (1.0 - (-1.0f64).exp()))).abs() < 0.01, "{hr}");
        let hr = ride(&mut h, 1800.0, 100.0);
        assert!((hr - 118.0).abs() < 0.01, "{hr}");
        assert_eq!(h.drift(), 0.0);
    }

    #[test]
    fn drift_builds_above_half_ftp_up_to_a_limit() {
        let mut h = DemoHeart::new(REST_BPM);
        let hr = ride(&mut h, 600.0, 150.0);
        assert!((2.8..=3.0).contains(&h.drift()), "{}", h.drift());
        assert!((hr - (REST_BPM + 72.0 + h.drift())).abs() < 0.5, "{hr}");
        ride(&mut h, 3600.0, 150.0);
        assert_eq!(h.drift(), DRIFT_MAX_BPM);
    }

    #[test]
    fn drift_eases_back_when_the_effort_is_easy() {
        let mut h = DemoHeart::new(REST_BPM);
        ride(&mut h, 1200.0, 150.0);
        let built = h.drift();
        ride(&mut h, 300.0, 50.0);
        assert!(h.drift() > 0.3 * built && h.drift() < 0.45 * built, "{} of {built}", h.drift());
    }

    #[test]
    fn heart_rate_stops_at_the_top() {
        let mut h = DemoHeart::new(REST_BPM);
        let hr = ride(&mut h, 600.0, 400.0);
        assert!((187.0..=TOP_BPM).contains(&hr), "{hr}");
    }

    #[test]
    fn same_inputs_give_the_same_heart_and_bad_inputs_are_ignored() {
        let (mut a, mut b) = (DemoHeart::new(95.0), DemoHeart::new(95.0));
        for i in 0..4000 {
            let w = 100.0 + 80.0 * ((i as f64) * 0.01).sin();
            assert_eq!(a.step(DT, w), b.step(DT, w));
        }
        let (hr, drift) = (a.hr(), a.drift());
        assert_eq!(a.step(-1.0, 300.0), hr);
        assert_eq!(a.step(f64::NAN, 300.0), hr);
        assert_eq!(a.drift(), drift);
        let mut c = DemoHeart::new(REST_BPM);
        let mut d = DemoHeart::new(REST_BPM);
        assert_eq!(ride(&mut c, 120.0, f64::NAN), ride(&mut d, 120.0, 0.0));
    }

    #[test]
    fn exhaustion_fades_after_two_minutes_above_the_limit() {
        let mut e = Exhaustion::new(100.0);
        for _ in 0..479 {
            assert!(!e.step(DT, Some(120.0)));
        }
        assert!(e.step(DT, Some(120.0)));
    }

    #[test]
    fn exhaustion_never_fades_at_the_limit_or_without_erg() {
        let mut e = Exhaustion::new(100.0);
        for _ in 0..4800 {
            assert!(!e.step(DT, Some(100.0)));
            assert!(!e.step(DT, None));
        }
    }

    #[test]
    fn exhaustion_lasts_until_fully_recovered() {
        let mut e = Exhaustion::new(100.0);
        for _ in 0..480 {
            e.step(DT, Some(120.0));
        }
        // 120 s of strain recovers at twice the rate: 60 s, i.e. 240 steps.
        for _ in 0..239 {
            assert!(e.step(DT, Some(80.0)));
        }
        assert!(!e.step(DT, Some(80.0)));
        for _ in 0..479 {
            assert!(!e.step(DT, Some(120.0)));
        }
        assert!(e.step(DT, Some(120.0)));
    }
}
