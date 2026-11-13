use clap::Parser;
use std::io::Read;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use vanity_miner::{state::{Intervals, Store}, *};

/// Search for a nonce whose encoded digest starts with PREFIX, or verify a proof (--verify).
#[derive(Parser)]
struct Cli {
    #[arg(long)] prefix: Option<String>,
    /// Text hashed as "<input>:<nonce>".
    #[arg(long, alias = "seed", default_value = "gensys")] input: String,
    #[arg(long, value_enum, default_value = "sha256")] algorithm: Algo,
    #[arg(long, value_enum, default_value = "base64")] encoding: Enc,
    #[arg(long, default_value_t = 6)] threads: usize,
    #[arg(long, default_value = "state")] state: PathBuf,
    #[arg(long, default_value_t = 1 << 18)] chunk: u64,
    /// Lowest nonce to search (multiple of --chunk). Lower nonces are never covered.
    #[arg(long, default_value_t = 0)] start_nonce: u64,
    #[arg(long, default_value_t = u64::MAX)] max_secs: u64,
    /// Verify a proof: JSON on stdin, or with --nonce and --digest.
    #[arg(long)] verify: bool,
    #[arg(long)] nonce: Option<u64>,
    #[arg(long)] digest: Option<String>,
}

static STOP: AtomicBool = AtomicBool::new(false);
extern "C" { fn signal(sig: i32, h: extern "C" fn(i32)) -> usize; }
extern "C" fn on_sig(_: i32) { STOP.store(true, Ordering::SeqCst); }

struct Shared { cursor: u64, pending: Vec<(u64, u64)>, done: Intervals }

fn main() {
    let c = Cli::parse();
    let prefix = c.prefix.clone().unwrap_or_else(|| if c.verify { String::new() } else { die("--prefix is required") });
    if c.verify { return run_verify(&c, &prefix); }
    let sp = Spec { algo: c.algorithm, enc: c.encoding, input: c.input.clone(), prefix };
    if let Err(e) = sp.target() { die(&e); }
    if c.threads == 0 || c.chunk == 0 || c.start_nonce % c.chunk != 0 { die("need threads >= 1, chunk >= 1, start-nonce a multiple of chunk"); }
    let (chunk, dir) = (c.chunk, c.state.clone());
    std::fs::create_dir_all(&dir).unwrap();
    let lock = std::fs::File::create(dir.join("lock")).unwrap(); // one miner per state dir
    if lock.try_lock().is_err() { die("state dir locked by another miner"); }
    if let Ok(t) = std::fs::read_to_string(dir.join("FOUND.json")) {
        let p: serde_json::Value = serde_json::from_str(&t).unwrap_or_else(|e| die(&format!("FOUND.json: {e}")));
        if p["prefix"] != sp.prefix || p["input"] != sp.input { die("FOUND.json in this state dir is for a different search"); }
        println!("{t}");
        return;
    }
    unsafe { signal(2, on_sig); signal(15, on_sig); }
    let mut store = Store::open(&dir, &sp.key(), chunk).unwrap_or_else(|e| die(&format!("state: {e}")));
    eprintln!("START algo={} prefix={} threads={} chunk={chunk} completed_nonces={}", name(&c.algorithm), sp.prefix, c.threads, store.done.total());
    let sh = Arc::new(Mutex::new(Shared { cursor: c.start_nonce, pending: vec![], done: store.done.clone() }));
    let hashed = Arc::new(AtomicU64::new(0));
    let found = Arc::new(Mutex::new(None));
    let t0 = Instant::now();
    let hs: Vec<_> = (0..c.threads).map(|_| {
        let (sh, hashed, found, sp) = (sh.clone(), hashed.clone(), found.clone(), sp.clone());
        std::thread::spawn(move || loop {
            if STOP.load(Ordering::Relaxed) { return; }
            let s = { // claim the lowest uncovered chunk at or after the cursor
                let mut g = sh.lock().unwrap();
                let s = g.done.next_free(g.cursor);
                match s.checked_add(chunk) { Some(e) => g.cursor = e, None => return }
                s
            };
            match mine_range(&sp, s, chunk, &STOP) {
                ChunkResult::Done => {
                    hashed.fetch_add(chunk, Ordering::Relaxed);
                    let mut g = sh.lock().unwrap();
                    g.done.insert(s, s + chunk);
                    g.pending.push((s, s + chunk));
                }
                ChunkResult::Found(n) => { // a chunk holding a hit is never recorded complete
                    *found.lock().unwrap() = Some(n);
                    STOP.store(true, Ordering::SeqCst);
                    return;
                }
                ChunkResult::Stopped => return, // partial chunk discarded
            }
        })
    }).collect();
    let (mut last_ck, mut last_pr) = (Instant::now(), Instant::now());
    loop {
        std::thread::sleep(Duration::from_millis(500));
        if t0.elapsed().as_secs() >= c.max_secs { STOP.store(true, Ordering::SeqCst); }
        drain(&mut store, &sh);
        if last_ck.elapsed().as_secs() >= 30 || STOP.load(Ordering::Relaxed) {
            store.checkpoint().unwrap_or_else(|e| die(&format!("checkpoint: {e}")));
            last_ck = Instant::now();
        }
        if last_pr.elapsed().as_secs() >= 10 {
            let h = hashed.load(Ordering::Relaxed);
            eprintln!("PROGRESS session_hashed={h} total_completed={} MH/s={:.2}", store.done.total(), h as f64 / t0.elapsed().as_secs_f64() / 1e6);
            last_pr = Instant::now();
        }
        if STOP.load(Ordering::Relaxed) && hs.iter().all(|h| h.is_finished()) { break; }
    }
    for h in hs { h.join().unwrap(); }
    drain(&mut store, &sh);
    store.checkpoint().unwrap();
    let hit = *found.lock().unwrap();
    match hit {
        Some(n) => {
            let d = sp.matches(n).unwrap_or_else(|| die("BUG: hit failed reference check"));
            let txt = serde_json::to_string_pretty(&sp.proof(n, &d)).unwrap();
            let tmp = dir.join("FOUND.tmp");
            std::fs::write(&tmp, &txt).unwrap();
            std::fs::File::open(&tmp).unwrap().sync_all().unwrap();
            std::fs::rename(&tmp, dir.join("FOUND.json")).unwrap();
            std::fs::File::open(&dir).unwrap().sync_all().unwrap();
            eprintln!("FOUND");
            println!("{txt}"); // stdout carries only the proof, so it pipes into --verify
        }
        None => eprintln!("STOPPED total_completed={}", store.done.total()),
    }
}

fn run_verify(c: &Cli, prefix: &str) {
    let p: serde_json::Value = if let (Some(n), Some(d)) = (c.nonce, &c.digest) {
        serde_json::json!({ "input": c.input, "nonce": n, "digest": d, "algorithm": name(&c.algorithm),
                            "encoding": name(&c.encoding), "prefix": prefix })
    } else {
        let mut s = String::new();
        std::io::stdin().read_to_string(&mut s).unwrap();
        serde_json::from_str(&s).unwrap_or_else(|e| die(&format!("bad JSON: {e}")))
    };
    match verify(&p) { Ok(m) => println!("{m}"), Err(e) => { eprintln!("INVALID: {e}"); std::process::exit(1) } }
}

/// Journal everything completed since the last call (idempotent; safe to call often).
fn drain(store: &mut Store, sh: &Mutex<Shared>) {
    let (p, d) = { let mut g = sh.lock().unwrap(); (std::mem::take(&mut g.pending), g.done.clone()) };
    store.done = d;
    if !p.is_empty() { store.append(&p).unwrap_or_else(|e| die(&format!("journal: {e}"))); }
}

fn die(m: &str) -> ! { eprintln!("error: {m}"); std::process::exit(2) }
