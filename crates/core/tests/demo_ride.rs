//! A demo ride in heart-rate mode: the pacer against the demo rider's heart,
//! stepped the way the app does (the demo rider at 4 Hz, the pacer once a second).

use velopopuli_core::demo::{DemoHeart, Exhaustion, FADE_CADENCE, FADE_POWER, REST_BPM};
use velopopuli_core::hrpace::{pace_start, HrPacer, PaceConfig, PaceInput, PaceOutput, PaceState, START_POWER_S};
use velopopuli_core::types::{RiderSettings, TrainerSettings};

/// The demo loop's step, as in `src-tauri/src/devices/sim.rs`.
const DT: f64 = 0.25;

/// The band, power limits and warm-up the app works out for the default rider (FTP 200, max HR 185).
fn cfg() -> PaceConfig {
    TrainerSettings::default().pace_config(&RiderSettings::default())
}

/// What the rider does in one second; scripts change the defaults.
struct Tick {
    paused: bool,
    /// The rider has switched to ERG.
    erg: bool,
}

struct Sample {
    t: f64,
    hr: f64,
    drift: f64,
    cadence: f64,
    /// The trainer's target this second (from the pacer, or the rider's ERG watts).
    watts: f64,
    /// The pacer's output, while heart-rate mode is on.
    out: Option<PaceOutput>,
}

/// Rides `secs` seconds from the start of a ride in heart-rate mode.
fn ride(secs: usize, mut tired: Option<Exhaustion>, mut script: impl FnMut(f64, &mut PaceConfig, &mut Tick)) -> Vec<Sample> {
    let mut cfg = cfg();
    let mut heart = DemoHeart::new(REST_BPM);
    let mut pacer: Option<HrPacer> = None;
    let mut target: Option<f64> = None;
    let mut erg_w: Option<f64> = None;
    let mut recent: Vec<f64> = Vec::new();
    let mut phase = 0.0f64;
    let mut log = Vec::with_capacity(secs);
    for i in 1..=secs {
        let t = i as f64;
        let mut tick = Tick { paused: false, erg: false };
        script(t, &mut cfg, &mut tick);
        // Four demo-rider steps, each riding the target set at the last command.
        let (mut power, mut cadence, mut hr) = (0.0, 0.0, 0.0);
        for _ in 0..4 {
            phase += DT;
            let wobble = 1.0 + 0.04 * (phase * 0.9).sin() + 0.03 * (phase * 2.3).sin();
            let fading = tired.as_mut().is_some_and(|e| e.step(DT, target));
            power = target.unwrap_or(190.0) * wobble;
            cadence = 88.0 + 2.0 * (phase * 0.5).sin();
            if fading {
                power *= FADE_POWER;
                cadence = FADE_CADENCE + 2.0 * (phase * 0.5).sin();
            }
            hr = heart.step(DT, power).round();
        }
        recent.push(power);
        if recent.len() > START_POWER_S {
            recent.remove(0);
        }
        let (watts, out) = if tick.erg {
            pacer = None;
            // Switching to ERG starts from the last target, rounded to 10 W, as the ride screen does.
            let w = *erg_w.get_or_insert_with(|| (target.unwrap_or(0.0) / 10.0).round().clamp(5.0, 100.0) * 10.0);
            (w, None)
        } else {
            erg_w = None;
            let p = pacer.get_or_insert_with(|| {
                let (start_w, warm_up) = pace_start(&cfg, t, Some(hr), &recent);
                HrPacer::new(&cfg, start_w, warm_up)
            });
            let out = p.step(&cfg, PaceInput { t, hr: Some(hr), power, cadence: Some(cadence), paused: tick.paused });
            (out.watts.round(), Some(out))
        };
        target = Some(watts);
        log.push(Sample { t, hr, drift: heart.drift(), cadence, watts, out });
    }
    log
}

fn at(s: &Sample) -> String {
    format!("t={} hr={} drift={:.1} cadence={:.0} watts={} out={:?}", s.t, s.hr, s.drift, s.cadence, s.watts, s.out.map(|o| (o.state, o.warning)))
}

fn state(s: &Sample) -> PaceState {
    s.out.expect("heart-rate mode is on").state
}

/// Second `t` of the ride (the log starts at t = 1).
fn sec(log: &[Sample], t: usize) -> &Sample {
    &log[t - 1]
}

#[test]
fn default_rider_warms_up_settles_in_the_band_and_drifts() {
    let log = ride(1800, None, |_, _, _| {});
    assert_eq!(state(sec(&log, 1)), PaceState::WarmUp, "{}", at(sec(&log, 1)));
    let settled = log.iter().find(|s| state(s) != PaceState::WarmUp).unwrap();
    assert!(settled.t <= 300.0, "still warming up: {}", at(settled));
    for s in &log[299..] {
        assert!((120.0..=139.0).contains(&s.hr), "out of band: {}", at(s));
        assert_eq!(s.out.unwrap().warning, None, "{}", at(s));
        assert!((80.0..=210.0).contains(&s.watts), "{}", at(s));
    }
    let (early, late) = (sec(&log, 300), sec(&log, 1800));
    assert!(late.drift >= 7.0, "too little drift: {}", at(late));
    assert!(late.hr >= early.hr + 4.0, "drift did not raise the heart rate: {} (from {})", at(late), at(early));
}

#[test]
fn an_hour_of_drift_lowers_the_power() {
    let log = ride(3600, None, |_, _, _| {});
    for s in &log[599..] {
        assert!((120.0..=139.0).contains(&s.hr), "out of band: {}", at(s));
    }
    let (mid, end) = (sec(&log, 1200), sec(&log, 3600));
    assert!(end.watts < mid.watts, "power did not ease with drift: {} (from {})", at(end), at(mid));
}

#[test]
fn band_change_pause_and_erg_and_back() {
    let log = ride(1800, None, |t, cfg, tick| {
        if t >= 900.0 {
            cfg.low = 130.0;
            cfg.high = 149.0;
        }
        tick.paused = (1200.0..1260.0).contains(&t);
        tick.erg = (1400.0..1500.0).contains(&t);
    });
    // A higher band: the pacer raises the power and the heart rate follows within four minutes.
    assert!(log[899..1140].iter().any(|s| s.hr >= 130.0), "never reached the new band: {}", at(sec(&log, 1140)));
    assert!(sec(&log, 1199).watts > sec(&log, 899).watts, "{} (from {})", at(sec(&log, 1199)), at(sec(&log, 899)));
    // Paused: the target holds.
    for s in &log[1199..1259] {
        assert_eq!(s.watts, sec(&log, 1199).watts, "watts moved while paused: {}", at(s));
    }
    // ERG holds the last target rounded to 10 W.
    let erg = (sec(&log, 1399).watts / 10.0).round() * 10.0;
    for s in &log[1399..1499] {
        assert!(s.out.is_none(), "{}", at(s));
        assert_eq!(s.watts, erg, "{}", at(s));
    }
    // Back to heart-rate mode mid-ride: no warm-up, starting near the ERG watts, and the band holds.
    let back = sec(&log, 1500);
    assert_ne!(state(back), PaceState::WarmUp, "{}", at(back));
    assert!((back.watts - erg).abs() <= 15.0, "{}", at(back));
    for s in &log[1499..] {
        assert!((130.0..=149.0).contains(&s.hr), "out of band: {}", at(s));
        assert_eq!(s.out.unwrap().warning, None, "{}", at(s));
    }
}

#[test]
fn an_exhausted_rider_struggles_then_recovers() {
    let log = ride(1200, Some(Exhaustion::new(100.0)), |_, _, _| {});
    let first = log.iter().position(|s| state(s) == PaceState::Struggling).expect("never struggled");
    assert!((120..=300).contains(&first), "struggled at the wrong time: {}", at(&log[first]));
    assert!(log[first - 4..=first].iter().all(|s| s.cadence < 60.0), "struggled without low cadence: {}", at(&log[first]));
    let mut episodes = 0;
    for w in log.windows(2) {
        if state(&w[1]) == PaceState::Struggling {
            assert!(w[1].watts <= w[0].watts, "raised while struggling: {}", at(&w[1]));
            if state(&w[0]) != PaceState::Struggling {
                episodes += 1;
            }
        }
        assert!(w[1].watts >= 80.0, "below the lowest power: {}", at(&w[1]));
    }
    assert!(episodes >= 2, "only {episodes} struggles");
    let recovered = log[first..].iter().find(|s| state(s) != PaceState::Struggling).expect("never recovered");
    assert!(recovered.cadence >= 70.0, "{}", at(recovered));
}
