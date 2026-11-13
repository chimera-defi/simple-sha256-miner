use gensys_miner::{sha::*, state::*, *};
use std::sync::atomic::AtomicBool;

fn hex(d: &[u8]) -> String { d.iter().map(|x| format!("{x:02x}")).collect() }

#[test]
fn sha_vectors() {
    assert_eq!(hex(&sha256_short(b"abc")), "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad");
    assert_eq!(hex(&sha256_short(b"")), "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855");
}

#[test]
fn simd_matches_scalar_and_finds_every_hit() {
    // 3-char prefix: scan 4M nonces; SIMD path must find exactly the scalar-first hit.
    let p = "gen";
    let stop = AtomicBool::new(false);
    let first_scalar = (0u64..).step_by(1).find(|&n| verify(n, p).is_some()).unwrap();
    let blk = (first_scalar / 8) * 8;
    assert_eq!(mine_range(0, blk, p, &stop), ChunkResult::Done); // nothing before it
    assert_eq!(mine_range(blk, 8, p, &stop), ChunkResult::Found(first_scalar));
}

#[test]
fn intervals_merge_and_next_free() {
    let mut i = Intervals::default();
    i.insert(0, 10); i.insert(20, 30); i.insert(10, 20);
    assert_eq!(i.0.len(), 1);
    assert_eq!(i.next_free(5), 30);
    i.insert(40, 50);
    assert_eq!(i.next_free(35), 35);
    assert_eq!(i.next_free(45), 50);
    assert_eq!(i.total(), 40);
}

#[test]
fn journal_checkpoint_recovery_and_torn_tail() {
    let d = std::env::temp_dir().join(format!("gs-test-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    let mut s = Store::open(&d, "gensys", 8).unwrap();
    s.append(&[(0, 8), (8, 16)]).unwrap();
    s.done.insert(0, 16);
    s.checkpoint().unwrap();
    s.append(&[(32, 40)]).unwrap();
    drop(s);
    // simulate torn write + garbage at journal tail
    use std::io::Write;
    std::fs::OpenOptions::new().append(true).open(d.join("journal.log")).unwrap().write_all(b"D 100 20").unwrap();
    let s = Store::open(&d, "gensys", 8).unwrap();
    assert_eq!(s.done.total(), 24);
    assert_eq!(s.done.next_free(0), 16);
    assert_eq!(s.done.next_free(32), 40);
    // mismatched params refused
    assert!(Store::open(&d, "other", 8).is_err());
    assert!(Store::open(&d, "gensys", 16).is_err());
    // corrupted checkpoint refused
    let c = d.join("checkpoint.txt");
    let t = std::fs::read_to_string(&c).unwrap().replace("I 0 16", "I 0 17");
    std::fs::write(&c, t).unwrap();
    assert!(Store::open(&d, "gensys", 8).is_err());
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn journal_without_checkpoint_is_bound_to_prefix_and_chunk() {
    let d = std::env::temp_dir().join(format!("gs-bind-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    let mut s = Store::open(&d, "aaa", 8).unwrap();
    s.append(&[(0, 8)]).unwrap();
    drop(s); // no checkpoint written: journal only
    assert!(Store::open(&d, "bbb", 8).is_err(), "other prefix must be refused");
    assert!(Store::open(&d, "aaa", 16).is_err(), "other chunk must be refused");
    assert_eq!(Store::open(&d, "aaa", 8).unwrap().done.total(), 8);
    // headerless (legacy) journal with no checkpoint: refuse
    let j = std::fs::read_to_string(d.join("journal.log")).unwrap();
    std::fs::write(d.join("journal.log"), j.split_once('\n').unwrap().1).unwrap();
    assert!(Store::open(&d, "aaa", 8).is_err());
    let _ = std::fs::remove_dir_all(&d);
}
