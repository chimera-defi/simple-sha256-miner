//! Vanity-hash search: find `n` so that encode(H("<input>:<n>")) starts with `prefix`.
#[cfg(target_arch = "x86_64")]
pub mod avx2;
pub mod state;
#[cfg(not(target_arch = "x86_64"))]
pub mod avx2 {
    pub fn available() -> bool {
        false
    }
    pub fn hash8(_: &[u8], _: [&[u8]; 8]) -> [[u8; 32]; 8] {
        unreachable!()
    }
}
use base64::{engine::general_purpose::STANDARD, Engine};
use clap::ValueEnum;
use serde_json::{json, Value};
use std::sync::atomic::{AtomicBool, Ordering};

#[derive(Clone, Copy, PartialEq, ValueEnum)]
pub enum Algo {
    Sha256,
    Blake3,
}
#[derive(Clone, Copy, PartialEq, ValueEnum)]
pub enum Enc {
    Base64,
    Hex,
}
/// SHA-256 engine. Does not change what is searched, so it is not part of the state key.
#[derive(Clone, Copy, PartialEq, ValueEnum, Debug)]
pub enum Backend {
    Lib,
    Avx2,
}

/// Startup check for an explicit `--backend avx2`: no silent fallback.
pub fn check_backend(sp: &Spec, be: Backend) -> Result<(), String> {
    if be == Backend::Lib {
        return Ok(());
    }
    if sp.algo != Algo::Sha256 {
        return Err("--backend avx2 supports sha256 only".into());
    }
    if !avx2::available() {
        return Err("--backend avx2: this CPU has no AVX2".into());
    }
    if sp.input.len() + 1 + 20 >= 56 {
        return Err(
            "--backend avx2: input too long for one block (need len(input)+21 < 56)".into(),
        );
    }
    Ok(())
}

pub fn name<T: ValueEnum>(v: &T) -> String {
    v.to_possible_value().unwrap().get_name().to_string()
}
fn parse<T: ValueEnum>(s: &str) -> Result<T, String> {
    T::from_str(s, false)
}

/// Everything that defines one search. Different settings never share state.
#[derive(Clone)]
pub struct Spec {
    pub algo: Algo,
    pub enc: Enc,
    pub input: String,
    pub prefix: String,
}

const B64: &str = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

impl Spec {
    /// State key: hex of the canonical settings, so any change selects a different search.
    pub fn key(&self) -> String {
        hex::encode(format!(
            "{}\n{}\n{}\n{}",
            name(&self.algo),
            name(&self.enc),
            self.input,
            self.prefix
        ))
    }
    /// Prefix as (value, bit count). Case-sensitive; hex must be lowercase.
    pub fn target(&self) -> Result<(u64, u32), String> {
        let (alpha, per, max) = if self.enc == Enc::Base64 {
            (B64, 6, 10)
        } else {
            ("0123456789abcdef", 4, 16)
        };
        if self.prefix.is_empty() || self.prefix.len() > max {
            return Err(format!("prefix must be 1-{max} chars"));
        }
        let mut t = 0u64;
        for c in self.prefix.chars() {
            t = t << per
                | alpha
                    .find(c)
                    .ok_or_else(|| format!("'{c}' is not in the {} alphabet", name(&self.enc)))?
                    as u64;
        }
        Ok((t, per * self.prefix.len() as u32))
    }
    fn message(&self, nonce: u64) -> Vec<u8> {
        format!("{}:{nonce}", self.input).into_bytes()
    }
    /// Reference digest (sha2 / blake3 crates): used to confirm hits and to verify proofs.
    pub fn digest(&self, nonce: u64) -> [u8; 32] {
        let m = self.message(nonce);
        match self.algo {
            Algo::Sha256 => {
                use sha2::{Digest, Sha256};
                Sha256::digest(&m).into()
            }
            Algo::Blake3 => *blake3::hash(&m).as_bytes(),
        }
    }
    pub fn encode(&self, d: &[u8; 32]) -> String {
        if self.enc == Enc::Base64 {
            STANDARD.encode(d)
        } else {
            hex::encode(d)
        }
    }
    pub fn matches(&self, nonce: u64) -> Option<[u8; 32]> {
        let d = self.digest(nonce);
        self.encode(&d).starts_with(&self.prefix).then_some(d)
    }
    /// Human-readable proof of a hit.
    pub fn proof(&self, nonce: u64, d: &[u8; 32]) -> Value {
        json!({ "input": self.input, "nonce": nonce, "message": format!("{}:{nonce}", self.input),
                "algorithm": name(&self.algo), "encoding": name(&self.enc), "prefix": self.prefix,
                "digest": self.encode(d), "digest_hex": hex::encode(d) })
    }
}

/// Recompute a proof from scratch. Extra fields are ignored; any mismatch is an error.
pub fn verify(p: &Value) -> Result<String, String> {
    let s = |k: &str| {
        p[k].as_str()
            .map(str::to_string)
            .ok_or(format!("missing string field '{k}'"))
    };
    let nonce = match &p["nonce"] {
        Value::Number(n) => n.as_u64(),
        Value::String(s) => s.parse().ok(),
        _ => None,
    }
    .ok_or("missing or invalid 'nonce'")?;
    let sp = Spec {
        algo: parse(&s("algorithm")?)?,
        enc: parse(&s("encoding")?)?,
        input: s("input")?,
        prefix: s("prefix")?,
    };
    sp.target()?;
    if let Some(m) = p["message"].as_str() {
        if m != format!("{}:{nonce}", sp.input) {
            return Err("message mismatch".into());
        }
    }
    let d = sp.digest(nonce);
    let (enc, claimed) = (sp.encode(&d), s("digest")?);
    if enc != claimed {
        return Err(format!("digest mismatch: recomputed {enc}"));
    }
    if let Some(h) = p["digest_hex"].as_str() {
        if h != hex::encode(d) {
            return Err("digest_hex mismatch".into());
        }
    }
    if !enc.starts_with(&sp.prefix) {
        return Err(format!("digest does not start with '{}'", sp.prefix));
    }
    Ok(format!("VALID {}:{nonce} -> {enc}", sp.input))
}

#[derive(Debug, PartialEq)]
pub enum ChunkResult {
    Done,
    Found(u64),
    Stopped,
}

const BATCH: usize = 128;
/// Nonces are searched in [0, NONCE_END). `u64::MAX` itself is never mined: a half-open
/// `[start, end)` interval cannot name it.
pub const NONCE_END: u64 = u64::MAX;

/// Hash nonces [start, start+len). Done means EVERY nonce was hashed; hits are re-confirmed
/// with the reference implementation.
pub fn mine_range(sp: &Spec, start: u64, len: u64, stop: &AtomicBool, be: Backend) -> ChunkResult {
    let (t, bits) = sp.target().expect("bad prefix");
    let m = bits.min(32); // prefilter on the top m bits of the first digest word
    let (want, shift) = ((t >> (bits - m)) as u32, 32 - m);
    let pre = format!("{}:", sp.input).into_bytes();
    let end = start.checked_add(len).expect("range overflows u64");
    let mut n = start;
    let mut heads = [0u32; BATCH];
    let mut buf = [[0u8; 20]; BATCH];
    let mut ln = [0usize; BATCH];
    while n < end {
        if ((n - start) / BATCH as u64).is_multiple_of(64) && stop.load(Ordering::Relaxed) {
            return ChunkResult::Stopped;
        }
        let cnt = BATCH.min((end - n) as usize);
        if sp.algo == Algo::Sha256 {
            // decimal text of n, then step it up by one per lane instead of formatting each nonce
            let (mut cur, mut x, mut k) = ([0u8; 20], n, 20);
            loop {
                k -= 1;
                cur[k] = b'0' + (x % 10) as u8;
                x /= 10;
                if x == 0 {
                    break;
                }
            }
            let mut cl = 20 - k;
            cur.copy_within(k.., 0);
            for i in 0..cnt {
                buf[i][..cl].copy_from_slice(&cur[..cl]);
                ln[i] = cl;
                let mut k = cl;
                loop {
                    if k == 0 {
                        cur.copy_within(0..cl, 1);
                        cur[0] = b'1';
                        cl += 1;
                        break;
                    }
                    k -= 1;
                    if cur[k] == b'9' {
                        cur[k] = b'0'
                    } else {
                        cur[k] += 1;
                        break;
                    }
                }
            }
            let bodies: [&[u8]; BATCH] = std::array::from_fn(|i| &buf[i][..ln[i]]);
            if be == Backend::Avx2 {
                for g in (0..cnt).step_by(8) {
                    // a short last group repeats its final lane; extras are ignored
                    let b: [&[u8]; 8] = std::array::from_fn(|j| {
                        let i = (g + j).min(cnt - 1);
                        &buf[i][..ln[i]]
                    });
                    for (j, d) in avx2::hash8(&pre, b).iter().enumerate().take(cnt - g) {
                        heads[g + j] = u32::from_be_bytes(d[..4].try_into().unwrap());
                    }
                }
            } else {
                tape_sha256::hash_short_heads(&pre, &bodies[..cnt], &mut heads[..cnt]);
            }
        } else {
            for (i, head) in heads.iter_mut().enumerate().take(cnt) {
                let mut msg = pre.clone();
                msg.extend_from_slice((n + i as u64).to_string().as_bytes());
                *head = u32::from_be_bytes(blake3::hash(&msg).as_bytes()[..4].try_into().unwrap());
            }
        }
        for (i, &head) in heads.iter().enumerate().take(cnt) {
            if head >> shift == want && sp.matches(n + i as u64).is_some() {
                return ChunkResult::Found(n + i as u64);
            }
        }
        n += cnt as u64;
    }
    ChunkResult::Done
}
