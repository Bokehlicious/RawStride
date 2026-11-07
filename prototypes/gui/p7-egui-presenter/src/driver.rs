//! The simulated user and the measurement suites. Runs on its own thread; the UI only draws.
//!
//! - `p5`: P5's RAW-mode scenarios (phase 1 `p4.rs` `run_all`: S1, S2, S4, S6, S5, NAT, MIX) with the
//!   same pauses, positions and random sequence (as P6c). The JPEG-mode scenarios and S3 are not run;
//!   their positions are kept, so NAT and MIX use the same images as P5.
//! - `stress`: M1 long stepping at the limit, M2 zoom-evict-return, M3 a tiny texture budget at
//!   10 images/s, M4 late loads, M5 400 steps for drift and release (as P6c).
//! - `m0`: textures of known size allocated and deleted in steps, to see what each memory counter
//!   counts under eframe's renderer (as P6c M0).

use std::collections::VecDeque;
use std::fmt::Write as _;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use crate::core::{Backend, Core, Kind, lock};
use crate::mem::{self, Sampler};

/// Work the driver asks the UI thread to do (textures can only be made there).
#[derive(Clone, Copy, Debug)]
pub enum UiCmd {
    /// Allocate textures until `n` exist, each `w` x `h` RGBA.
    M0Alloc { n: usize, w: usize, h: usize },
    /// Drop textures until `n` remain.
    M0Keep { n: usize },
    Close,
}

/// Shared between the driver and the UI.
#[derive(Default)]
pub struct UiShared {
    pub cmds: Mutex<VecDeque<UiCmd>>,
    pub done: AtomicU64,
    pub upload_ms: Mutex<Vec<f64>>,
    pub frame_ms: Mutex<Vec<f64>>,
    pub too_large: AtomicU64,
    pub max_texture_side: AtomicU64,
    pub frames: AtomicU64,
}

pub struct Config {
    pub suite: String,
    pub out: PathBuf,
    pub tag: String,
    pub renderer: String,
    pub tex_mb: usize,
    pub core_mb: usize,
}

pub struct Driver<B: Backend> {
    pub core: Arc<Core<B>>,
    pub sampler: Arc<Sampler>,
    pub ui: Arc<UiShared>,
    pub wake: Box<dyn Fn() + Send + Sync>,
    pub cfg: Config,
}

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    fn range(&mut self, lo: u64, hi: u64) -> u64 {
        lo + self.next() % (hi - lo + 1)
    }
}

fn sleep(ms: u64) {
    std::thread::sleep(Duration::from_millis(ms));
}

impl<B: Backend> Driver<B> {
    pub fn run(self) {
        let t0 = Instant::now();
        let cpu0 = mem::cpu_ms();
        let mut summary = String::new();
        sleep(1500); // let the window settle
        let snap0 = self.sampler.snapshot("start", Duration::from_millis(2500));
        let mut snaps = vec![("start".to_string(), snap0)];
        let mut extra = String::new();
        match self.cfg.suite.as_str() {
            "m0" => self.m0(&mut extra),
            "p5" => self.p5(),
            "stress" => self.stress(&mut extra, &mut snaps),
            other => extra.push_str(&format!("error=unknown suite {other}\n")),
        }
        snaps.push(("end".into(), self.sampler.snapshot("end", Duration::from_millis(3000))));
        let wall = t0.elapsed().as_secs_f64() * 1000.0;
        let cpu = mem::cpu_ms() - cpu0;
        self.write_results(&mut summary, &extra, &snaps, wall, cpu);
        lock(&self.ui.cmds).push_back(UiCmd::Close);
        (self.wake)();
        eprintln!("p7: results written, close sent");
        // watchdog: the results are on disk; do not let a stuck shutdown hold the run
        sleep(20_000);
        mem::hard_exit(0, "watchdog 20 s after close");
    }

    fn reset(&self, scen: &str) {
        self.core.reset(scen);
        self.sampler.set_mark(scen);
    }

    fn wait(&self, ms: u64) {
        self.core.wait_ready(Duration::from_millis(ms));
    }

    // ---------- P5 scenarios (RAW mode) ----------

    fn p5(&self) {
        let c = &self.core;
        let settle = 2000;
        let mut pos = 0usize;
        let mut fresh = |n: usize| {
            let p = pos;
            pos += n;
            p
        };
        // S1: next image, then Z after 0 / 50 / 150 ms
        self.reset("S1");
        let mut p = fresh(40);
        c.go(p, "start");
        self.wait(3000);
        sleep(settle);
        for w in [0u64, 50, 150] {
            for _ in 0..3 {
                p += 1;
                c.go(p, &format!("raw fit change (z after {w})"));
                sleep(w);
                c.zoom(true, &format!("raw zoom {w} ms after change"));
                self.wait(3000);
                sleep(300);
                c.zoom(false, "raw zoom out");
                sleep(1000);
            }
        }
        // S2: rapid stepping at 4, 7, 10 images/s, at fit and at 100 %
        for zoomed in [false, true] {
            for rate in [4u64, 7, 10] {
                self.reset("S2");
                let mut q = fresh(30);
                c.go(q, "start");
                if zoomed {
                    c.zoom(true, "start");
                }
                self.wait(3000);
                sleep(settle);
                let z = if zoomed { "100%" } else { "fit" };
                for _ in 0..20 {
                    q += 1;
                    c.go(q, &format!("raw {z} step {rate}/s"));
                    sleep(1000 / rate);
                }
                self.wait(3000);
            }
        }
        // S4: cold first display, then Z
        for _ in 0..3 {
            self.reset("S4");
            let q = fresh(5);
            c.go(q, "raw cold first view");
            self.wait(5000);
            c.zoom(true, "raw cold zoom after first view");
            self.wait(5000);
            sleep(500);
        }
        // S6: Z after a pause of 1, 3, 5 s
        self.reset("S6");
        p = fresh(20);
        c.go(p, "start");
        self.wait(3000);
        sleep(settle);
        for pause in [1000u64, 3000, 5000] {
            for _ in 0..2 {
                p += 1;
                c.go(p, &format!("raw fit change (z after {pause})"));
                sleep(pause);
                c.zoom(true, &format!("raw zoom after {} s pause", pause / 1000));
                self.wait(3000);
                sleep(300);
                c.zoom(false, "raw zoom out");
                sleep(300);
            }
        }
        // S5: back to an image after 1-5 steps
        self.reset("S5");
        let p0 = fresh(30);
        c.go(p0, "start");
        self.wait(3000);
        sleep(settle);
        for (base, k) in (p0..).zip(1..=5usize) {
            for i in 1..=k {
                c.go(base + i, "raw fit step");
                sleep(500);
            }
            for i in (0..k).rev() {
                let label = if i == 0 { format!("raw fit back after {k} steps") } else { "raw fit step back".to_string() };
                c.go(base + i, &label);
                sleep(500);
            }
            c.go(base + 1, "raw fit step");
            sleep(800);
        }
        // JPEG-mode scenarios and S3 are not run; their positions are kept (as P6c)
        fresh(40 + 180 + 15 + 20 + 30);
        fresh(40);
        // NAT
        self.reset("NAT");
        let mut p = fresh(100) + 5;
        c.go(p, "start");
        self.wait(3000);
        sleep(settle);
        let mut rng = Rng(0x9E37_79B9_7F4A_7C15);
        for _ in 0..50 {
            p = if rng.range(0, 9) == 0 { p.saturating_sub(1) } else { p + 1 };
            c.go(p, "raw change");
            let dwell = rng.range(150, 1500);
            if rng.range(0, 4) == 0 {
                let before = rng.range(150, dwell.max(151));
                sleep(before);
                c.zoom(true, "raw zoom");
                sleep(rng.range(1000, 3000));
                c.zoom(false, "raw zoom out");
                sleep(dwell - before.min(dwell));
            } else {
                sleep(dwell);
            }
        }
        // MIX: mode switches are logged, the view stays in RAW mode (only "mix raw ..." compares to P5)
        self.reset("MIX");
        let lo = fresh(200) + 50;
        let hi = lo + 11;
        c.go(lo, "start");
        self.wait(3000);
        sleep(settle);
        let mut p = lo;
        let mut dir: isize = 1;
        let mut mode = "raw";
        for _ in 0..150 {
            let r = rng.range(0, 99);
            if r < 60 {
                if (dir > 0 && p >= hi) || (dir < 0 && p <= lo) {
                    dir = -dir;
                }
                p = p.saturating_add_signed(dir);
                c.go(p, &format!("mix {mode} change"));
            } else if r < 75 {
                let j: isize = if rng.range(0, 1) == 0 { -3 } else { 3 };
                p = p.saturating_add_signed(j).clamp(lo, hi);
                c.go(p, &format!("mix {mode} jump"));
            } else if r < 90 {
                c.zoom(true, &format!("mix {mode} zoom"));
                sleep(rng.range(500, 2000));
                c.zoom(false, &format!("mix {mode} zoom out"));
            } else {
                mode = if mode == "raw" { "jpeg" } else { "raw" };
                self.core.state().request(&format!("mix to {mode}"));
            }
            sleep(rng.range(150, 1200));
        }
        self.wait(3000);
    }

    // ---------- stress: M1-M5 ----------

    fn stress(&self, extra: &mut String, snaps: &mut Vec<(String, Vec<(String, f64)>)>) {
        let c = &self.core;
        // M1: 100 steps forward at the budget limit
        self.reset("M1");
        let mut p = 10_000;
        c.go(p, "start");
        self.wait(5000);
        for _ in 0..100 {
            p += 1;
            c.go(p, "m1 step");
            self.wait(3000);
            sleep(600);
        }
        snaps.push(("M1 end".into(), self.sampler.snapshot("M1 end", Duration::from_millis(2500))));
        // M2: zoom to a point, step away until the image is evicted, come back
        for rep in 0..5usize {
            self.reset("M2");
            let base = 20_000 + rep * 100;
            c.go(base, "start");
            self.wait(5000);
            c.state().center = [0.3, 0.4];
            c.zoom(true, "m2 zoom");
            self.wait(5000);
            sleep(300);
            let h0 = c.state().serving(base, Kind::Zoom).map(|r| r.hash);
            let mut steps = 0;
            for k in 1..=30 {
                c.go(base + k, "m2 away");
                self.wait(3000);
                sleep(400);
                steps = k;
                if !c.state().resident.iter().any(|r| r.pos == base) {
                    break;
                }
            }
            let evicted = !c.state().resident.iter().any(|r| r.pos == base);
            c.go(base, "m2 return");
            self.wait(5000);
            sleep(200);
            let g = c.state();
            let rec = g.open.and_then(|i| g.recs.get(i)).cloned();
            let h1 = g.serving(base, Kind::Zoom).map(|r| r.hash);
            let lat = rec.as_ref().and_then(|r| r.shown.map(|s| (s - r.t).as_secs_f64() * 1000.0));
            let _ = writeln!(
                extra,
                "m2.{rep}=steps_away {steps}; evicted {evicted}; hit {}; latency_ms {}; zoom_kept {}; position_kept {}; same_pixels {}",
                rec.as_ref().is_some_and(|r| r.hit),
                lat.map(|l| format!("{l:.0}")).unwrap_or_else(|| "-".into()),
                g.zoomed,
                g.center == [0.3, 0.4],
                h0.is_some() && h0 == h1
            );
            drop(g);
        }
        c.state().center = [0.5, 0.5];
        // M3: a tiny texture budget (about 2 A7R fit views), 10 images/s, fit and 100 %
        let full = c.state().tex_budget;
        for zoomed in [false, true] {
            self.reset("M3");
            {
                let mut g = c.state();
                g.tex_budget = 130 << 20;
                g.stats.peak_tex_bytes = 0;
                g.stats.overshoot_max_bytes = i64::MIN;
            }
            let mut q = 30_000 + if zoomed { 1000 } else { 0 };
            c.go(q, "start");
            if zoomed {
                c.zoom(true, "start");
            }
            self.wait(5000);
            for _ in 0..40 {
                q += 1;
                c.go(q, &format!("m3 10/s {}", if zoomed { "100%" } else { "fit" }));
                sleep(100);
            }
            self.wait(5000);
            sleep(1000);
            let g = c.state();
            let _ = writeln!(
                extra,
                "m3.{}=budget_mb 130; peak_tex_mb {:.0}; overshoot_max_mb {:.0}; discarded {}",
                if zoomed { "zoomed" } else { "fit" },
                g.stats.peak_tex_bytes as f64 / mem::MB,
                g.stats.overshoot_max_bytes as f64 / mem::MB,
                g.stats.discarded
            );
            drop(g);
            c.state().tex_budget = full;
        }
        // M4: loads that finish after the user moved on
        self.reset("M4");
        let d0 = c.state().stats.discarded;
        let mut q = 40_000;
        c.go(q, "start");
        self.wait(5000);
        sleep(1000);
        for _ in 0..5 {
            q += 1;
            c.go(q, "m4 skip");
            sleep(100);
        }
        sleep(4000);
        {
            let g = c.state();
            let _ = writeln!(
                extra,
                "m4=discarded {}; tex_mb {:.0}; budget_mb {:.0}",
                g.stats.discarded - d0,
                g.tex_bytes as f64 / mem::MB,
                g.tex_budget as f64 / mem::MB
            );
        }
        // M5: 400 steps every 500 ms at the limit: drift and release
        self.reset("M5");
        snaps.push(("M5 start".into(), self.sampler.snapshot("M5 start", Duration::from_millis(2500))));
        let mut q = 50_000;
        c.go(q, "start");
        self.wait(5000);
        for i in 0..400 {
            q += 1;
            c.go(q, "m5 step");
            sleep(500);
            if i % 50 == 49 {
                self.sampler.set_mark(&format!("M5 step {}", i + 1));
            }
        }
        self.wait(5000);
        snaps.push(("M5 end".into(), self.sampler.snapshot("M5 end", Duration::from_millis(2500))));
        self.reset("M5 cleared");
        snaps.push(("M5 cleared +3 s".into(), self.sampler.snapshot("M5 cleared +3 s", Duration::from_millis(3000))));
        snaps.push(("M5 cleared +13 s".into(), self.sampler.snapshot("M5 cleared +13 s", Duration::from_millis(10_000))));
    }

    // ---------- M0: what textures cost in each counter ----------

    fn ui_cmd(&self, cmd: UiCmd) {
        let before = self.ui.done.load(Ordering::SeqCst);
        lock(&self.ui.cmds).push_back(cmd);
        (self.wake)();
        let end = Instant::now() + Duration::from_secs(60);
        while self.ui.done.load(Ordering::SeqCst) == before && Instant::now() < end {
            (self.wake)();
            sleep(5);
        }
        sleep(100); // one more frame, so the upload is submitted to the GPU
        (self.wake)();
    }

    fn m0(&self, extra: &mut String) {
        // the A7R half-size fit view (4784 x 3188 RGBA, 58 MiB)
        let (w, h) = (4784usize, 3188usize);
        let one = (w * h * 4) as f64 / mem::MB;
        let _ = writeln!(extra, "m0.image={w}x{h}; mb {one:.1}");
        let mut snap = |label: String, n: usize, settle: u64| {
            let s = self.sampler.snapshot(&label, Duration::from_millis(settle));
            let vals: Vec<String> = s.iter().map(|(k, v)| format!("{k} {v:.1}")).collect();
            let _ = writeln!(extra, "m0.{label}=n {n}; tex_mb {:.1}; {}", n as f64 * one, vals.join("; "));
        };
        snap("base".into(), 0, 3000);
        for n in [4usize, 8, 16, 24, 32] {
            self.ui_cmd(UiCmd::M0Alloc { n, w, h });
            snap(format!("upload {n}"), n, 3000);
        }
        for n in [24usize, 16, 8, 4, 0] {
            self.ui_cmd(UiCmd::M0Keep { n });
            snap(format!("delete {n}"), n, 3000);
        }
        snap("after 10 s".into(), 0, 10_000);
    }

    // ---------- results ----------

    fn write_results(&self, summary: &mut String, extra: &str, snaps: &[(String, Vec<(String, f64)>)], wall: f64, cpu: f64) {
        let g = self.core.state();
        let st = &g.stats;
        let q = |v: &[f64]| -> String {
            if v.is_empty() {
                return "-".into();
            }
            let mut s = v.to_vec();
            s.sort_by(|a, b| a.total_cmp(b));
            let at = |p: f64| s[((s.len() as f64 * p).ceil() as usize).clamp(1, s.len()) - 1];
            format!("n {} median {:.1} p95 {:.1} max {:.1}", s.len(), at(0.5), at(0.95), at(1.0))
        };
        let _ = writeln!(summary, "tag={}", self.cfg.tag);
        let _ = writeln!(summary, "suite={}", self.cfg.suite);
        let _ = writeln!(summary, "renderer={}", self.cfg.renderer);
        let _ = writeln!(summary, "tex_budget_mb={}", self.cfg.tex_mb);
        let _ = writeln!(summary, "core_budget_mb={}", self.cfg.core_mb);
        let _ = writeln!(summary, "max_texture_side={}", self.ui.max_texture_side.load(Ordering::SeqCst));
        let _ = writeln!(summary, "wall_ms={wall:.0}");
        let _ = writeln!(summary, "cpu_ms={cpu:.0}");
        let _ = writeln!(summary, "avg_cores={:.2}", if wall > 0.0 { cpu / wall } else { 0.0 });
        let _ = writeln!(summary, "frames={}", self.ui.frames.load(Ordering::SeqCst));
        let _ = writeln!(summary, "frame_ms={}", q(&lock(&self.ui.frame_ms)));
        let _ = writeln!(summary, "upload_ms={}", q(&lock(&self.ui.upload_ms)));
        let _ = writeln!(summary, "too_large={}", self.ui.too_large.load(Ordering::SeqCst));
        let _ = writeln!(summary, "unpacks={} unpack_ms={}", st.unpacks, q(&st.unpack_ms));
        for k in ["Fit", "Zoom", "Ring"] {
            let _ = writeln!(summary, "render_{k}_ms={}", q(st.render_ms.get(k).map(|v| v.as_slice()).unwrap_or(&[])));
        }
        let _ = writeln!(
            summary,
            "renders={} failures={} refused={} discarded={} evicted_textures={} evicted_sessions={}",
            st.renders, st.failures, st.refused, st.discarded, st.evicted_textures, st.evicted_sessions
        );
        let _ = writeln!(
            summary,
            "peak_tex_mb={:.0} peak_sessions_mb={:.0} overshoot_max_mb={:.0}",
            st.peak_tex_bytes as f64 / mem::MB,
            st.peak_sess_bytes as f64 / mem::MB,
            st.overshoot_max_bytes as f64 / mem::MB
        );
        if !st.last_error.is_empty() {
            let _ = writeln!(summary, "last_error={}", st.last_error.replace('\n', " "));
        }
        for (label, s) in snaps {
            let vals: Vec<String> = s.iter().map(|(k, v)| format!("{k} {v:.1}")).collect();
            let _ = writeln!(summary, "snap.{label}={}", vals.join("; "));
        }
        summary.push_str(extra);
        // events in P5's format, plus the drawn time and the miss cause
        let strat = format!("egui-{}@{}", self.cfg.renderer, self.cfg.tex_mb);
        let mut ev = String::from("group,strategy,scenario,label,pos,view,t_req,latency,shown,hit,on_screen,drawn,miss\n");
        let t0 = g.recs.first().map(|r| r.t);
        for r in &g.recs {
            let shown = r.shown.filter(|s| r.left.is_none_or(|l| *s <= l));
            let ms = |d: Duration| d.as_secs_f64() * 1000.0;
            let lat = shown.map(|s| format!("{:.1}", ms(s - r.t))).unwrap_or_default();
            let on_screen = r.left.map(|l| format!("{:.1}", ms(l - r.t))).unwrap_or_else(|| "NaN".into());
            let t_req = t0.map(|t| ms(r.t - t)).unwrap_or(0.0);
            let _ = writeln!(
                ev,
                "{},{strat},{},{},{},{},{t_req:.1},{lat},{},{},{on_screen},{lat},{}",
                self.cfg.tag,
                r.scen,
                r.label.replace(',', ";"),
                r.pos,
                if r.zoomed { "Zoom" } else { "Fit" },
                u8::from(shown.is_some()),
                u8::from(r.hit),
                r.miss
            );
        }
        drop(g);
        let write = |name: String, text: &str| {
            let p = self.cfg.out.join(name);
            if let Err(e) = std::fs::write(&p, text) {
                eprintln!("p7: {}: {e}", p.display());
            }
        };
        write(format!("{}.txt", self.cfg.tag), summary);
        if self.cfg.suite != "m0" {
            write(format!("{}-events.csv", self.cfg.tag), &ev);
        }
        write(format!("{}-mem.csv", self.cfg.tag), &self.sampler.csv());
    }
}
