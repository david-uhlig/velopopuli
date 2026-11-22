//! Replays a heart-rate paced ride against a model of that rider's heart, so pacer
//! changes can be compared on a real day rather than on a made-up heart.
//!
//!     cargo run -p velopopuli-core --release --example hrpace_replay -- ride.fit [ftp] [max_hr]
//!     cargo run -p velopopuli-core --release --example hrpace_replay -- --sweep ride.fit > sweep.csv
//!
//! The ride must carry the pacing developer fields (`hr_band_low`, `hr_pace_state`, …), which rides
//! recorded in heart-rate mode have. FTP defaults to 190 W and max HR to 185. The first form prints
//! what happened on the day beside replays with different settings; `--sweep` writes one CSV row
//! per trend window (30–90 s) × edge gate (0–2 bpm) × ease wait (15, 30 s).
//!
//! The heart model is fitted to the ride: heart rate = a + b·fast(power) + c·slow(power) + d·minutes,
//! where fast and slow are first-order lags of power (after a dead time). What the model can't
//! explain from power (breathing, drinking, posture, strap noise) is kept as a residual and added
//! back in every replay, so each controller meets the same wiggles. The trainer is assumed to hold
//! the target with the same error it showed on the day.

use fitparser::profile::MesgNum;
use velopopuli_core::hrpace::{pace_start, HrPacer, PaceConfig, PaceInput, PaceState};

struct Ride {
    hr: Vec<f64>,
    power: Vec<f64>,
    cadence: Vec<Option<f64>>,
    target: Vec<Option<f64>>,
    low: f64,
    high: f64,
}

fn read(path: &str) -> Result<Ride, Box<dyn std::error::Error>> {
    let records = fitparser::from_bytes(&std::fs::read(path)?)?;
    let mut r = Ride { hr: vec![], power: vec![], cadence: vec![], target: vec![], low: 0.0, high: 0.0 };
    for rec in records.iter().filter(|x| x.kind() == MesgNum::Record) {
        let get = |name: &str| rec.fields().iter().find(|f| f.name() == name).and_then(|f| f.value().to_string().parse::<f64>().ok());
        let (Some(hr), Some(p)) = (get("heart_rate"), get("power")) else { continue };
        r.hr.push(hr);
        r.power.push(p);
        r.cadence.push(get("cadence"));
        r.target.push(get("target_power").filter(|v| *v < 65535.0));
        if let (Some(lo), Some(hi)) = (get("hr_band_low"), get("hr_band_high")) {
            (r.low, r.high) = (lo, hi);
        }
    }
    if r.low == 0.0 {
        return Err("the ride carries no heart-rate band (was it paced?)".into());
    }
    Ok(r)
}

#[derive(Clone, Copy, Debug)]
struct Heart {
    dead_s: usize,
    tau_fast: f64,
    tau_slow: f64,
    coef: [f64; 4],
}

impl Heart {
    /// Model heart rate for a whole power series.
    fn run(&self, power: &[f64]) -> Vec<f64> {
        let feats = features(power, self.dead_s, self.tau_fast, self.tau_slow);
        feats.iter().map(|f| (0..4).map(|k| self.coef[k] * f[k]).sum()).collect()
    }
}

/// Model inputs per second: constant, fast lag, slow lag, minutes.
fn features(power: &[f64], dead_s: usize, tau_fast: f64, tau_slow: f64) -> Vec<[f64; 4]> {
    let (af, as_) = (1.0 - (-1.0 / tau_fast).exp(), 1.0 - (-1.0 / tau_slow).exp());
    let (mut f, mut s) = (0.0, 0.0);
    (0..power.len())
        .map(|i| {
            let p = if i >= dead_s { power[i - dead_s] } else { 0.0 };
            f += (p - f) * af;
            s += (p - s) * as_;
            [1.0, f, s, i as f64 / 60.0]
        })
        .collect()
}

/// Solves the 4×4 normal equations by Gaussian elimination.
#[allow(clippy::needless_range_loop)]
fn least_squares(x: &[[f64; 4]], y: &[f64]) -> [f64; 4] {
    let mut a = [[0.0; 5]; 4];
    for (row, &v) in x.iter().zip(y) {
        for i in 0..4 {
            for j in 0..4 {
                a[i][j] += row[i] * row[j];
            }
            a[i][4] += row[i] * v;
        }
    }
    for c in 0..4 {
        let p = (c..4).max_by(|&i, &j| a[i][c].abs().total_cmp(&a[j][c].abs())).unwrap();
        a.swap(c, p);
        for r in 0..4 {
            if r != c {
                let k = a[r][c] / a[c][c];
                for j in c..5 {
                    a[r][j] -= k * a[c][j];
                }
            }
        }
    }
    [a[0][4] / a[0][0], a[1][4] / a[1][1], a[2][4] / a[2][2], a[3][4] / a[3][3]]
}

fn fit(ride: &Ride) -> (Heart, f64) {
    let mut best: Option<(Heart, f64)> = None;
    for dead_s in [0, 5, 10, 15, 20, 30] {
        for tau_fast in [10.0, 20.0, 30.0, 45.0, 60.0, 90.0, 120.0] {
            for tau_slow in [240.0, 360.0, 600.0, 900.0, 1200.0, 1800.0] {
                let x = features(&ride.power, dead_s, tau_fast, tau_slow);
                let coef = least_squares(&x, &ride.hr);
                let h = Heart { dead_s, tau_fast, tau_slow, coef };
                let rmse = (h.run(&ride.power).iter().zip(&ride.hr).map(|(m, y)| (m - y).powi(2)).sum::<f64>() / ride.hr.len() as f64).sqrt();
                if best.is_none_or(|(_, e)| rmse < e) {
                    best = Some((h, rmse));
                }
            }
        }
    }
    best.unwrap()
}

#[derive(Default)]
struct Stats {
    to_band_s: Option<usize>,
    warm_end_s: usize,
    in_band: f64,
    inner: f64,
    below: f64,
    above: f64,
    hr_mean: f64,
    hr_sd: f64,
    hr_max: f64,
    changes: usize,
    reversals: usize,
    tgt_sd: f64,
    tgt_min: f64,
    tgt_max: f64,
    mean_power: f64,
    mid_band_moves: usize,
}

fn average(v: &[Stats]) -> Stats {
    let n = v.len() as f64;
    let f = |g: &dyn Fn(&Stats) -> f64| v.iter().map(g).sum::<f64>() / n;
    let u = |g: &dyn Fn(&Stats) -> usize| (v.iter().map(g).sum::<usize>() as f64 / n).round() as usize;
    Stats {
        to_band_s: Some(u(&|s| s.to_band_s.unwrap_or(0))),
        warm_end_s: u(&|s| s.warm_end_s),
        in_band: f(&|s| s.in_band),
        inner: f(&|s| s.inner),
        below: f(&|s| s.below),
        above: f(&|s| s.above),
        hr_mean: f(&|s| s.hr_mean),
        hr_sd: f(&|s| s.hr_sd),
        hr_max: f(&|s| s.hr_max),
        changes: u(&|s| s.changes),
        reversals: u(&|s| s.reversals),
        tgt_sd: f(&|s| s.tgt_sd),
        tgt_min: f(&|s| s.tgt_min),
        tgt_max: f(&|s| s.tgt_max),
        mean_power: f(&|s| s.mean_power),
        mid_band_moves: u(&|s| s.mid_band_moves),
    }
}

fn mean_sd(v: &[f64]) -> (f64, f64) {
    let m = v.iter().sum::<f64>() / v.len() as f64;
    (m, (v.iter().map(|x| (x - m).powi(2)).sum::<f64>() / v.len() as f64).sqrt())
}

/// Rides the whole ride again with a pacer whose edge gate is `gate`; returns smoothed HR, target and state per second.
fn replay(ride: &Ride, heart: &Heart, residual: &[f64], cfg: &PaceConfig, gate: f64, window_s: f64, ease_wait_s: f64) -> (Vec<f64>, Vec<f64>, Vec<PaceState>) {
    let n = ride.hr.len();
    // How far the trainer's power sat from its target on the day, second by second.
    let err: Vec<f64> = (0..n).map(|i| ride.target[i.saturating_sub(1)].map_or(0.0, |t| ride.power[i] - t)).collect();
    let (start_w, warm) = pace_start(cfg, 0.0, None, &[]);
    let mut pacer = HrPacer::new(cfg, start_w, warm).with_edge_gate(gate).with_timing(window_s, ease_wait_s);
    let mut power = vec![0.0; n];
    let (mut hs, mut tgt, mut st) = (vec![0.0; n], vec![0.0; n], vec![PaceState::WarmUp; n]);
    let mut watts = cfg.ftp * 0.5;
    // The heart model is causal, so the series can be rebuilt one second at a time.
    for i in 0..n {
        power[i] = (watts + err[i]).max(0.0);
        let model = heart.run(&power[..=i]);
        let hr = (model[i] + residual[i]).round();
        let out = pacer.step(cfg, PaceInput { t: i as f64, hr: Some(hr), power: power[i], cadence: ride.cadence[i], paused: false });
        watts = out.watts;
        hs[i] = out.hr_smooth.unwrap_or(0.0);
        tgt[i] = out.watts;
        st[i] = out.state;
    }
    (hs, tgt, st)
}

fn stats(hs: &[f64], tgt: &[f64], st: &[PaceState], low: f64, high: f64) -> Stats {
    let mut s = Stats { warm_end_s: st.iter().position(|x| *x != PaceState::WarmUp).unwrap_or(st.len()), ..Default::default() };
    s.to_band_s = hs.iter().position(|h| *h >= low);
    let we = s.warm_end_s;
    let h = &hs[we..];
    let n = h.len() as f64;
    let (ilo, ihi) = (low + 3.0_f64.min((high - low) / 4.0), high - 3.0_f64.min((high - low) / 4.0));
    s.in_band = 100.0 * h.iter().filter(|x| **x >= low && **x <= high).count() as f64 / n;
    s.inner = 100.0 * h.iter().filter(|x| **x >= ilo && **x <= ihi).count() as f64 / n;
    s.below = 100.0 * h.iter().filter(|x| **x < low).count() as f64 / n;
    s.above = 100.0 * h.iter().filter(|x| **x > high).count() as f64 / n;
    (s.hr_mean, s.hr_sd) = mean_sd(h);
    s.hr_max = h.iter().cloned().fold(0.0, f64::max);
    let t = &tgt[we..];
    (_, s.tgt_sd) = mean_sd(t);
    s.tgt_min = t.iter().cloned().fold(f64::INFINITY, f64::min);
    s.tgt_max = t.iter().cloned().fold(0.0, f64::max);
    let mut last_dir = 0.0;
    for i in 1..t.len() {
        let d = t[i] - t[i - 1];
        if d.abs() > 0.5 {
            s.changes += 1;
            if last_dir != 0.0 && d.signum() != last_dir {
                s.reversals += 1;
            }
            last_dir = d.signum();
            if h[i] >= ilo && h[i] <= ihi {
                s.mid_band_moves += 1;
            }
        }
    }
    s.mean_power = t.iter().sum::<f64>() / t.len() as f64;
    s
}

fn row(name: &str, s: &Stats) {
    let mm = |v: usize| format!("{}:{:02}", v / 60, v % 60);
    println!(
        "{name:<30} {:>6} {:>6} {:>6.1} {:>6.1} {:>5.1}/{:<4.1} {:>6.1} {:>5.2} {:>5.0} {:>5} {:>5} {:>5} {:>5.1} {:>4.0}-{:<4.0} {:>5.0}",
        s.to_band_s.map_or("-".into(), mm),
        mm(s.warm_end_s),
        s.in_band,
        s.inner,
        s.below,
        s.above,
        s.hr_mean,
        s.hr_sd,
        s.hr_max,
        s.changes,
        s.reversals,
        s.mid_band_moves,
        s.tgt_sd,
        s.tgt_min,
        s.tgt_max,
        s.mean_power
    );
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let mut path = args.next().ok_or("usage: hrpace_replay [--sweep] <ride.fit> [ftp] [max_hr]")?;
    // --sweep prints one CSV row per trend window (30–90 s) × edge gate (0–2 bpm) × ease wait (15, 30 s).
    let sweep = path == "--sweep";
    if sweep {
        path = args.next().ok_or("usage: hrpace_replay --sweep <ride.fit> [ftp] [max_hr]")?;
    }
    let ftp: f64 = args.next().map_or(Ok(190.0), |v| v.parse())?;
    let max_hr: f64 = args.next().map_or(Ok(185.0), |v| v.parse())?;
    let ride = read(&path)?;
    let (heart, rmse) = fit(&ride);
    let model = heart.run(&ride.power);
    let residual: Vec<f64> = ride.hr.iter().zip(&model).map(|(y, m)| y - m).collect();
    println!("{path}");
    println!(
        "heart model: dead {} s, fast lag {} s, slow lag {} s, {:.1} + {:.3}·fast + {:.3}·slow + {:.3}/min; fit error {:.2} bpm rms",
        heart.dead_s, heart.tau_fast, heart.tau_slow, heart.coef[0], heart.coef[1], heart.coef[2], heart.coef[3], rmse
    );
    let cfg = PaceConfig { low: ride.low, high: ride.high, min_w: (0.4 * ftp).round(), max_w: (1.05 * ftp).round(), warmup_s: 840.0, ftp, max_hr };
    if sweep {
        let shifts = [0usize, 557, 1123, 1699, 2281, 2843, 3407];
        let keep = 900.min(residual.len());
        let shifted = |k: usize| -> Vec<f64> {
            let tail = &residual[keep..];
            let mut r = residual[..keep].to_vec();
            r.extend((0..tail.len()).map(|i| tail[(i + k) % tail.len()]));
            r
        };
        let ride_name = std::path::Path::new(&path).file_stem().map_or(path.clone(), |s| s.to_string_lossy().into_owned());
        println!("ride,window_s,gate_bpm,ease_wait_s,in_band,in_band_sd,inner,below,above,hr_mean,hr_sd,hr_max,changes,reversals,tgt_sd,tgt_min,avg_target");
        for ease_wait_s in [15.0, 30.0] {
            for w in (30..=90).step_by(5) {
                for g in [0.0, 0.5, 1.0, 1.5, 2.0] {
                    let all: Vec<Stats> = shifts
                        .iter()
                        .map(|&k| {
                            let (hs, tg, st) = replay(&ride, &heart, &shifted(k), &cfg, g, w as f64, ease_wait_s);
                            stats(&hs, &tg, &st, ride.low, ride.high)
                        })
                        .collect();
                    let a = average(&all);
                    let sd = (all.iter().map(|s| (s.in_band - a.in_band).powi(2)).sum::<f64>() / all.len() as f64).sqrt();
                    println!(
                        "{ride_name},{w},{g},{ease_wait_s},{:.2},{:.2},{:.2},{:.2},{:.2},{:.2},{:.3},{:.1},{},{},{:.2},{:.1},{:.1}",
                        a.in_band, sd, a.inner, a.below, a.above, a.hr_mean, a.hr_sd, a.hr_max, a.changes, a.reversals, a.tgt_sd, a.tgt_min, a.mean_power
                    );
                }
            }
        }
        return Ok(());
    }
    println!("pacer: band {}-{}, {}-{} W, FTP {}, max HR {}, warm-up {} min\n", cfg.low, cfg.high, cfg.min_w, cfg.max_w, cfg.ftp, cfg.max_hr, cfg.warmup_s / 60.0);
    println!(
        "{:<30} {:>6} {:>6} {:>6} {:>6} {:>10} {:>6} {:>5} {:>5} {:>5} {:>5} {:>5} {:>5} {:>9} {:>5}",
        "", "band@", "warm", "band%", "inner%", "below/abv", "HRavg", "HRsd", "HRmax", "moves", "rev", "midbd", "tgtSD", "target", "avgW"
    );
    println!("{}", "-".repeat(132));

    // What happened on the day, measured the same way.
    let pace: Vec<(f64, f64, PaceState)> = {
        let records = fitparser::from_bytes(&std::fs::read(&path)?)?;
        records
            .iter()
            .filter(|x| x.kind() == MesgNum::Record)
            .filter_map(|rec| {
                let get = |name: &str| rec.fields().iter().find(|f| f.name() == name).and_then(|f| f.value().to_string().parse::<f64>().ok());
                let state = match get("hr_pace_state")? as u8 {
                    0 => PaceState::WarmUp,
                    1 => PaceState::Holding,
                    2 => PaceState::Raising,
                    3 => PaceState::Easing,
                    4 => PaceState::Struggling,
                    _ => PaceState::NoHr,
                };
                get("heart_rate")?;
                Some((get("hr_smooth")?, get("target_power")?, state))
            })
            .collect()
    };
    let (hs, tg, st): (Vec<f64>, Vec<f64>, Vec<PaceState>) = (pace.iter().map(|p| p.0).collect(), pace.iter().map(|p| p.1).collect(), pace.iter().map(|p| p.2).collect());
    row("on the day (recorded)", &stats(&hs, &tg, &st, ride.low, ride.high));

    // Each variant meets the day's wiggles at several offsets (the warm-up's own stay put), and the results are averaged.
    let shifts = [0usize, 557, 1123, 1699, 2281, 2843, 3407];
    let shifted = |k: usize| -> Vec<f64> {
        let keep = 900.min(residual.len());
        let tail = &residual[keep..];
        let mut r = residual[..keep].to_vec();
        r.extend((0..tail.len()).map(|i| tail[(i + k) % tail.len()]));
        r
    };
    let gates = [("none", f64::INFINITY), ("2.0", 2.0), ("1.5", 1.5), ("1.0", 1.0), ("0.5", 0.5), ("0", 0.0)];
    for window_s in [45.0, 90.0] {
        for ease_wait_s in [15.0, 30.0] {
            for (gname, gate) in gates {
                let all: Vec<Stats> = shifts
                    .iter()
                    .map(|&k| {
                        let (hs, tg, st) = replay(&ride, &heart, &shifted(k), &cfg, gate, window_s, ease_wait_s);
                        stats(&hs, &tg, &st, ride.low, ride.high)
                    })
                    .collect();
                row(&format!("trend {window_s:.0}s ease {ease_wait_s:.0}s gate {gname}"), &average(&all));
            }
        }
        println!();
    }
    println!("band@ first second in the band · warm = warm-up end · band%/inner%/below/abv after the warm-up · moves = target changes,");
    println!("rev = direction reversals, midbd = moves made with smoothed HR inside the inner band · avgW = mean target after the warm-up");
    Ok(())
}
