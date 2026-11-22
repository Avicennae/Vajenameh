//! جست‌وجو روی پایگاه کوچک آزمایشی (tests/fixtures/mini.sqlite) که با tools/build_db.py ساخته شده
//! (همان کدِ ساخت، همان طرحوارهٔ پایگاه اصلی). منبع داده: tools/tests/fixtures/mini/*.sql

use std::collections::BTreeSet;
use std::path::PathBuf;
use std::sync::Arc;

use dehkhoda_core::search::MatchKind::{Contains, Exact, Prefix};
use dehkhoda_core::{Database, Error};

fn fixture() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/mini.sqlite")
}

fn db() -> Database {
    Database::open(fixture()).unwrap()
}

fn words(q: &str) -> Vec<(String, dehkhoda_core::search::MatchKind)> {
    db().search_words(q, 50).unwrap().into_iter().map(|h| (h.word_norm, h.kind)).collect()
}

fn names(q: &str) -> Vec<String> {
    words(q).into_iter().map(|(w, _)| w).collect()
}

fn ids(page: &dehkhoda_core::fulltext::FtsPage) -> BTreeSet<i64> {
    page.hits.iter().map(|h| h.id).collect()
}

#[test]
fn info_is_read_from_meta() {
    let d = db();
    let i = d.info();
    assert_eq!((i.row_count, i.distinct_words, i.schema_version, i.norm_version), (29, 27, 2, 1));
    assert!(i.has_fts);
    assert!(i.built_at.ends_with('Z'));
}

#[test]
fn madda_query_ranks_exact_spelling_first_then_prefix_then_contains() {
    let got = words("آب");
    let expect = [
        ("آب", Exact), ("اب", Exact),
        ("آباد", Prefix), ("آبادان", Prefix), ("آب آسیا", Prefix), ("ابار", Prefix), ("ابن مقله", Prefix),
        ("کتاب", Contains), ("آسیاب", Contains), ("کتابخانه", Contains), ("کتابفروش", Contains),
    ];
    let expect: Vec<_> = expect.iter().map(|(w, k)| (w.to_string(), *k)).collect();
    assert_eq!(got, expect);
}

#[test]
fn plain_alef_query_finds_madda_words_but_ranks_exact_spelling_higher() {
    let got = names("اب");
    assert_eq!(&got[..2], ["اب", "آب"]); // املای دقیق اول، ولی «آب» هم پیدا می‌شود
    assert_eq!(&got[2..4], ["ابار", "ابن مقله"]); // پیشوندی با املای دقیق
    assert_eq!(&got[4..7], ["آباد", "آبادان", "آب آسیا"]); // پیشوندی فقط با fold
}

#[test]
fn prefix_results_are_shortest_first() {
    let got = words("کار");
    assert_eq!(got[0], ("کار".to_string(), Exact));
    let rest: Vec<String> = got[1..]
        .iter()
        .map(|(w, k)| {
            assert_eq!(*k, Prefix);
            w.clone()
        })
        .collect();
    let mut by_length = rest.clone();
    by_length.sort_by_key(|w| (w.chars().count(), w.clone()));
    assert_eq!(rest, by_length, "کوتاه‌ترها اول، بعد به ترتیب کاراکتر");
    let mut found = rest.clone();
    found.sort();
    let mut expect: Vec<String> = ["کارد", "کارگاه", "کارگر", "کاروان"].iter().map(|w| w.to_string()).collect();
    expect.sort();
    assert_eq!(found, expect);
}

#[test]
fn contains_only_after_prefix_and_never_for_one_char() {
    assert_eq!(words("بار"), [("بار", Exact), ("باران", Prefix), ("ابار", Contains), ("انبار", Contains)].map(|(w, k)| (w.to_string(), k)).to_vec());
    let one = words("آ");
    assert!(one.iter().all(|(_, k)| *k == Prefix), "{one:?}"); // تک‌حرفی: بدون مرحلهٔ «شامل»
    // مدخل‌های با «آ» (املای دقیق) قبل از «ا»ها
    let n: Vec<_> = one.iter().map(|(w, _)| w.as_str()).collect();
    assert_eq!(&n[..6], ["آب", "آباد", "آمدن", "آسیاب", "آبادان", "آب آسیا"]);
    assert_eq!(&n[6..], ["اب", "ابار", "انبار", "ابن مقله"]);
}

#[test]
fn homographs_are_listed_once_with_a_count() {
    let hits = db().search_words("آب", 50).unwrap();
    assert_eq!((hits[0].word_norm.as_str(), hits[0].id, hits[0].count), ("آب", 1, 3));
    assert_eq!(hits.iter().filter(|h| h.word_norm == "آب").count(), 1);
}

#[test]
fn user_input_is_normalised() {
    // ك عربی، ZWNJ بین دو واژه، فاصله‌های اضافه، همزهٔ ترکیبی، اعراب
    assert_eq!(names("كار")[0], "کار");
    assert_eq!(words("آب\u{200C}آسیا")[0], ("آب آسیا".to_string(), Exact));
    assert_eq!(words("  آب   آسیا ")[0], ("آب آسیا".to_string(), Exact));
    assert_eq!(words("خانهٔ")[0], ("خانه".to_string(), Exact));
    assert_eq!(words("خانۀ")[0], ("خانه".to_string(), Exact));
    assert_eq!(words("كِتَاب")[0], ("کتاب".to_string(), Exact));
    assert_eq!(words("موسى")[0], ("موسی".to_string(), Exact));
    assert_eq!(words("علي")[0], ("علی".to_string(), Exact));
    // «مَقلة» (اعراب + ة) ⇒ «مقله»؛ سرواژهٔ مستقلی نیست ولی در «ابن مقله» (مرحلهٔ «شامل») پیدا می‌شود
    assert_eq!(words("مَقلة"), vec![("ابن مقله".to_string(), Contains)]);
}

#[test]
fn stored_arabic_form_is_found_with_persian_keyboard() {
    // سرواژهٔ «كوه» (ك عربی) در فیکسچر هست؛ با «کوه» فارسی پیدا می‌شود
    assert_eq!(words("کوه")[0], ("کوه".to_string(), Exact));
}

#[test]
fn degenerate_and_hostile_queries_do_not_error() {
    let d = db();
    for q in ["", "   ", "\u{200C}", "\u{064E}\u{0651}", "%", "_", "'", "\"", "a'; DROP TABLE words;--", "آ%", "\\", &"آ".repeat(5000)] {
        let r = d.search_words(q, 50);
        assert!(r.is_ok(), "{q:?}: {r:?}");
    }
    assert!(d.search_words("", 50).unwrap().is_empty());
    assert!(d.search_words("zzzzzz", 50).unwrap().is_empty());
}

#[test]
fn limit_is_respected_and_clamped() {
    let d = db();
    assert_eq!(d.search_words("ا", 3).unwrap().len(), 3);
    assert_eq!(d.search_words("ا", 0).unwrap().len(), 1);
    assert!(d.search_words("ا", 10_000).unwrap().len() <= 200);
}

#[test]
fn entry_group_shows_all_homographs_together() {
    let d = db();
    let g = d.get_entry_group("آب", None, 0).unwrap().unwrap();
    assert_eq!(g.word_norm, "آب");
    assert_eq!(g.entries.iter().map(|e| e.id).collect::<Vec<_>>(), [1, 2, 3]);
    assert!(g.entries[0].html.contains("فردوسی"));
    // «اب» سرواژهٔ جدایی است، نه بخشی از «آب»
    let g = d.get_entry_group("اب", None, 0).unwrap().unwrap();
    assert_eq!(g.entries.iter().map(|e| e.id).collect::<Vec<_>>(), [4]);
    // ورودی خام هم نرمال می‌شود
    assert_eq!(d.get_entry_group("كتاب", None, 0).unwrap().unwrap().entries[0].id, 11);
    // بدون املای دقیق، با fold پیدا می‌شود
    assert_eq!(d.get_entry_group("اسیاب", None, 0).unwrap().unwrap().word_norm, "آسیاب");
    assert!(d.get_entry_group("آبی", None, 0).unwrap().is_none());
    assert!(d.get_entry_group("", None, 0).unwrap().is_none());
}

#[test]
fn entry_group_reports_total_and_offset() {
    let g = db().get_entry_group("آب", None, 0).unwrap().unwrap();
    assert_eq!((g.total, g.offset, g.entries.len(), g.word.as_str()), (3, 0, 3, "آب"));
}

#[test]
fn large_groups_are_paged_and_a_focused_entry_selects_its_page() {
    let d = db();
    let page = |focus: Option<i64>, offset: usize| {
        let g = d.get_entry_group_paged("آب", focus, offset, 2).unwrap().unwrap();
        (g.entries.iter().map(|e| e.id).collect::<Vec<_>>(), g.offset, g.total)
    };
    assert_eq!(page(None, 0), (vec![1, 2], 0, 3));
    assert_eq!(page(None, 2), (vec![3], 2, 3));
    assert_eq!(page(None, 3), (vec![3], 2, 3)); // مضرب اندازهٔ صفحه می‌شود
    assert_eq!(page(None, 99), (vec![3], 2, 3)); // بیرون از گروه ⇒ آخرین صفحه
    assert_eq!(page(Some(3), 0), (vec![3], 2, 3)); // مدخل هدف، صفحه را تعیین می‌کند
    assert_eq!(page(Some(1), 5), (vec![1, 2], 0, 3)); // و بر offset اولویت دارد
    assert_eq!(page(Some(4), 0), (vec![1, 2], 0, 3)); // id ی که عضو این گروه نیست نادیده گرفته می‌شود
    let one = d.get_entry_group_paged("آب", None, 1, 0).unwrap().unwrap(); // اندازهٔ صفحهٔ ۰ ⇒ ۱
    assert_eq!((one.entries.len(), one.offset), (1, 1));
}

#[test]
fn entry_html_is_sanitised() {
    let g = db().get_entry_group("آمدن", None, 0).unwrap().unwrap();
    let h = &g.entries[0].html;
    for bad in ["<script", "alert", "onclick", "onerror", "onmouseover", "javascript", "<img", "style=", "href", "size=", "#00ff00", "evil"] {
        assert!(!h.contains(bad), "{bad} in {h}");
    }
    for good in ["class=\"hlight\"", "dir=\"rtl\"", "<font color=\"red\">قرمز</font>", "پانویس", "سبز"] {
        assert!(h.contains(good), "{good} missing in {h}");
    }
}

#[test]
fn entry_html_keeps_the_data_markup() {
    let g = db().get_entry_group("آب", None, 0).unwrap().unwrap();
    let h = &g.entries[0].html;
    assert!(h.contains("<span class=\"hlight\">آب .</span>"));
    assert!(h.contains("<font color=\"blue\">◄</font>"));
    assert!(h.contains("<br>") && h.contains("<font color=\"red\">فردوسی .</font>"));
    assert!(!h.contains("</div>") && !h.contains('\\'));
}

#[test]
fn fulltext_basic_queries() {
    let d = db();
    let p = d.search_fulltext("خطاط", 0, 10).unwrap();
    assert_eq!(ids(&p), BTreeSet::from([9]));
    let hit = &p.hits[0];
    assert_eq!((hit.word.as_str(), hit.word_norm.as_str()), ("ابن مقلة", "ابن مقله"));
    assert!(hit.snippet.iter().any(|s| s.hit && s.text == "خطاط"), "{:?}", hit.snippet);

    // fold: با و بدون «آ» یکی است، و در خلاصه «آ» اصلی دیده می‌شود
    for q in ["اسیاب", "آسیاب"] {
        let p = d.search_fulltext(q, 0, 10).unwrap();
        assert_eq!(ids(&p), BTreeSet::from([8]), "{q}");
        assert!(p.hits[0].snippet.iter().any(|s| s.hit && s.text == "آسیاب"));
    }
    assert_eq!(ids(&d.search_fulltext("اسیا", 0, 10).unwrap()), BTreeSet::from([7, 8])); // پیشوندی
    assert_eq!(ids(&d.search_fulltext("خط نسخ", 0, 10).unwrap()), BTreeSet::from([9, 12])); // AND
    assert_eq!(ids(&d.search_fulltext("\"خط نسخ\"", 0, 10).unwrap()), BTreeSet::from([9])); // عبارت
    assert_eq!(ids(&d.search_fulltext("جای نگهداری", 0, 10).unwrap()), BTreeSet::from([12, 22]));
    assert_eq!(ids(&d.search_fulltext("نگهداری کالا", 0, 10).unwrap()), BTreeSet::from([22]));
    // «غُرّه ٔ آن» در متن و ورودی هر دو نرمال می‌شوند (اعراب، تشدید، همزهٔ ترکیبی، آ→ا)
    assert_eq!(ids(&d.search_fulltext("غره ان", 0, 10).unwrap()), BTreeSet::from([2]));
    assert_eq!(ids(&d.search_fulltext("غُرّه ٔ خانه", 0, 10).unwrap()), BTreeSet::from([25]));
    assert_eq!(ids(&d.search_fulltext("غره", 0, 10).unwrap()), BTreeSet::from([2, 25]));
    assert!(d.search_fulltext("زرافه", 0, 10).unwrap().hits.is_empty());
}

#[test]
fn fulltext_paging() {
    let d = db();
    let mut all = BTreeSet::new();
    let mut offset = 0;
    loop {
        let p = d.search_fulltext("جای", offset, 2).unwrap();
        assert!(p.hits.len() <= 2);
        all.extend(ids(&p));
        offset += 2;
        if !p.has_more {
            break;
        }
        assert!(offset < 20);
    }
    assert_eq!(all, BTreeSet::from([8, 12, 17, 22, 25]));
}

#[test]
fn fulltext_hostile_queries_do_not_error() {
    let d = db();
    for q in ["AND", "OR", "NOT", "NEAR(", "(", ")", "\"", "\"\"", "a*b", "خط OR نسخ", "NOT خط", "col:x", "' OR 1=1 --", "*", "-", "^خط", "{خط}", "", "   ", "،؛"] {
        let r = d.search_fulltext(q, 0, 10);
        assert!(r.is_ok(), "{q:?}: {r:?}");
    }
}

#[test]
fn fulltext_unavailable_is_a_clear_error() {
    let dir = tempfile::tempdir().unwrap();
    let copy = dir.path().join("nofts.sqlite");
    std::fs::copy(fixture(), &copy).unwrap();
    rusqlite::Connection::open(&copy).unwrap().execute("UPDATE meta SET value='0' WHERE key='has_fts'", []).unwrap();
    let d = Database::open(&copy).unwrap();
    assert!(matches!(d.search_fulltext("آب", 0, 10), Err(Error::NoFullText)));
    assert!(!d.search_words("آب", 10).unwrap().is_empty()); // جست‌وجوی واژه همچنان کار می‌کند
}

#[test]
fn incompatible_or_missing_databases_are_rejected() {
    let dir = tempfile::tempdir().unwrap();
    // نسخهٔ نرمال‌سازی نامتناسب
    let bad = dir.path().join("bad.sqlite");
    std::fs::copy(fixture(), &bad).unwrap();
    rusqlite::Connection::open(&bad).unwrap().execute("UPDATE meta SET value='99' WHERE key='norm_version'", []).unwrap();
    assert!(matches!(Database::open(&bad), Err(Error::IncompatibleDb(m)) if m.contains("99")));
    // فایلی که پایگاه نیست
    let junk = dir.path().join("junk.sqlite");
    std::fs::write(&junk, "این یک پایگاه داده نیست").unwrap();
    assert!(Database::open(&junk).is_err());
    // پایگاه SQLite دیگری
    let other = dir.path().join("other.sqlite");
    rusqlite::Connection::open(&other).unwrap().execute_batch("CREATE TABLE t(a)").unwrap();
    assert!(matches!(Database::open(&other), Err(Error::IncompatibleDb(_))));
    // فایل ناموجود
    assert!(matches!(Database::open(dir.path().join("nope.sqlite")), Err(Error::Io(_))));
}

#[test]
fn database_is_opened_read_only_and_leaves_no_side_files() {
    let dir = tempfile::tempdir().unwrap();
    let copy = dir.path().join("ro.sqlite");
    std::fs::copy(fixture(), &copy).unwrap();
    let d = Database::open(&copy).unwrap();
    d.search_words("آب", 10).unwrap();
    d.search_fulltext("آب", 0, 10).unwrap();
    let files: Vec<_> = std::fs::read_dir(dir.path()).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().into_owned()).collect();
    assert_eq!(files, ["ro.sqlite"], "{files:?}");
    // نوشتن ممکن نیست (حتی اگر کسی SQL بفرستد)
    let ro = rusqlite::Connection::open_with_flags(&copy, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY).unwrap();
    assert!(ro.execute("DELETE FROM words", []).is_err());
}

#[test]
fn concurrent_searches_agree_with_sequential_results() {
    let d = Arc::new(db());
    let queries = ["آب", "اب", "کار", "بار", "کتاب", "آ", "خانه"];
    let expect: Vec<_> = queries.iter().map(|q| d.search_words(q, 50).unwrap()).collect();
    let threads: Vec<_> = (0..8)
        .map(|t| {
            let d = Arc::clone(&d);
            let expect = expect.clone();
            std::thread::spawn(move || {
                for i in 0..60 {
                    let k = (i + t) % queries.len();
                    assert_eq!(d.search_words(queries[k], 50).unwrap(), expect[k]);
                    d.search_fulltext("جای", 0, 5).ok(); // ممکن است توسط نخ دیگر Cancelled شود؛ مهم این است که نمی‌ترکد
                    d.get_entry_group(queries[k], None, 0).unwrap();
                }
            })
        })
        .collect();
    for t in threads {
        t.join().unwrap();
    }
}
