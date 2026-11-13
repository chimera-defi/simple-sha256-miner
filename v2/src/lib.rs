pub mod sha;
pub mod state;
use std::sync::atomic::{AtomicBool, Ordering};

pub const MSG_PREFIX: &str = "gensys:"; // input = MSG_PREFIX + decimal nonce (same as v1)
const ABC: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

pub fn input_for(nonce: u64) -> String { format!("{MSG_PREFIX}{nonce}") }

pub fn base64(d: &[u8]) -> String {
    let mut s = String::new();
    for c in d.chunks(3) {
        let x = (c[0] as u32) << 16 | (*c.get(1).unwrap_or(&0) as u32) << 8 | *c.get(2).unwrap_or(&0) as u32;
        for k in 0..4 {
            if k <= c.len() { s.push(ABC[((x >> (18 - 6 * k)) & 63) as usize] as char) } else { s.push('=') }
        }
    }
    s
}

/// Case-sensitive Base64 prefix (<=10 chars => <=60 bits) as an integer.
pub fn target_bits(prefix: &str) -> Option<(u64, u32)> {
    if prefix.is_empty() || prefix.len() > 10 { return None; }
    let mut t = 0u64;
    for c in prefix.bytes() { t = t << 6 | ABC.iter().position(|&a| a == c)? as u64; }
    Some((t, 6 * prefix.len() as u32))
}

/// Full scalar check, independent of the SIMD path.
pub fn verify(nonce: u64, prefix: &str) -> Option<([u8; 32], String)> {
    let d = sha::sha256_short(input_for(nonce).as_bytes());
    let b = base64(&d);
    if b.starts_with(prefix) { Some((d, b)) } else { None }
}

#[derive(Debug, PartialEq)]
pub enum ChunkResult { Done, Found(u64), Stopped }

fn itoa(mut n: u64, out: &mut [u8; 20]) -> usize {
    let mut tmp = [0u8; 20];
    let mut i = 20;
    loop { i -= 1; tmp[i] = b'0' + (n % 10) as u8; n /= 10; if n == 0 { break; } }
    let l = 20 - i;
    out[..l].copy_from_slice(&tmp[i..]);
    l
}

/// Hash nonces [start, start+len). `len` and `start` must be multiples of 8.
/// Returns Done only if EVERY nonce in the range was hashed.
pub fn mine_range(start: u64, len: u64, prefix: &str, stop: &AtomicBool) -> ChunkResult {
    let (target, bits) = target_bits(prefix).expect("bad prefix");
    let m = bits.min(32); // prefilter on top m bits of digest word 0
    let want = (target >> (bits - m)) as u32;
    let shift = 32 - m;
    let avx2 = is_x86_feature_detected!("avx2");
    let mut bufs = [[0u8; 64]; 8];
    let pl = MSG_PREFIX.len();
    for b in bufs.iter_mut() { b[..pl].copy_from_slice(MSG_PREFIX.as_bytes()); }
    let mut n = start;
    let end = start + len;
    while n < end {
        if (n - start) & 0x3fff == 0 && stop.load(Ordering::Relaxed) { return ChunkResult::Stopped; }
        let mut w = [[0u32; 8]; 16];
        let mut cand = [false; 8];
        if avx2 {
            for l in 0..8 {
                let mut dg = [0u8; 20];
                let dl = itoa(n + l as u64, &mut dg);
                let b = &mut bufs[l];
                b[pl..pl + dl].copy_from_slice(&dg[..dl]);
                let tot = pl + dl;
                b[tot] = 0x80;
                for x in b[tot + 1..56].iter_mut() { *x = 0; }
                for i in 0..14 { w[i][l] = u32::from_be_bytes([b[4 * i], b[4 * i + 1], b[4 * i + 2], b[4 * i + 3]]); }
                w[15][l] = (tot as u32) * 8;
            }
            let (h0, _) = unsafe { sha::sha256_x8_h01(&w) };
            for l in 0..8 { cand[l] = (h0[l] >> shift) == want; }
        } else {
            for l in 0..8 { cand[l] = true; } // portable fallback: verify each
        }
        for l in 0..8 {
            if cand[l] && verify(n + l as u64, prefix).is_some() { return ChunkResult::Found(n + l as u64); }
        }
        n += 8;
    }
    ChunkResult::Done
}
