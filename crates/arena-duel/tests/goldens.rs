//! Every command and event of 120 s of brain-against-brain play, hashed per seed: a change to any rule, roll or
//! number shows here. A change meant to alter fights rewrites the file: delete it and run the test once.

const FILE: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/goldens/event_hashes.json"
);

/// Each seed's hash, as the file holds them (`arena_duel::fingerprint`, which the wasm check works out too).
fn hashes() -> String {
    arena_duel::fingerprint::event_hashes_json(20, 120.0)
}

#[test]
fn event_log_hashes_are_pinned() {
    let got = hashes();
    match std::fs::read_to_string(FILE) {
        Ok(want) => assert_eq!(
            got, want,
            "the fights changed: if that was meant, delete {FILE} and run again"
        ),
        Err(_) => {
            assert!(std::fs::write(FILE, &got).is_ok(), "could not write {FILE}");
            eprintln!("wrote {FILE}: the first run pins the hashes");
        }
    }
}
