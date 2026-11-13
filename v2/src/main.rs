use gensys_miner::{state::Store, *};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

static STOP: AtomicBool = AtomicBool::new(false);
extern "C" {
    fn signal(sig: i32, h: extern "C" fn(i32)) -> usize;
}
extern "C" fn on_sig(_: i32) { STOP.store(true, Ordering::SeqCst); }

struct Shared {
    cursor: u64,
    pending: Vec<(u64, u64)>, // completed, not yet journaled
    done: state::Intervals,
}

fn main() {
    let mut a = std::env::args().skip(1);
    let (mut dir, mut prefix, mut threads, mut chunk, mut start, mut max_secs) =
        (PathBuf::from("state"), "gensys".to_string(), 6usize, 1u64 << 18, 0u64, u64::MAX);
    while let Some(k) = a.next() {
        let v = a.next().unwrap_or_else(|| die(&format!("{k} needs a value")));
        match k.as_str() {
            "--state" => dir = v.into(),
            "--prefix" => prefix = v,
            "--threads" => threads = v.parse().unwrap(),
            "--chunk" => chunk = v.parse().unwrap(),
            "--start-nonce" => start = v.parse().unwrap(), // allocation begins here; lower nonces only if already uncovered AND >= start
            "--max-secs" => max_secs = v.parse().unwrap(),
            _ => die(&format!("unknown flag {k}")),
        }
    }
    if threads == 0 || target_bits(&prefix).is_none() { die("need threads >= 1 and a 1-10 char Base64 prefix"); }
    if chunk % 8 != 0 || start % chunk != 0 { die("chunk must be a multiple of 8 and start-nonce a multiple of chunk"); }
    std::fs::create_dir_all(&dir).unwrap();
    // single-instance lock on the state dir
    let lock = std::fs::File::create(dir.join("lock")).unwrap();
    if lock.try_lock().is_err() { die("state dir locked by another miner"); }
    if let Ok(t) = std::fs::read_to_string(dir.join("FOUND.txt")) {
        if !t.contains(&format!("prefix={prefix}\n")) { die("FOUND.txt in this state dir is for a different prefix"); }
        println!("already found:\n{t}");
        return;
    }
    unsafe { signal(2, on_sig); signal(15, on_sig); }
    let mut store = Store::open(&dir, &prefix, chunk).unwrap_or_else(|e| die(&format!("state: {e}")));
    let resumed = store.done.total();
    println!("START prefix={prefix} threads={threads} chunk={chunk} start={start} completed_nonces={resumed} intervals={}", store.done.0.len());
    let sh = Arc::new(Mutex::new(Shared { cursor: start, pending: vec![], done: store.done.clone() }));
    let hashed = Arc::new(AtomicU64::new(0));
    let found: Arc<Mutex<Option<u64>>> = Arc::new(Mutex::new(None));
    let t0 = Instant::now();
    let hs: Vec<_> = (0..threads).map(|_| {
        let (sh, hashed, found, prefix) = (sh.clone(), hashed.clone(), found.clone(), prefix.clone());
        std::thread::spawn(move || loop {
            if STOP.load(Ordering::Relaxed) { return; }
            let s = { // claim lowest uncovered chunk >= cursor
                let mut g = sh.lock().unwrap();
                let s = g.done.next_free(g.cursor);
                g.cursor = s + chunk;
                s
            };
            match mine_range(s, chunk, &prefix, &STOP) {
                ChunkResult::Done => {
                    hashed.fetch_add(chunk, Ordering::Relaxed);
                    let mut g = sh.lock().unwrap();
                    g.done.insert(s, s + chunk);
                    g.pending.push((s, s + chunk));
                }
                ChunkResult::Found(n) => { // chunk NOT recorded complete
                    *found.lock().unwrap() = Some(n);
                    STOP.store(true, Ordering::SeqCst);
                    return;
                }
                ChunkResult::Stopped => return, // partial chunk discarded, never recorded
            }
        })
    }).collect();
    let (mut last_ck, mut last_pr) = (Instant::now(), Instant::now());
    loop {
        std::thread::sleep(Duration::from_millis(500));
        if t0.elapsed().as_secs() >= max_secs { STOP.store(true, Ordering::SeqCst); }
        drain(&mut store, &sh);
        if last_ck.elapsed().as_secs() >= 30 || STOP.load(Ordering::Relaxed) {
            store.checkpoint().unwrap_or_else(|e| die(&format!("checkpoint: {e}")));
            last_ck = Instant::now();
        }
        if last_pr.elapsed().as_secs() >= 10 {
            let h = hashed.load(Ordering::Relaxed);
            println!("PROGRESS session_hashed={h} total_completed={} MH/s={:.2}", store.done.total(), h as f64 / t0.elapsed().as_secs_f64() / 1e6);
            last_pr = Instant::now();
        }
        if STOP.load(Ordering::Relaxed) && hs.iter().all(|h| h.is_finished()) { break; }
    }
    for h in hs { h.join().unwrap(); }
    drain(&mut store, &sh);
    store.checkpoint().unwrap();
    let fnd = *found.lock().unwrap();
    if let Some(n) = fnd {
        let (d, b) = verify(n, &prefix).unwrap_or_else(|| die("BUG: found nonce failed scalar verify"));
        let hex: String = d.iter().map(|x| format!("{x:02x}")).collect();
        let txt = format!("nonce={n}\ninput={}\nsha256_hex={hex}\nbase64={b}\nprefix={prefix}\n", input_for(n));
        let tmp = dir.join("FOUND.tmp");
        std::fs::write(&tmp, &txt).unwrap();
        std::fs::File::open(&tmp).unwrap().sync_all().unwrap();
        std::fs::rename(&tmp, dir.join("FOUND.txt")).unwrap();
        std::fs::File::open(&dir).unwrap().sync_all().unwrap();
        println!("FOUND\n{txt}");
    } else {
        println!("STOPPED total_completed={}", store.done.total());
    }
}

/// Journal everything completed since the last call (idempotent; safe to call often).
fn drain(store: &mut Store, sh: &Mutex<Shared>) {
    let (p, d) = { let mut g = sh.lock().unwrap(); (std::mem::take(&mut g.pending), g.done.clone()) };
    store.done = d;
    if !p.is_empty() { store.append(&p).unwrap_or_else(|e| die(&format!("journal: {e}"))); }
}

fn die(m: &str) -> ! { eprintln!("error: {m}"); std::process::exit(2) }
