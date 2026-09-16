//! Test G (determinism): same seed → byte-identical state across reruns, thread counts, and
//! save→load→continue. These tests are the CI gate for every future PR.

use chronica_engine::persistence::{load_from_file, save_to_file};
use chronica_engine::sim::{Config, Sim};

fn cfg() -> Config {
    Config { seed: 12345, width: 96, height: 64 }
}

#[test]
fn rerun_is_byte_identical() {
    let mut a = Sim::new(cfg());
    let mut b = Sim::new(cfg());
    a.run_days(120);
    b.run_days(120);
    assert_eq!(a.to_bytes(), b.to_bytes(), "two runs of the same seed must be byte-identical");
}

#[test]
fn different_seeds_differ() {
    let mut a = Sim::new(cfg());
    let mut b = Sim::new(Config { seed: 54321, ..cfg() });
    a.run_days(30);
    b.run_days(30);
    assert_ne!(a.state_hash(), b.state_hash());
}

#[test]
fn save_load_continue_equals_uninterrupted() {
    let dir = std::env::temp_dir().join("chronica_test_saves");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("det_midpoint.crn");

    let mut whole = Sim::new(cfg());
    whole.run_days(200);

    let mut first = Sim::new(cfg());
    first.run_days(100);
    save_to_file(&first, &path).unwrap();
    let mut resumed = load_from_file(&path).unwrap();
    resumed.run_days(100);

    assert_eq!(
        whole.to_bytes(),
        resumed.to_bytes(),
        "save→load→continue must equal the uninterrupted run"
    );
}

#[test]
fn thread_count_invariance() {
    // Serial pool vs default (all-core) pool must agree exactly.
    let one = rayon::ThreadPoolBuilder::new().num_threads(1).build().unwrap();
    let hash_serial = one.install(|| {
        let mut s = Sim::new(cfg());
        s.run_days(90);
        s.state_hash()
    });
    let many = rayon::ThreadPoolBuilder::new().num_threads(8).build().unwrap();
    let hash_parallel = many.install(|| {
        let mut s = Sim::new(cfg());
        s.run_days(90);
        s.state_hash()
    });
    assert_eq!(hash_serial, hash_parallel, "thread count must not change outcomes");
}
