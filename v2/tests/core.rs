use std::sync::atomic::AtomicBool;
use vanity_miner::{state::*, *};

fn spec(algo: Algo, enc: Enc, prefix: &str) -> Spec {
    Spec {
        algo,
        enc,
        input: "gensys".into(),
        prefix: prefix.into(),
    }
}

#[test]
fn known_vector_and_proof_roundtrip() {
    // sha256("gensys:0"), cross-checked with Python hashlib
    let sp = spec(Algo::Sha256, Enc::Base64, "mOfz");
    let d = sp.matches(0).unwrap();
    assert_eq!(
        hex::encode(d),
        "98e7f3cc036453eb7cc0c0b11d692cf6b4f4b067811e3fb514a5768ac203a7c5"
    );
    assert_eq!(
        sp.encode(&d),
        "mOfzzANkU+t8wMCxHWks9rT0sGeBHj+1FKV2isIDp8U="
    );
    assert!(verify(&sp.proof(0, &d)).is_ok());
}

#[test]
fn tampered_proofs_fail() {
    let sp = spec(Algo::Sha256, Enc::Base64, "mOfz");
    let good = sp.proof(0, &sp.matches(0).unwrap());
    for (k, v) in [
        ("nonce", serde_json::json!(1)),
        ("prefix", "MOfz".into()),
        ("input", "other".into()),
        ("algorithm", "blake3".into()),
        ("encoding", "hex".into()),
        ("digest", "x".into()),
    ] {
        let mut p = good.clone();
        p[k] = v;
        assert!(verify(&p).is_err(), "tampered {k} accepted");
    }
}

#[test]
fn miner_finds_the_first_reference_hit_for_both_algorithms() {
    for (a, e, p) in [
        (Algo::Sha256, Enc::Base64, "ge"),
        (Algo::Blake3, Enc::Hex, "abc"),
    ] {
        let sp = spec(a, e, p);
        let first = (0u64..).find(|&n| sp.matches(n).is_some()).unwrap();
        let stop = AtomicBool::new(false);
        let s = first / 8 * 8;
        assert_eq!(
            mine_range(&sp, 0, s, &stop, Backend::Lib),
            ChunkResult::Done
        ); // nothing missed before it
        assert_eq!(
            mine_range(&sp, s, 64, &stop, Backend::Lib),
            ChunkResult::Found(first)
        );
    }
}

#[test]
fn state_is_bound_to_settings_and_survives_a_torn_journal() {
    let d = std::env::temp_dir().join(format!("vm-test-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    let (a, b) = (
        spec(Algo::Sha256, Enc::Base64, "aa").key(),
        spec(Algo::Sha256, Enc::Hex, "aa").key(),
    );
    let mut s = Store::open(&d, &a, 8).unwrap();
    s.append(&[(0, 8)]).unwrap();
    drop(s); // journal only, no checkpoint
    assert!(
        Store::open(&d, &b, 8).is_err(),
        "other settings must be refused"
    );
    assert!(
        Store::open(&d, &a, 16).is_err(),
        "other chunk size must be refused"
    );
    use std::io::Write;
    std::fs::OpenOptions::new()
        .append(true)
        .open(d.join("journal.log"))
        .unwrap()
        .write_all(b"D 100 20")
        .unwrap();
    assert_eq!(Store::open(&d, &a, 8).unwrap().done.total(), 8);
    let j = std::fs::read_to_string(d.join("journal.log")).unwrap(); // headerless + no checkpoint: refuse
    std::fs::write(d.join("journal.log"), j.split_once('\n').unwrap().1).unwrap();
    assert!(Store::open(&d, &a, 8).is_err());
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn intervals_merge() {
    let mut i = Intervals::default();
    i.insert(0, 10);
    i.insert(20, 30);
    i.insert(10, 20);
    assert_eq!((i.0.len(), i.next_free(5), i.total()), (1, 30, 30));
}

#[test]
fn unreadable_checkpoint_or_journal_fails_closed() {
    let d = std::env::temp_dir().join(format!("vm-ck-{}", std::process::id()));
    for file in ["checkpoint.txt", "journal.log"] {
        for bad_bytes in [false, true] {
            let _ = std::fs::remove_dir_all(&d);
            std::fs::create_dir_all(&d).unwrap();
            // a directory in place of the file = read error; 0xff bytes = invalid UTF-8
            if bad_bytes {
                std::fs::write(d.join(file), [0xffu8, 0xfe]).unwrap()
            } else {
                std::fs::create_dir(d.join(file)).unwrap()
            }
            assert!(
                Store::open(&d, "k", 8).is_err(),
                "{file} bad_bytes={bad_bytes} was treated as empty"
            );
        }
    }
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn found_json_is_reverified_before_it_is_returned() {
    let sp = spec(Algo::Sha256, Enc::Base64, "mOfz");
    let good = sp.proof(0, &sp.matches(0).unwrap());
    let run = |p: &serde_json::Value, prefix: &str| {
        let d = std::env::temp_dir().join(format!("vm-found-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        std::fs::write(d.join("FOUND.json"), p.to_string()).unwrap();
        let o = std::process::Command::new(env!("CARGO_BIN_EXE_vanity-miner"))
            .args(["--prefix", prefix, "--state"])
            .arg(&d)
            .output()
            .unwrap();
        let _ = std::fs::remove_dir_all(&d);
        o.status.success()
    };
    assert!(
        run(&good, "mOfz"),
        "valid proof for this search must be returned"
    );
    assert!(
        !run(&good, "mOfy"),
        "proof for another search must be refused"
    );
    let mut t = good.clone();
    t["digest"] = "mOfzAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=".into();
    assert!(!run(&t, "mOfz"), "tampered digest must be refused");
    let mut t = good.clone();
    t["message"] = "gensys:1".into();
    assert!(!run(&t, "mOfz"), "inconsistent message must be refused");
}

fn run(args: &[&str]) -> std::process::Output {
    std::process::Command::new(env!("CARGO_BIN_EXE_vanity-miner"))
        .args(args)
        .output()
        .unwrap()
}

#[test]
fn fast_first_word_path_equals_reference() {
    // short messages take the AVX2 first-word path (when available), a long input the portable one
    let mut nonces: Vec<u64> = (0..5_000)
        .chain(99_990..100_010)
        .chain(999_999_990..1_000_000_010)
        .collect();
    nonces.extend((0..20).map(|i| u64::MAX - i));
    for input in ["gensys", &"x".repeat(30), &"x".repeat(40)] {
        let sp = Spec {
            input: input.into(),
            ..spec(Algo::Sha256, Enc::Hex, "0")
        };
        let pre = format!("{input}:").into_bytes();
        for n in [1usize, 7, 8, 9, 128] {
            // partial and full lane groups
            for ch in nonces.chunks(n) {
                let txt: Vec<String> = ch.iter().map(|x| x.to_string()).collect();
                let bodies: Vec<&[u8]> = txt.iter().map(|t| t.as_bytes()).collect();
                let mut out = vec![0u32; ch.len()];
                tape_sha256::hash_short_heads(&pre, &bodies, &mut out);
                for (j, &x) in ch.iter().enumerate() {
                    assert_eq!(
                        out[j],
                        u32::from_be_bytes(sp.digest(x)[..4].try_into().unwrap()),
                        "{input} nonce {x}"
                    );
                }
            }
        }
    }
}

#[test]
fn mine_range_covers_odd_lengths_and_the_top_of_the_nonce_space() {
    let stop = AtomicBool::new(false);
    let sp = spec(Algo::Sha256, Enc::Base64, "ge");
    let first = (0u64..).find(|&n| sp.matches(n).is_some()).unwrap();
    for len in [first + 1, first + 5] {
        assert_eq!(
            mine_range(&sp, 0, len, &stop, Backend::Lib),
            ChunkResult::Found(first)
        );
    }
    assert_eq!(
        mine_range(
            &spec(Algo::Sha256, Enc::Base64, "zzzzzz"),
            7,
            5003,
            &stop,
            Backend::Lib
        ),
        ChunkResult::Done
    );
    // the last mineable nonce is u64::MAX - 1 (20 digits): a hit there must be found
    let top = Spec {
        enc: Enc::Hex,
        ..sp
    };
    let last = u64::MAX - 1;
    let top = Spec {
        prefix: hex::encode(top.digest(last))[..10].into(),
        ..top
    };
    assert_eq!(
        mine_range(&top, NONCE_END - 1000, 1000, &stop, Backend::Lib),
        ChunkResult::Found(last)
    );
}

#[test]
fn nonce_space_end_is_exactly_once_and_exits() {
    // start in the last chunk: [start, 2^64-1) is a short final chunk, then the miner must stop by itself
    let chunk = 1u64 << 18;
    let start = (u64::MAX / chunk * chunk).to_string();
    let d = std::env::temp_dir().join(format!("vm-end-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    let args = [
        "--prefix",
        "ffffffffffffffff",
        "--encoding",
        "hex",
        "--threads",
        "3",
        "--start-nonce",
        &start,
        "--state",
        d.to_str().unwrap(),
    ];
    for _ in 0..2 {
        // second run resumes from the checkpoint and must also exit at once
        let o = run(&args);
        let err = String::from_utf8(o.stderr).unwrap();
        assert_eq!(o.status.code(), Some(3), "{err}");
        assert!(
            err.contains(&format!("EXHAUSTED total_completed={}", chunk - 1)),
            "{err}"
        );
    }
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn avx2_digests_equal_reference() {
    if !avx2::available() {
        eprintln!("note: no AVX2, skipping");
        return;
    }
    let sp = spec(Algo::Sha256, Enc::Base64, "zzzz");
    let pre = b"gensys:".to_vec();
    let mut nonces: Vec<u64> = (0..20_000)
        .chain(99_990..100_010)
        .chain(999_999_990..1_000_000_010)
        .collect();
    nonces.extend((0..20).map(|i| u64::MAX - i));
    for ch in nonces.chunks(8) {
        let txt: Vec<String> = (0..8)
            .map(|j| ch[j.min(ch.len() - 1)].to_string())
            .collect();
        let b: [&[u8]; 8] = std::array::from_fn(|j| txt[j].as_bytes());
        let out = avx2::hash8(&pre, b);
        for j in 0..ch.len() {
            assert_eq!(out[j], sp.digest(ch[j]), "nonce {}", ch[j]);
        }
    }
}

#[test]
fn both_backends_find_the_same_winner() {
    if !avx2::available() {
        eprintln!("note: no AVX2, skipping");
        return;
    }
    let sp = spec(Algo::Sha256, Enc::Base64, "ge");
    let first = (0u64..).find(|&n| sp.matches(n).is_some()).unwrap();
    let stop = AtomicBool::new(false);
    for len in [first + 5, first + 1] {
        // odd lengths exercise the short last lane group
        for be in [Backend::Lib, Backend::Avx2] {
            assert_eq!(
                mine_range(&sp, 0, len, &stop, be),
                ChunkResult::Found(first),
                "{be:?}"
            );
        }
    }
    let none = spec(Algo::Sha256, Enc::Base64, "zzzzzz");
    for be in [Backend::Lib, Backend::Avx2] {
        assert_eq!(mine_range(&none, 7, 5003, &stop, be), ChunkResult::Done);
    }
}

#[test]
fn checkpoints_resume_across_backends() {
    if !avx2::available() {
        eprintln!("note: no AVX2, skipping");
        return;
    }
    let d = std::env::temp_dir().join(format!("vm-xb-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    let ds = d.to_str().unwrap();
    let go = |be: &str| {
        String::from_utf8(
            run(&[
                "--prefix",
                "zzzzzzzzz",
                "--threads",
                "2",
                "--chunk",
                "4096",
                "--max-secs",
                "1",
                "--backend",
                be,
                "--state",
                ds,
            ])
            .stderr,
        )
        .unwrap()
    };
    let total = |e: &str| {
        e.split("completed_nonces=")
            .nth(1)
            .unwrap()
            .split_whitespace()
            .next()
            .unwrap()
            .parse::<u64>()
            .unwrap()
    };
    let a = go("lib");
    assert_eq!(total(&a), 0);
    let b = go("avx2");
    assert!(total(&b) > 0, "avx2 run did not see lib's coverage: {b}");
    let c = go("lib");
    assert!(
        total(&c) > total(&b),
        "lib run did not see avx2's coverage: {c}"
    );
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn avx2_backend_refuses_blake3_and_long_input() {
    let d = std::env::temp_dir().join(format!("vm-bad-{}", std::process::id()));
    let ds = d.to_str().unwrap();
    assert!(!run(&[
        "--prefix",
        "ab",
        "--backend",
        "avx2",
        "--algorithm",
        "blake3",
        "--state",
        ds
    ])
    .status
    .success());
    let long = "x".repeat(40);
    assert!(!run(&[
        "--prefix",
        "ab",
        "--backend",
        "avx2",
        "--input",
        &long,
        "--state",
        ds
    ])
    .status
    .success());
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn nonce_space_end_exits_under_avx2_too() {
    if !avx2::available() {
        return;
    }
    let start = (u64::MAX / (1u64 << 18) * (1u64 << 18)).to_string();
    let d = std::env::temp_dir().join(format!("vm-end2-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    let o = run(&[
        "--prefix",
        "ffffffffffffffff",
        "--encoding",
        "hex",
        "--backend",
        "avx2",
        "--threads",
        "3",
        "--start-nonce",
        &start,
        "--state",
        d.to_str().unwrap(),
    ]);
    assert_eq!(o.status.code(), Some(3));
    let _ = std::fs::remove_dir_all(&d);
}
