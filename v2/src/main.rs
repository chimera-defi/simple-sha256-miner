use clap::Parser;
use std::io::Read;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use vanity_miner::{
    state::{Intervals, Store},
    *,
};

/// Search for a nonce whose encoded digest starts with PREFIX, or verify a proof (--verify).
#[derive(Parser)]
struct Cli {
    #[arg(long)]
    prefix: Option<String>,
    /// Text hashed as "<input>:<nonce>".
    #[arg(long, alias = "seed", default_value = "gensys")]
    input: String,
    #[arg(long, value_enum, default_value = "sha256")]
    algorithm: Algo,
    #[arg(long, value_enum, default_value = "base64")]
    encoding: Enc,
    /// SHA-256 engine: lib (default) or avx2 (opt-in, x86-64 only, sha256 only, short input).
    #[arg(long, value_enum, default_value = "lib")]
    backend: Backend,
    #[arg(long, default_value_t = 6)]
    threads: usize,
    #[arg(long, default_value = "state")]
    state: PathBuf,
    #[arg(long, default_value_t = 1 << 18)]
    chunk: u64,
    /// Lowest nonce to search (multiple of --chunk). Lower nonces are never covered.
    #[arg(long, default_value_t = 0)]
    start_nonce: u64,
    #[arg(long, default_value_t = u64::MAX)]
    max_secs: u64,
    /// Verify a proof: JSON on stdin, or with --nonce and --digest.
    #[arg(long)]
    verify: bool,
    #[arg(long)]
    nonce: Option<u64>,
    #[arg(long)]
    digest: Option<String>,
}

static STOP: AtomicBool = AtomicBool::new(false);
extern "C" {
    fn signal(sig: i32, h: extern "C" fn(i32)) -> usize;
}
extern "C" fn on_sig(_: i32) {
    STOP.store(true, Ordering::SeqCst);
}

struct Shared {
    cursor: u64,
    pending: Vec<(u64, u64)>,
    done: Intervals,
}

fn main() {
    let c = Cli::parse();
    let prefix = c.prefix.clone().unwrap_or_else(|| {
        if c.verify {
            String::new()
        } else {
            die("--prefix is required")
        }
    });
    if c.verify {
        return run_verify(&c, &prefix);
    }
    let sp = Spec {
        algo: c.algorithm,
        enc: c.encoding,
        input: c.input.clone(),
        prefix,
    };
    if let Err(e) = sp.target() {
        die(&e);
    }
    if let Err(e) = check_backend(&sp, c.backend) {
        die(&e);
    }
    if c.threads == 0 || c.chunk == 0 || c.start_nonce % c.chunk != 0 || c.start_nonce == NONCE_END
    {
        die("need threads >= 1, chunk >= 1, start-nonce a multiple of chunk and below 2^64-1");
    }
    let (chunk, dir) = (c.chunk, c.state.clone());
    ok(std::fs::create_dir_all(&dir), "state dir");
    let lock = ok(std::fs::File::create(dir.join("lock")), "lock file"); // one miner per state dir
    if lock.try_lock().is_err() {
        die("state dir locked by another miner");
    }
    match std::fs::read_to_string(dir.join("FOUND.json")) {
        Ok(t) => {
            // only ever return a proof that re-verifies AND belongs to this search
            let p: serde_json::Value = ok(serde_json::from_str(&t), "FOUND.json");
            let ours = p["algorithm"] == name(&sp.algo)
                && p["encoding"] == name(&sp.enc)
                && p["input"] == sp.input
                && p["prefix"] == sp.prefix;
            if let Err(e) = verify(&p) {
                die(&format!("FOUND.json failed verification: {e}"));
            }
            if !ours {
                die("FOUND.json is for a different search");
            }
            println!("{t}");
            return;
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => die(&format!("FOUND.json: {e}")),
    }
    unsafe {
        signal(2, on_sig);
        signal(15, on_sig);
    }
    let mut store = ok(Store::open(&dir, &sp.key(), chunk), "state");
    eprintln!(
        "START algo={} backend={:?} prefix={} threads={} chunk={chunk} completed_nonces={}",
        name(&c.algorithm),
        c.backend,
        sp.prefix,
        c.threads,
        store.done.total()
    );
    let sh = Arc::new(Mutex::new(Shared {
        cursor: c.start_nonce,
        pending: vec![],
        done: store.done.clone(),
    }));
    let hashed = Arc::new(AtomicU64::new(0));
    let found = Arc::new(Mutex::new(None));
    let t0 = Instant::now();
    let hs: Vec<_> = (0..c.threads)
        .map(|_| {
            let (sh, hashed, found, sp, be) = (
                sh.clone(),
                hashed.clone(),
                found.clone(),
                sp.clone(),
                c.backend,
            );
            std::thread::spawn(move || loop {
                if STOP.load(Ordering::Relaxed) {
                    return;
                }
                let (s, e) = {
                    // claim the lowest uncovered chunk at or after the cursor
                    let mut g = sh.lock().unwrap();
                    let s = g.done.next_free(g.cursor);
                    if s == NONCE_END {
                        return;
                    } // nonce space exhausted
                    let e = s.saturating_add(chunk); // saturates at NONCE_END: the last chunk may be short
                    g.cursor = e;
                    (s, e)
                };
                match mine_range(&sp, s, e - s, &STOP, be) {
                    ChunkResult::Done => {
                        hashed.fetch_add(e - s, Ordering::Relaxed);
                        let mut g = sh.lock().unwrap();
                        g.done.insert(s, e);
                        g.pending.push((s, e));
                    }
                    ChunkResult::Found(n) => {
                        // a chunk holding a hit is never recorded complete
                        *found.lock().unwrap() = Some(n);
                        STOP.store(true, Ordering::SeqCst);
                        return;
                    }
                    ChunkResult::Stopped => return, // partial chunk discarded
                }
            })
        })
        .collect();
    let (mut last_ck, mut last_pr) = (Instant::now(), Instant::now());
    loop {
        std::thread::sleep(Duration::from_millis(500));
        if t0.elapsed().as_secs() >= c.max_secs {
            STOP.store(true, Ordering::SeqCst);
        }
        drain(&mut store, &sh);
        if last_ck.elapsed().as_secs() >= 30 || STOP.load(Ordering::Relaxed) {
            ok(store.checkpoint(), "checkpoint");
            last_ck = Instant::now();
        }
        if last_pr.elapsed().as_secs() >= 10 {
            let h = hashed.load(Ordering::Relaxed);
            eprintln!(
                "PROGRESS session_hashed={h} total_completed={} MH/s={:.2}",
                store.done.total(),
                h as f64 / t0.elapsed().as_secs_f64() / 1e6
            );
            last_pr = Instant::now();
        }
        if hs.iter().all(|h| h.is_finished()) {
            break;
        } // stopped, found, or nonce space exhausted
    }
    for h in hs {
        h.join().unwrap();
    }
    drain(&mut store, &sh);
    ok(store.checkpoint(), "checkpoint");
    let hit = *found.lock().unwrap();
    match hit {
        Some(n) => {
            let d = sp
                .matches(n)
                .unwrap_or_else(|| die("BUG: hit failed reference check"));
            let txt = serde_json::to_string_pretty(&sp.proof(n, &d)).unwrap();
            let tmp = dir.join("FOUND.tmp");
            ok(std::fs::write(&tmp, &txt), "FOUND.tmp write");
            ok(
                ok(std::fs::File::open(&tmp), "FOUND.tmp open").sync_all(),
                "FOUND.tmp sync",
            );
            ok(
                std::fs::rename(&tmp, dir.join("FOUND.json")),
                "FOUND.json rename",
            );
            ok(
                ok(std::fs::File::open(&dir), "state dir open").sync_all(),
                "state dir sync",
            );
            eprintln!("FOUND");
            println!("{txt}"); // stdout carries only the proof, so it pipes into --verify
        }
        None if store.done.next_free(c.start_nonce) == NONCE_END => {
            eprintln!(
                "EXHAUSTED total_completed={} (no match in [{}, 2^64-1))",
                store.done.total(),
                c.start_nonce
            );
            std::process::exit(3)
        }
        None => eprintln!(
            "STOPPED total_completed={} rate_MH/s={:.2}",
            store.done.total(),
            hashed.load(Ordering::Relaxed) as f64 / t0.elapsed().as_secs_f64() / 1e6
        ),
    }
}

fn run_verify(c: &Cli, prefix: &str) {
    let p: serde_json::Value = if let (Some(n), Some(d)) = (c.nonce, &c.digest) {
        serde_json::json!({ "input": c.input, "nonce": n, "digest": d, "algorithm": name(&c.algorithm),
                            "encoding": name(&c.encoding), "prefix": prefix })
    } else {
        let mut s = String::new();
        ok(std::io::stdin().read_to_string(&mut s), "stdin");
        ok(serde_json::from_str(&s), "bad JSON")
    };
    match verify(&p) {
        Ok(m) => println!("{m}"),
        Err(e) => {
            eprintln!("INVALID: {e}");
            std::process::exit(1)
        }
    }
}

/// Journal everything completed since the last call (idempotent; safe to call often).
fn drain(store: &mut Store, sh: &Mutex<Shared>) {
    let (p, d) = {
        let mut g = sh.lock().unwrap();
        (std::mem::take(&mut g.pending), g.done.clone())
    };
    store.done = d;
    if !p.is_empty() {
        ok(store.append(&p), "journal");
    }
}

/// Unwrap a result or exit 2 with `error: <ctx>: <err>` (same exit path as `die`).
fn ok<T, E: std::fmt::Display>(r: Result<T, E>, ctx: &str) -> T {
    r.unwrap_or_else(|e| die(&format!("{ctx}: {e}")))
}

fn die(m: &str) -> ! {
    eprintln!("error: {m}");
    std::process::exit(2)
}
