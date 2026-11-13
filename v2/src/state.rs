//! Durable completed-interval state: atomic checkpoint + append-only journal.
//! Only intervals whose every nonce was hashed are ever recorded. Anything
//! lost on a crash is simply re-mined; nothing is ever over-claimed.
use std::collections::BTreeMap;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

fn fnv(s: &str) -> u64 {
    let mut h = 0xcbf29ce484222325u64;
    for b in s.bytes() {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

/// Merged set of half-open [start,end) nonce intervals.
#[derive(Default, Clone)]
pub struct Intervals(pub BTreeMap<u64, u64>);

impl Intervals {
    pub fn insert(&mut self, s: u64, e: u64) {
        if s >= e {
            return;
        }
        let (mut s, mut e) = (s, e);
        let lo: Vec<(u64, u64)> = self.0.range(..=e).rev().take_while(|(_, &pe)| pe >= s).map(|(&a, &b)| (a, b)).collect();
        for (a, b) in lo {
            s = s.min(a);
            e = e.max(b);
            self.0.remove(&a);
        }
        self.0.insert(s, e);
    }
    /// First nonce >= n not covered.
    pub fn next_free(&self, n: u64) -> u64 {
        match self.0.range(..=n).next_back() {
            Some((_, &e)) if e > n => e,
            _ => n,
        }
    }
    pub fn total(&self) -> u64 {
        self.0.iter().map(|(s, e)| e - s).sum()
    }
}

pub struct Store {
    pub dir: PathBuf,
    pub prefix: String,
    pub chunk: u64,
    pub done: Intervals,
    journal: File,
}

impl Store {
    pub fn open(dir: &Path, prefix: &str, chunk: u64) -> io::Result<Store> {
        fs::create_dir_all(dir)?;
        let mut done = Intervals::default();
        if let Ok(txt) = fs::read_to_string(dir.join("checkpoint.txt")) {
            done = parse_checkpoint(&txt, prefix, chunk)?;
        }
        if let Ok(txt) = fs::read_to_string(dir.join("journal.log")) {
            for line in txt.lines() {
                if let Some((s, e)) = parse_journal_line(line) {
                    done.insert(s, e); // torn/corrupt lines are skipped
                }
            }
        }
        let journal = OpenOptions::new().create(true).append(true).open(dir.join("journal.log"))?;
        File::open(dir)?.sync_all()?; // make the journal's directory entry durable
        Ok(Store { dir: dir.to_path_buf(), prefix: prefix.into(), chunk, done, journal })
    }

    /// Append completed intervals to the journal and fsync.
    pub fn append(&mut self, ivs: &[(u64, u64)]) -> io::Result<()> {
        let mut buf = String::new();
        for &(s, e) in ivs {
            let body = format!("D {s} {e}");
            buf += &format!("{body} {:016x}\n", fnv(&body));
        }
        self.journal.write_all(buf.as_bytes())?;
        self.journal.sync_data()
    }

    /// Atomic snapshot (tmp+fsync+rename+dir fsync), then truncate journal.
    /// A crash between the two steps is harmless: replay is idempotent.
    pub fn checkpoint(&mut self) -> io::Result<()> {
        let mut body = format!("gensys-ckpt v1\nprefix {}\nchunk {}\n", self.prefix, self.chunk);
        for (s, e) in &self.done.0 {
            body += &format!("I {s} {e}\n");
        }
        body += &format!("sum {:016x}\n", fnv(&body));
        let tmp = self.dir.join("checkpoint.tmp");
        let mut f = File::create(&tmp)?;
        f.write_all(body.as_bytes())?;
        f.sync_all()?;
        fs::rename(&tmp, self.dir.join("checkpoint.txt"))?;
        File::open(&self.dir)?.sync_all()?;
        self.journal.set_len(0)?;
        self.journal.sync_all()
    }
}

fn parse_journal_line(l: &str) -> Option<(u64, u64)> {
    let p: Vec<&str> = l.split(' ').collect();
    if p.len() != 4 || p[0] != "D" { return None; }
    let body = format!("D {} {}", p[1], p[2]);
    if format!("{:016x}", fnv(&body)) != p[3] { return None; }
    let (s, e) = (p[1].parse().ok()?, p[2].parse().ok()?);
    if s < e { Some((s, e)) } else { None }
}

fn parse_checkpoint(txt: &str, prefix: &str, chunk: u64) -> io::Result<Intervals> {
    let bad = |m: &str| io::Error::new(io::ErrorKind::InvalidData, format!("checkpoint: {m}"));
    let idx = txt.rfind("sum ").ok_or_else(|| bad("no checksum"))?;
    let (body, sum) = txt.split_at(idx);
    if format!("sum {:016x}\n", fnv(body)) != sum { return Err(bad("checksum mismatch")); }
    let mut it = Intervals::default();
    for l in body.lines() {
        let p: Vec<&str> = l.split(' ').collect();
        match p.as_slice() {
            ["prefix", v] if *v != prefix => return Err(bad("prefix differs from state")),
            ["chunk", v] if v.parse::<u64>().ok() != Some(chunk) => return Err(bad("chunk size differs from state")),
            ["I", s, e] => it.insert(s.parse().map_err(|_| bad("num"))?, e.parse().map_err(|_| bad("num"))?),
            _ => {}
        }
    }
    Ok(it)
}
