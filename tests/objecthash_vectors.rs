//! object-hash parity suite: every vector was generated from the real npm
//! `object-hash@3.0.0` (the version pinned by the monkeytype frontend and
//! backend) by scripts/gen_objecthash_vectors.mjs. Asserting the serialized
//! byte stream as well as the hash makes any mismatch diagnosable at the exact
//! byte instead of an opaque digest difference (the 461 failure mode).

use serde::Deserialize;

#[derive(Deserialize)]
struct Vector {
    name: String,
    value: serde_json::Value,
    serialized: String,
    hash: String,
}

fn vectors() -> Vec<Vector> {
    serde_json::from_str(include_str!("vectors/objecthash.json")).expect("valid vector file")
}

#[test]
fn serialization_matches_npm_object_hash() {
    for v in vectors() {
        let got = monkeytype_tui::objecthash::serialize(&v.value);
        assert_eq!(
            got, v.serialized,
            "serialization mismatch for vector `{}`",
            v.name
        );
    }
}

#[test]
fn hash_matches_npm_object_hash() {
    for v in vectors() {
        let got = monkeytype_tui::objecthash::object_hash(&v.value);
        assert_eq!(got, v.hash, "hash mismatch for vector `{}`", v.name);
    }
}
