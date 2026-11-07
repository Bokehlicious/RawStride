//! Memory and CPU counters (the four layers of P6c, ADR 0008 §17.2): process private bytes and
//! working set (L2), GPU memory per process and adapter (L3, PDH), and system commit (L4, control).
//! The sampler adds our own accounting (L1: LibRaw sessions, textures) and writes one row every 250 ms.
//! Copied from P6c (`sys` and `gpu`), without the WebView2 process kinds: P7 is one process.

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use crate::core::lock;

pub const MB: f64 = 1048576.0;

/// L1 values supplied by the app: (core sessions MB, texture MB, pending MB).
pub type L1 = Box<dyn Fn() -> (f64, f64, f64) + Send + Sync>;

pub const HEADER: &str = "t_ms,mark,app_priv,app_ws,tree_priv,proc_ded,proc_shared,proc_commit,adapter_ded,adapter_shared,sys_commit,sys_avail,core_sessions,tex,pending";

pub struct Sampler {
    pub rows: Mutex<Vec<String>>,
    pub last: Mutex<Vec<f64>>,
    pub mark: Mutex<String>,
    t0: Instant,
}

impl Sampler {
    pub fn start(l1: L1) -> Result<Arc<Sampler>, String> {
        let s = Arc::new(Sampler { rows: Mutex::new(Vec::new()), last: Mutex::new(Vec::new()), mark: Mutex::new("start".into()), t0: Instant::now() });
        let me = s.clone();
        std::thread::Builder::new()
            .name("p7-sampler".into())
            .spawn(move || me.run(l1))
            .map_err(|e| format!("spawn sampler: {e}"))?;
        Ok(s)
    }

    fn run(&self, l1: L1) {
        let mut q = gpu::Query::new();
        loop {
            let t = Instant::now();
            let own = sys::own_pid();
            let pids = sys::tree_pids();
            let (app_priv, app_ws) = sys::mem_mb(own).unwrap_or((f64::NAN, f64::NAN));
            let tree_priv: f64 = pids.iter().filter_map(|p| sys::mem_mb(*p)).map(|m| m.0).sum();
            let g = q.collect();
            let sum = |m: &std::collections::HashMap<u32, f64>| -> f64 { m.iter().filter(|(p, _)| pids.contains(p)).map(|(_, v)| *v).sum() };
            let (sess, tex, pend) = l1();
            let vals = vec![
                self.t0.elapsed().as_secs_f64() * 1000.0,
                app_priv,
                app_ws,
                tree_priv,
                sum(&g.ded),
                sum(&g.shared),
                sum(&g.commit),
                g.adapter_ded,
                g.adapter_shared,
                g.sys_commit,
                g.sys_avail,
                sess,
                tex,
                pend,
            ];
            let mark = lock(&self.mark).replace(',', ";");
            let mut row = format!("{:.1},{mark}", vals[0]);
            for v in &vals[1..] {
                row.push_str(&format!(",{v:.1}"));
            }
            lock(&self.rows).push(row);
            *lock(&self.last) = vals;
            std::thread::sleep(Duration::from_millis(250).saturating_sub(t.elapsed()));
        }
    }

    pub fn set_mark(&self, m: &str) {
        *lock(&self.mark) = m.to_string();
    }

    /// The latest row as (name, value) pairs, after waiting `settle` (PDH updates about once a second).
    pub fn snapshot(&self, label: &str, settle: Duration) -> Vec<(String, f64)> {
        self.set_mark(label);
        std::thread::sleep(settle);
        let names: Vec<&str> = HEADER.split(',').filter(|n| *n != "mark").collect();
        names.iter().map(|n| n.to_string()).zip(lock(&self.last).iter().copied()).collect()
    }

    pub fn csv(&self) -> String {
        let mut s = String::from(HEADER);
        s.push('\n');
        for r in lock(&self.rows).iter() {
            s.push_str(r);
            s.push('\n');
        }
        s
    }
}

pub fn cpu_ms() -> f64 {
    sys::cpu_ms(&sys::tree_pids())
}

/// Ends the process once the results are written. P7's first run showed that the process can stay
/// alive after the window has closed; the stderr markers say where it stopped. Measurement only.
pub fn hard_exit(code: u32, why: &str) {
    eprintln!("p7: hard exit ({why})");
    sys::terminate(code);
}

mod sys {
    use std::ffi::c_void;

    #[repr(C)]
    struct ProcessEntry {
        size: u32,
        usage: u32,
        pid: u32,
        heap_id: usize,
        module_id: u32,
        threads: u32,
        parent: u32,
        pri: i32,
        flags: u32,
        exe: [u16; 260],
    }
    #[repr(C)]
    #[derive(Default)]
    struct MemCountersEx {
        cb: u32,
        page_fault_count: u32,
        peak_working_set: usize,
        working_set: usize,
        quota_peak_paged_pool: usize,
        quota_paged_pool: usize,
        quota_peak_non_paged_pool: usize,
        quota_non_paged_pool: usize,
        pagefile_usage: usize,
        peak_pagefile_usage: usize,
        private_usage: usize,
    }
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn CreateToolhelp32Snapshot(flags: u32, pid: u32) -> *mut c_void;
        fn Process32FirstW(snap: *mut c_void, e: *mut ProcessEntry) -> i32;
        fn Process32NextW(snap: *mut c_void, e: *mut ProcessEntry) -> i32;
        fn OpenProcess(access: u32, inherit: i32, pid: u32) -> *mut c_void;
        fn CloseHandle(h: *mut c_void) -> i32;
        fn K32GetProcessMemoryInfo(p: *mut c_void, m: *mut MemCountersEx, cb: u32) -> i32;
        fn GetProcessTimes(p: *mut c_void, c: *mut u64, e: *mut u64, k: *mut u64, u: *mut u64) -> i32;
        fn GetCurrentProcessId() -> u32;
        fn GetCurrentProcess() -> *mut c_void;
        fn TerminateProcess(p: *mut c_void, code: u32) -> i32;
    }

    /// Ends this process at once, without DLL detach or exit handlers (see `super::hard_exit`).
    pub fn terminate(code: u32) {
        // SAFETY: the pseudo handle of the current process needs no closing.
        unsafe {
            TerminateProcess(GetCurrentProcess(), code);
        }
    }

    pub fn own_pid() -> u32 {
        // SAFETY: no arguments.
        unsafe { GetCurrentProcessId() }
    }

    /// This process and all its descendants (the WebView2 processes).
    pub fn tree_pids() -> Vec<u32> {
        // SAFETY: plain Win32 calls with valid out-pointers; handles are closed.
        unsafe {
            let snap = CreateToolhelp32Snapshot(2, 0);
            if snap.is_null() || snap as isize == -1 {
                return vec![own_pid()];
            }
            let mut all: Vec<(u32, u32)> = Vec::new();
            let mut e: ProcessEntry = std::mem::zeroed();
            e.size = std::mem::size_of::<ProcessEntry>() as u32;
            let mut ok = Process32FirstW(snap, &mut e);
            while ok != 0 {
                all.push((e.pid, e.parent));
                ok = Process32NextW(snap, &mut e);
            }
            CloseHandle(snap);
            let mut tree = vec![own_pid()];
            let mut i = 0;
            while i < tree.len() {
                let p = tree[i];
                for (pid, parent) in &all {
                    if *parent == p && !tree.contains(pid) {
                        tree.push(*pid);
                    }
                }
                i += 1;
            }
            tree
        }
    }

    /// (private bytes, working set) in MB.
    pub fn mem_mb(pid: u32) -> Option<(f64, f64)> {
        // SAFETY: plain Win32 calls with a valid out-struct; the handle is closed.
        unsafe {
            let h = OpenProcess(0x1000, 0, pid);
            if h.is_null() {
                return None;
            }
            let mut m = MemCountersEx { cb: std::mem::size_of::<MemCountersEx>() as u32, ..Default::default() };
            let ok = K32GetProcessMemoryInfo(h, &mut m, m.cb) != 0;
            CloseHandle(h);
            ok.then(|| (m.private_usage as f64 / super::MB, m.working_set as f64 / super::MB))
        }
    }

    /// Kernel + user CPU time of the given processes, in ms.
    pub fn cpu_ms(pids: &[u32]) -> f64 {
        let mut total = 0.0;
        for pid in pids {
            // SAFETY: plain Win32 calls with valid out-pointers; the handle is closed.
            unsafe {
                let h = OpenProcess(0x1000, 0, *pid);
                if h.is_null() {
                    continue;
                }
                let (mut c, mut e, mut k, mut u) = (0u64, 0u64, 0u64, 0u64);
                if GetProcessTimes(h, &mut c, &mut e, &mut k, &mut u) != 0 {
                    total += (k + u) as f64 / 10_000.0;
                }
                CloseHandle(h);
            }
        }
        total
    }
}

mod gpu {
    use std::collections::HashMap;

    #[link(name = "pdh")]
    unsafe extern "system" {
        fn PdhOpenQueryW(src: *const u16, user: usize, q: *mut isize) -> u32;
        fn PdhAddEnglishCounterW(q: isize, path: *const u16, user: usize, c: *mut isize) -> u32;
        fn PdhCollectQueryData(q: isize) -> u32;
        fn PdhGetFormattedCounterArrayW(c: isize, fmt: u32, size: *mut u32, count: *mut u32, buf: *mut u8) -> u32;
        fn PdhGetFormattedCounterValue(c: isize, fmt: u32, typ: *mut u32, v: *mut FmtValue) -> u32;
    }
    #[repr(C)]
    struct Item {
        name: *const u16,
        status: u32,
        _pad: u32,
        value: i64,
    }
    #[repr(C)]
    struct FmtValue {
        status: u32,
        _pad: u32,
        value: i64,
    }
    const PDH_FMT_LARGE: u32 = 0x400;
    const PDH_MORE_DATA: u32 = 0x8000_07D2;

    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(std::iter::once(0)).collect()
    }

    #[derive(Default)]
    pub struct Sample {
        pub ded: HashMap<u32, f64>,
        pub shared: HashMap<u32, f64>,
        pub commit: HashMap<u32, f64>,
        pub adapter_ded: f64,
        pub adapter_shared: f64,
        pub sys_commit: f64,
        pub sys_avail: f64,
    }

    /// A persistent PDH query; wildcard instances are expanded on every collection.
    pub struct Query {
        q: isize,
        c: [isize; 7],
    }

    impl Query {
        pub fn new() -> Query {
            let mut q = Query { q: 0, c: [0; 7] };
            let paths = [
                "\\GPU Process Memory(*)\\Dedicated Usage",
                "\\GPU Process Memory(*)\\Shared Usage",
                "\\GPU Process Memory(*)\\Total Committed",
                "\\GPU Adapter Memory(*)\\Dedicated Usage",
                "\\GPU Adapter Memory(*)\\Shared Usage",
                "\\Memory\\Committed Bytes",
                "\\Memory\\Available MBytes",
            ];
            // SAFETY: PDH calls with valid out-pointers; the query lives as long as the sampler.
            unsafe {
                if PdhOpenQueryW(std::ptr::null(), 0, &mut q.q) != 0 {
                    q.q = 0;
                    return q;
                }
                for (i, p) in paths.iter().enumerate() {
                    let w = wide(p);
                    PdhAddEnglishCounterW(q.q, w.as_ptr(), 0, &mut q.c[i]);
                }
            }
            q
        }

        fn array(&self, c: isize) -> Vec<(String, f64)> {
            let mut out = Vec::new();
            // SAFETY: buffer sized as PDH asks; item names point into it.
            unsafe {
                let (mut size, mut count) = (0u32, 0u32);
                if PdhGetFormattedCounterArrayW(c, PDH_FMT_LARGE, &mut size, &mut count, std::ptr::null_mut()) != PDH_MORE_DATA {
                    return out;
                }
                let mut buf = vec![0u8; size as usize + 64];
                if PdhGetFormattedCounterArrayW(c, PDH_FMT_LARGE, &mut size, &mut count, buf.as_mut_ptr()) != 0 {
                    return out;
                }
                let items = std::slice::from_raw_parts(buf.as_ptr() as *const Item, count as usize);
                for it in items {
                    let mut n = 0;
                    while *it.name.add(n) != 0 {
                        n += 1;
                    }
                    out.push((String::from_utf16_lossy(std::slice::from_raw_parts(it.name, n)), it.value as f64));
                }
            }
            out
        }

        fn single(&self, c: isize) -> f64 {
            let mut v = FmtValue { status: 0, _pad: 0, value: 0 };
            let mut t = 0u32;
            // SAFETY: valid counter handle and out-pointers.
            if unsafe { PdhGetFormattedCounterValue(c, PDH_FMT_LARGE, &mut t, &mut v) } != 0 {
                return f64::NAN;
            }
            v.value as f64
        }

        pub fn collect(&mut self) -> Sample {
            let mut s = Sample::default();
            if self.q == 0 {
                return s;
            }
            // SAFETY: valid query handle.
            unsafe { PdhCollectQueryData(self.q) };
            // instance names look like "pid_1234_luid_0x..._phys_0"
            let per_pid = |v: Vec<(String, f64)>| -> HashMap<u32, f64> {
                let mut m = HashMap::new();
                for (name, val) in v {
                    if let Some(pid) = name.strip_prefix("pid_").and_then(|r| r.split('_').next()).and_then(|p| p.parse::<u32>().ok()) {
                        *m.entry(pid).or_insert(0.0) += val / super::MB;
                    }
                }
                m
            };
            s.ded = per_pid(self.array(self.c[0]));
            s.shared = per_pid(self.array(self.c[1]));
            s.commit = per_pid(self.array(self.c[2]));
            s.adapter_ded = self.array(self.c[3]).iter().map(|x| x.1).sum::<f64>() / super::MB;
            s.adapter_shared = self.array(self.c[4]).iter().map(|x| x.1).sum::<f64>() / super::MB;
            s.sys_commit = self.single(self.c[5]) / super::MB;
            s.sys_avail = self.single(self.c[6]);
            s
        }
    }
}

