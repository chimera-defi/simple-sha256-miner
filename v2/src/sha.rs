//! SHA-256 for single-block messages: scalar reference + AVX2 8-lane.
use std::arch::x86_64::*;

const K: [u32; 64] = [
    0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4, 0xab1c5ed5,
    0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe, 0x9bdc06a7, 0xc19bf174,
    0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f, 0x4a7484aa, 0x5cb0a9dc, 0x76f988da,
    0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7, 0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967,
    0x27b70a85, 0x2e1b2138, 0x4d2c6dfc, 0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85,
    0xa2bfe8a1, 0xa81a664b, 0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070,
    0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
    0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7, 0xc67178f2,
];
const IV: [u32; 8] = [
    0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab, 0x5be0cd19,
];

/// Scalar SHA-256 of a message < 56 bytes (one block).
pub fn sha256_short(msg: &[u8]) -> [u8; 32] {
    assert!(msg.len() < 56);
    let mut blk = [0u8; 64];
    blk[..msg.len()].copy_from_slice(msg);
    blk[msg.len()] = 0x80;
    blk[56..].copy_from_slice(&((msg.len() as u64) * 8).to_be_bytes());
    let mut w = [0u32; 64];
    for i in 0..16 {
        w[i] = u32::from_be_bytes([blk[4 * i], blk[4 * i + 1], blk[4 * i + 2], blk[4 * i + 3]]);
    }
    for i in 16..64 {
        let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
        let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
        w[i] = w[i - 16].wrapping_add(s0).wrapping_add(w[i - 7]).wrapping_add(s1);
    }
    let mut v = IV;
    for i in 0..64 {
        let [a, b, c, d, e, f, g, h] = v;
        let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
        let ch = (e & f) ^ (!e & g);
        let t1 = h.wrapping_add(s1).wrapping_add(ch).wrapping_add(K[i]).wrapping_add(w[i]);
        let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
        let maj = (a & b) ^ (a & c) ^ (b & c);
        let t2 = s0.wrapping_add(maj);
        v = [t1.wrapping_add(t2), a, b, c, d.wrapping_add(t1), e, f, g];
    }
    let mut out = [0u8; 32];
    for i in 0..8 {
        out[4 * i..4 * i + 4].copy_from_slice(&v[i].wrapping_add(IV[i]).to_be_bytes());
    }
    out
}

#[inline(always)]
unsafe fn rotr<const N: i32, const M: i32>(x: __m256i) -> __m256i {
    _mm256_or_si256(_mm256_srli_epi32::<N>(x), _mm256_slli_epi32::<M>(x))
}

/// 8 single-block hashes in parallel. `w[i][lane]` = big-endian message word i.
/// Returns the first two digest words (h0, h1) per lane.
#[target_feature(enable = "avx2")]
pub unsafe fn sha256_x8_h01(w_in: &[[u32; 8]; 16]) -> ([u32; 8], [u32; 8]) {
    let mut w = [_mm256_setzero_si256(); 64];
    for i in 0..16 {
        w[i] = _mm256_loadu_si256(w_in[i].as_ptr() as *const __m256i);
    }
    for i in 16..64 {
        let a = w[i - 15];
        let b = w[i - 2];
        let s0 = _mm256_xor_si256(_mm256_xor_si256(rotr::<7, 25>(a), rotr::<18, 14>(a)), _mm256_srli_epi32(a, 3));
        let s1 = _mm256_xor_si256(_mm256_xor_si256(rotr::<17, 15>(b), rotr::<19, 13>(b)), _mm256_srli_epi32(b, 10));
        w[i] = _mm256_add_epi32(_mm256_add_epi32(w[i - 16], s0), _mm256_add_epi32(w[i - 7], s1));
    }
    let mut v = [_mm256_setzero_si256(); 8];
    for i in 0..8 {
        v[i] = _mm256_set1_epi32(IV[i] as i32);
    }
    for i in 0..64 {
        let (a, b, c, d, e, f, g, h) = (v[0], v[1], v[2], v[3], v[4], v[5], v[6], v[7]);
        let s1 = _mm256_xor_si256(_mm256_xor_si256(rotr::<6, 26>(e), rotr::<11, 21>(e)), rotr::<25, 7>(e));
        let ch = _mm256_xor_si256(_mm256_and_si256(e, f), _mm256_andnot_si256(e, g));
        let t1 = _mm256_add_epi32(
            _mm256_add_epi32(h, s1),
            _mm256_add_epi32(_mm256_add_epi32(ch, _mm256_set1_epi32(K[i] as i32)), w[i]),
        );
        let s0 = _mm256_xor_si256(_mm256_xor_si256(rotr::<2, 30>(a), rotr::<13, 19>(a)), rotr::<22, 10>(a));
        let maj = _mm256_xor_si256(_mm256_xor_si256(_mm256_and_si256(a, b), _mm256_and_si256(a, c)), _mm256_and_si256(b, c));
        let t2 = _mm256_add_epi32(s0, maj);
        v[7] = g;
        v[6] = f;
        v[5] = e;
        v[4] = _mm256_add_epi32(d, t1);
        v[3] = c;
        v[2] = b;
        v[1] = a;
        v[0] = _mm256_add_epi32(t1, t2);
    }
    let h0 = _mm256_add_epi32(v[0], _mm256_set1_epi32(IV[0] as i32));
    let h1 = _mm256_add_epi32(v[1], _mm256_set1_epi32(IV[1] as i32));
    let mut o0 = [0u32; 8];
    let mut o1 = [0u32; 8];
    _mm256_storeu_si256(o0.as_mut_ptr() as *mut __m256i, h0);
    _mm256_storeu_si256(o1.as_mut_ptr() as *mut __m256i, h1);
    (o0, o1)
}
