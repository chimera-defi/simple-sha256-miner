use std::sync::atomic::AtomicBool;
use vanity_miner::{state::*, *};

fn spec(algo: Algo, enc: Enc, prefix: &str) -> Spec { Spec { algo, enc, input: "gensys".into(), prefix: prefix.into() } }

#[test]
fn known_vector_and_proof_roundtrip() {
    // sha256("gensys:0"), cross-checked with Python hashlib
    let sp = spec(Algo::Sha256, Enc::Base64, "mOfz");
    let d = sp.matches(0).unwrap();
    assert_eq!(hex::encode(d), "98e7f3cc036453eb7cc0c0b11d692cf6b4f4b067811e3fb514a5768ac203a7c5");
    assert_eq!(sp.encode(&d), "mOfzzANkU+t8wMCxHWks9rT0sGeBHj+1FKV2isIDp8U=");
    assert!(verify(&sp.proof(0, &d)).is_ok());
}

#[test]
fn tampered_proofs_fail() {
    let sp = spec(Algo::Sha256, Enc::Base64, "mOfz");
    let good = sp.proof(0, &sp.matches(0).unwrap());
    for (k, v) in [("nonce", serde_json::json!(1)), ("prefix", "MOfz".into()), ("input", "other".into()),
                   ("algorithm", "blake3".into()), ("encoding", "hex".into()), ("digest", "x".into())] {
        let mut p = good.clone();
        p[k] = v;
        assert!(verify(&p).is_err(), "tampered {k} accepted");
    }
}

#[test]
fn miner_finds_the_first_reference_hit_for_both_algorithms() {
    for (a, e, p) in [(Algo::Sha256, Enc::Base64, "ge"), (Algo::Blake3, Enc::Hex, "abc")] {
        let sp = spec(a, e, p);
        let first = (0u64..).find(|&n| sp.matches(n).is_some()).unwrap();
        let stop = AtomicBool::new(false);
        let s = first / 8 * 8;
        assert_eq!(mine_range(&sp, 0, s, &stop), ChunkResult::Done); // nothing missed before it
        assert_eq!(mine_range(&sp, s, 64, &stop), ChunkResult::Found(first));
    }
}

#[test]
fn state_is_bound_to_settings_and_survives_a_torn_journal() {
    let d = std::env::temp_dir().join(format!("vm-test-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    let (a, b) = (spec(Algo::Sha256, Enc::Base64, "aa").key(), spec(Algo::Sha256, Enc::Hex, "aa").key());
    let mut s = Store::open(&d, &a, 8).unwrap();
    s.append(&[(0, 8)]).unwrap();
    drop(s); // journal only, no checkpoint
    assert!(Store::open(&d, &b, 8).is_err(), "other settings must be refused");
    assert!(Store::open(&d, &a, 16).is_err(), "other chunk size must be refused");
    use std::io::Write;
    std::fs::OpenOptions::new().append(true).open(d.join("journal.log")).unwrap().write_all(b"D 100 20").unwrap();
    assert_eq!(Store::open(&d, &a, 8).unwrap().done.total(), 8);
    let j = std::fs::read_to_string(d.join("journal.log")).unwrap(); // headerless + no checkpoint: refuse
    std::fs::write(d.join("journal.log"), j.split_once('\n').unwrap().1).unwrap();
    assert!(Store::open(&d, &a, 8).is_err());
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn intervals_merge() {
    let mut i = Intervals::default();
    i.insert(0, 10); i.insert(20, 30); i.insert(10, 20);
    assert_eq!((i.0.len(), i.next_free(5), i.total()), (1, 30, 30));
}
