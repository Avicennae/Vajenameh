//! آزمون‌های روی «پایگاه کامل» (۳۱۲٬۵۰۷ مدخل). پیش‌فرض اجرا نمی‌شوند، چون به فایل بزرگ نیاز دارند:
//!
//!     python tools/build_db.py
//!     cargo test -p dehkhoda-core --release --test full_db -- --ignored --nocapture
//!
//! مسیر دیگر: متغیر محیطی DEHKHODA_DB. حتماً با --release اجرا کنید (SQLite در حالت debug بدون بهینه‌سازی ساخته می‌شود).

use std::path::PathBuf;
use std::time::Instant;

use dehkhoda_core::search::MatchKind;
use dehkhoda_core::{fold, normalize, Database, Error};
use rusqlite::{Connection, OpenFlags};

fn db_path() -> PathBuf {
    std::env::var("DEHKHODA_DB")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../src-tauri/resources/dehkhoda.sqlite"))
}

fn open() -> Database {
    Database::open(db_path()).expect("full database not found: run `python tools/build_db.py` first")
}

fn percentile(sorted: &[f64], p: f64) -> f64 {
    let idx = ((sorted.len() as f64 - 1.0) * p).round() as usize;
    sorted[idx]
}

#[test]
#[ignore = "needs the full database"]
fn every_stored_column_matches_the_rust_normalizer() {
    let conn = Connection::open_with_flags(db_path(), OpenFlags::SQLITE_OPEN_READ_ONLY).unwrap();
    let mut st = conn.prepare("SELECT id, word, word_norm, word_fold, meaning_text FROM words").unwrap();
    let mut rows = st.query([]).unwrap();
    let (mut n, mut bad): (usize, Vec<String>) = (0, vec![]);
    while let Some(r) = rows.next().unwrap() {
        let (id, word, wn, wf, mt): (i64, String, String, String, String) =
            (r.get(0).unwrap(), r.get(1).unwrap(), r.get(2).unwrap(), r.get(3).unwrap(), r.get(4).unwrap());
        n += 1;
        let ok = normalize(&word) == wn && fold(&wn) == wf && normalize(&mt) == mt;
        if !ok && bad.len() < 5 {
            bad.push(format!("id {id}: {word:?}"));
        }
    }
    println!("checked {n} rows (word_norm, word_fold, meaning_text) — mismatches: {}", bad.len());
    assert_eq!(n, 312_507);
    assert!(bad.is_empty(), "Rust and Python normalizers disagree: {bad:?}");
}

#[test]
#[ignore = "needs the full database"]
fn known_words_and_homographs() {
    let db = open();
    let i = db.info();
    println!("{i:?}");
    assert_eq!((i.row_count, i.has_fts), (312_507, true));
    let g = db.get_entry_group("آب", None, 0).unwrap().unwrap();
    println!("«آب»: {} entries, html bytes {:?}", g.entries.len(), g.entries.iter().map(|e| e.html.len()).collect::<Vec<_>>());
    assert!(g.entries.len() >= 3, "homographs must be grouped");
    for q in ["آب", "کتاب", "کتابخانه", "دانشگاه", "ابن مقله"] {
        let hits = db.search_words(q, 50).unwrap();
        println!("{q:>10} → {:?}", hits.iter().take(6).map(|h| format!("{}{}", h.word_norm, if h.count > 1 { format!("×{}", h.count) } else { String::new() })).collect::<Vec<_>>());
    }
    assert_eq!(db.search_words("آب", 50).unwrap()[0].word_norm, "آب");
    assert_eq!(db.search_words("کتاب", 50).unwrap()[0].kind, MatchKind::Exact);
}

#[test]
#[ignore = "needs the full database"]
fn giant_homograph_groups_are_paged() {
    let db = open();
    let t = Instant::now();
    let g = db.get_entry_group("احمد", None, 0).unwrap().unwrap();
    println!("«احمد»: total {} entries, first page {} entries ({} KB html) in {:.1} ms", g.total, g.entries.len(), g.entries.iter().map(|e| e.html.len()).sum::<usize>() / 1024, t.elapsed().as_secs_f64() * 1000.0);
    assert_eq!((g.total, g.offset, g.entries.len()), (3012, 0, dehkhoda_core::search::GROUP_PAGE_SIZE));
    // وسط و انتهای گروه، و صفحه‌ای که مدخل هدف را دارد
    let last = db.get_entry_group("احمد", None, 10_000).unwrap().unwrap();
    assert_eq!(last.offset as usize, (3012 - 1) / 40 * 40);
    assert_eq!(last.entries.len(), 3012 - last.offset as usize);
    let target = last.entries.last().unwrap().id;
    let focused = db.get_entry_group("احمد", Some(target), 0).unwrap().unwrap();
    assert_eq!(focused.offset, last.offset);
    assert!(focused.entries.iter().any(|e| e.id == target));
    // گروه‌های کوچک کامل برمی‌گردند
    let small = db.get_entry_group("کتاب", None, 0).unwrap().unwrap();
    assert_eq!((small.total as usize, small.entries.len()), (3, 3));
}

#[test]
#[ignore = "needs the full database"]
fn headword_search_latency_is_under_50ms() {
    let db = open();
    let queries = [
        "ا", "آ", "ب", "ک", "م", "ش", "کا", "آب", "اب", "دا", "کار", "کتا", "کتاب", "دانش", "دانشگ", "مدرس", "مدرسه", "نهال", "شاه",
        "عل", "علی", "موسی", "خانه", "فرهنگ", "زرافه", "قلمدان", "ابن", "بنی", "اب ا", "آب آ", "نیم فاصله", "بسیار", "xyz", "۱۲۳",
    ];
    for q in queries {
        db.search_words(q, 50).unwrap(); // گرم کردن کش
    }
    let mut all: Vec<f64> = Vec::new();
    println!("{:>12} {:>7} {:>8} {:>8} {:>6}", "query", "hits", "median", "max", "kinds(e/p/c)");
    for q in queries {
        let mut ts = Vec::new();
        let mut last = Vec::new();
        for _ in 0..15 {
            let t = Instant::now();
            last = db.search_words(q, 50).unwrap();
            ts.push(t.elapsed().as_secs_f64() * 1000.0);
        }
        ts.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let k = |m: MatchKind| last.iter().filter(|h| h.kind == m).count();
        println!("{q:>12} {:>7} {:>6.2}ms {:>6.2}ms  {}/{}/{}", last.len(), percentile(&ts, 0.5), ts[ts.len() - 1], k(MatchKind::Exact), k(MatchKind::Prefix), k(MatchKind::Contains));
        all.extend(ts);
    }
    all.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let (p50, p95, max) = (percentile(&all, 0.5), percentile(&all, 0.95), all[all.len() - 1]);
    println!("ALL {} runs: p50 {p50:.2} ms, p95 {p95:.2} ms, max {max:.2} ms", all.len());
    assert!(p95 < 50.0, "p95 {p95:.1} ms exceeds the 50 ms target");
}

#[test]
#[ignore = "needs the full database"]
fn entry_group_and_fulltext_timings() {
    let db = open();
    for k in ["آب", "کتاب", "من", "ابن مقله"] {
        let t = Instant::now();
        let g = db.get_entry_group(k, None, 0).unwrap().unwrap();
        println!("get_entry_group({k:?}): {} entries in {:.2} ms", g.entries.len(), t.elapsed().as_secs_f64() * 1000.0);
    }
    let queries = ["خطاط", "آسیاب", "ابن مقله", "\"خط نسخ\"", "وزیر عباسی", "شهری در خوزستان", "اسب", "آب", "و", "زرافه", "فردوسی"];
    db.search_fulltext("خطاط", 0, 5).unwrap(); // گرم کردن
    println!("{:>20} {:>6} {:>10}", "fts query", "hits", "time");
    for q in queries {
        let t = Instant::now();
        let r = db.search_fulltext(q, 0, 50).unwrap();
        println!("{q:>20} {:>6}{} {:>8.1}ms", r.hits.len(), if r.has_more { "+" } else { " " }, t.elapsed().as_secs_f64() * 1000.0);
    }
    let r = db.search_fulltext("وزیر عباسی", 0, 3).unwrap();
    for h in &r.hits {
        let s: String = h.snippet.iter().map(|x| if x.hit { format!("«{}»", x.text) } else { x.text.clone() }).collect();
        println!("  {} → {s}", h.word);
    }
}

#[test]
#[ignore = "needs the full database"]
fn a_newer_fulltext_search_cancels_the_running_one() {
    let db = std::sync::Arc::new(open());
    let heavy = {
        let db = db.clone();
        std::thread::spawn(move || db.search_fulltext("و", 0, 50)) // واژه‌ای که تقریباً در همهٔ مدخل‌ها هست
    };
    std::thread::sleep(std::time::Duration::from_millis(3));
    let quick = db.search_fulltext("خطاط", 0, 5);
    assert!(quick.is_ok(), "{quick:?}");
    match heavy.join().unwrap() {
        Ok(p) => println!("heavy query finished before it could be cancelled ({} hits)", p.hits.len()),
        Err(Error::Cancelled) => println!("heavy query was cancelled by the newer search"),
        Err(e) => panic!("unexpected error: {e}"),
    }
    // و اتصال بعد از قطع شدن سالم است
    assert!(!db.search_fulltext("آسیاب", 0, 5).unwrap().hits.is_empty());
}
