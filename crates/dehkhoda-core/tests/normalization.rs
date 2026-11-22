//! نرمال‌ساز Rust باید دقیقاً با نسخهٔ Python (tools/fa_text.py) یکی باشد:
//! هم بردارهای دست‌نویس و هم هزاران رشتهٔ تصادفی با خروجی Python.

use dehkhoda_core::{fold, normalize, NORM_VERSION};
use serde_json::Value;

fn load(name: &str) -> Value {
    let path = format!("{}/../../shared/{}", env!("CARGO_MANIFEST_DIR"), name);
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path}: {e}"));
    serde_json::from_str(&text).unwrap()
}

fn s(v: &Value) -> &str {
    v.as_str().unwrap()
}

fn escape(x: &str) -> String {
    x.escape_unicode().to_string()
}

#[test]
fn hand_written_vectors() {
    let v = load("normalization_vectors.json");
    assert_eq!(v["norm_version"].as_u64().unwrap() as u32, NORM_VERSION);
    let cases = v["cases"].as_array().unwrap();
    assert!(cases.len() >= 30);
    for c in cases {
        let input = s(&c["in"]);
        let n = normalize(input);
        assert_eq!(n, s(&c["norm"]), "norm: {} ({})", s(&c["name"]), escape(input));
        assert_eq!(fold(&n), s(&c["fold"]), "fold: {}", s(&c["name"]));
    }
}

#[test]
fn golden_corpus_from_python() {
    let v = load("normalization_golden.json");
    assert_eq!(v["norm_version"].as_u64().unwrap() as u32, NORM_VERSION);
    let cases = v["cases"].as_array().unwrap();
    assert!(cases.len() >= 4000);
    let mut checked = 0;
    for c in cases {
        let input = s(&c["in"]);
        let n = normalize(input);
        assert_eq!(n, s(&c["norm"]), "norm mismatch for input {}", escape(input));
        assert_eq!(fold(&n), s(&c["fold"]), "fold mismatch for input {}", escape(input));
        checked += 1;
    }
    assert_eq!(checked, cases.len());
}

#[test]
fn idempotent_on_the_golden_inputs() {
    let v = load("normalization_golden.json");
    for c in v["cases"].as_array().unwrap() {
        let n = normalize(s(&c["in"]));
        assert_eq!(normalize(&n), n);
        assert_eq!(fold(&fold(&n)), fold(&n));
        assert_eq!(fold(&n).chars().count(), n.chars().count());
    }
}
