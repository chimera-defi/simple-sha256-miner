//! Vanity-hash search: find `n` so that encode(H("<input>:<n>")) starts with `prefix`.
pub mod state;
use base64::{engine::general_purpose::STANDARD, Engine};
use clap::ValueEnum;
use serde_json::{json, Value};
use std::sync::atomic::{AtomicBool, Ordering};

#[derive(Clone, Copy, PartialEq, ValueEnum)]
pub enum Algo { Sha256, Blake3 }
#[derive(Clone, Copy, PartialEq, ValueEnum)]
pub enum Enc { Base64, Hex }

pub fn name<T: ValueEnum>(v: &T) -> String { v.to_possible_value().unwrap().get_name().to_string() }
fn parse<T: ValueEnum>(s: &str) -> Result<T, String> { T::from_str(s, false) }

/// Everything that defines one search. Different settings never share state.
#[derive(Clone)]
pub struct Spec { pub algo: Algo, pub enc: Enc, pub input: String, pub prefix: String }

const B64: &str = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

impl Spec {
    /// State key: hex of the canonical settings, so any change selects a different search.
    pub fn key(&self) -> String {
        hex::encode(format!("{}\n{}\n{}\n{}", name(&self.algo), name(&self.enc), self.input, self.prefix))
    }
    /// Prefix as (value, bit count). Case-sensitive; hex must be lowercase.
    pub fn target(&self) -> Result<(u64, u32), String> {
        let (alpha, per, max) = if self.enc == Enc::Base64 { (B64, 6, 10) } else { ("0123456789abcdef", 4, 16) };
        if self.prefix.is_empty() || self.prefix.len() > max { return Err(format!("prefix must be 1-{max} chars")); }
        let mut t = 0u64;
        for c in self.prefix.chars() {
            t = t << per | alpha.find(c).ok_or_else(|| format!("'{c}' is not in the {} alphabet", name(&self.enc)))? as u64;
        }
        Ok((t, per * self.prefix.len() as u32))
    }
    fn message(&self, nonce: u64) -> Vec<u8> { format!("{}:{nonce}", self.input).into_bytes() }
    /// Reference digest (sha2 / blake3 crates): used to confirm hits and to verify proofs.
    pub fn digest(&self, nonce: u64) -> [u8; 32] {
        let m = self.message(nonce);
        match self.algo {
            Algo::Sha256 => { use sha2::{Digest, Sha256}; Sha256::digest(&m).into() }
            Algo::Blake3 => *blake3::hash(&m).as_bytes(),
        }
    }
    pub fn encode(&self, d: &[u8; 32]) -> String {
        if self.enc == Enc::Base64 { STANDARD.encode(d) } else { hex::encode(d) }
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
    let s = |k: &str| p[k].as_str().map(str::to_string).ok_or(format!("missing string field '{k}'"));
    let nonce = match &p["nonce"] { Value::Number(n) => n.as_u64(), Value::String(s) => s.parse().ok(), _ => None }
        .ok_or("missing or invalid 'nonce'")?;
    let sp = Spec { algo: parse(&s("algorithm")?)?, enc: parse(&s("encoding")?)?, input: s("input")?, prefix: s("prefix")? };
    sp.target()?;
    let d = sp.digest(nonce);
    let (enc, claimed) = (sp.encode(&d), s("digest")?);
    if enc != claimed { return Err(format!("digest mismatch: recomputed {enc}")); }
    if let Some(h) = p["digest_hex"].as_str() { if h != hex::encode(d) { return Err("digest_hex mismatch".into()); } }
    if !enc.starts_with(&sp.prefix) { return Err(format!("digest does not start with '{}'", sp.prefix)); }
    Ok(format!("VALID {}:{nonce} -> {enc}", sp.input))
}

#[derive(Debug, PartialEq)]
pub enum ChunkResult { Done, Found(u64), Stopped }

const BATCH: usize = 128;

/// Hash nonces [start, start+len). Done means EVERY nonce was hashed; hits are re-confirmed
/// with the reference implementation.
pub fn mine_range(sp: &Spec, start: u64, len: u64, stop: &AtomicBool) -> ChunkResult {
    let (t, bits) = sp.target().expect("bad prefix");
    let m = bits.min(32); // prefilter on the top m bits of the first digest word
    let (want, shift) = ((t >> (bits - m)) as u32, 32 - m);
    let pre = format!("{}:", sp.input).into_bytes();
    let end = start + len;
    let mut n = start;
    while n < end {
        if (n - start) / BATCH as u64 % 64 == 0 && stop.load(Ordering::Relaxed) { return ChunkResult::Stopped; }
        let cnt = BATCH.min((end - n) as usize);
        let mut heads = [0u32; BATCH];
        if sp.algo == Algo::Sha256 {
            let mut buf = [[0u8; 20]; BATCH];
            let mut ln = [0usize; BATCH];
            for i in 0..cnt {
                let (mut x, mut k) = (n + i as u64, 20);
                loop { k -= 1; buf[i][k] = b'0' + (x % 10) as u8; x /= 10; if x == 0 { break; } }
                buf[i].copy_within(k.., 0);
                ln[i] = 20 - k;
            }
            let bodies: [&[u8]; BATCH] = std::array::from_fn(|i| &buf[i][..ln[i]]);
            let mut out = [[0u8; 32]; BATCH];
            tape_sha256::hash_many_prefixed(&pre, &bodies[..cnt], &mut out[..cnt]);
            for i in 0..cnt { heads[i] = u32::from_be_bytes(out[i][..4].try_into().unwrap()); }
        } else {
            for i in 0..cnt {
                let mut msg = pre.clone();
                msg.extend_from_slice((n + i as u64).to_string().as_bytes());
                heads[i] = u32::from_be_bytes(blake3::hash(&msg).as_bytes()[..4].try_into().unwrap());
            }
        }
        for i in 0..cnt {
            if heads[i] >> shift == want && sp.matches(n + i as u64).is_some() { return ChunkResult::Found(n + i as u64); }
        }
        n += cnt as u64;
    }
    ChunkResult::Done
}
