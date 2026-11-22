//! جست‌وجوی متن‌کامل در معنی‌ها (FTS5، جدول بدون‌محتوا روی fold(meaning_text)).
//!
//! ورودی کاربر هرگز مستقیم به MATCH نمی‌رسد: همهٔ واژه‌ها در گیومه قرار می‌گیرند (پس AND/OR/NEAR/*/-
//! و پرانتز معنای ویژه ندارند و خطای نحوی نمی‌دهند). واژه‌های جدا ⇒ AND، «عبارت در گیومه» ⇒ عبارت دقیق،
//! و واژهٔ آخر وقتی کاربر هنوز فاصله نزده پیشوندی است.

use std::sync::atomic::Ordering;
use std::sync::Arc;

use rusqlite::params;
use serde::Serialize;

use crate::normalize::{fold, is_ignored, is_space, normalize};
use crate::snippet::{make_snippet, QueryWord, Segment};
use crate::{Database, Error, Result};

const MAX_TERMS: usize = 12;
const SNIPPET_CHARS: usize = 170;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FtsQuery {
    /// عبارت MATCH آمادهٔ اجرا
    pub match_expr: String,
    /// واژه‌ها برای هایلایت
    pub words: Vec<QueryWord>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct FtsHit {
    pub id: i64,
    pub word: String,
    pub word_norm: String,
    pub snippet: Vec<Segment>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct FtsPage {
    pub hits: Vec<FtsHit>,
    pub has_more: bool,
}

fn alnum_runs(s: &str) -> Vec<String> {
    let mut runs = Vec::new();
    let mut cur = String::new();
    for c in s.chars() {
        if c.is_alphanumeric() {
            cur.push(c);
        } else if !cur.is_empty() {
            runs.push(std::mem::take(&mut cur));
        }
    }
    if !cur.is_empty() {
        runs.push(cur);
    }
    runs
}

/// کاربر هنوز در حال تایپ واژهٔ آخر است؟ آخرین نویسهٔ «دیدنی» ورودی خام حرف/رقم باشد
/// (نه فاصله یا گیومه). باید روی ورودی خام سنجید: `normalize` فاصلهٔ آخر را حذف می‌کند.
fn is_typing(raw: &str) -> bool {
    raw.chars().rev().find(|&c| !is_ignored(c)).map_or(false, |c| !is_space(c) && c.is_alphanumeric())
}

/// ورودی خام را به عبارت MATCH امن تبدیل می‌کند؛ اگر واژهٔ قابل جست‌وجویی نبود `None`.
pub fn build_fts_query(raw: &str) -> Option<FtsQuery> {
    let folded = fold(&normalize(raw));
    let typing = is_typing(raw);

    // جدا کردن بخش‌های داخل و بیرون گیومه
    let mut segments: Vec<(String, bool)> = Vec::new();
    let (mut cur, mut quoted) = (String::new(), false);
    for c in folded.chars() {
        if c == '"' {
            segments.push((std::mem::take(&mut cur), quoted));
            quoted = !quoted;
        } else {
            cur.push(c);
        }
    }
    segments.push((cur, quoted));

    // (اجزای عبارت، آیا گیومه‌ای بوده)
    let mut terms: Vec<(Vec<String>, bool)> = Vec::new();
    for (seg, q) in segments {
        if q {
            let runs = alnum_runs(&seg);
            if !runs.is_empty() {
                terms.push((runs, true));
            }
        } else {
            for piece in seg.split_whitespace() {
                let runs = alnum_runs(piece);
                if !runs.is_empty() {
                    terms.push((runs, false));
                }
            }
        }
    }
    terms.truncate(MAX_TERMS);
    if terms.is_empty() {
        return None;
    }

    let last = terms.len() - 1;
    let mut parts = Vec::with_capacity(terms.len());
    let mut words = Vec::new();
    for (i, (runs, was_quoted)) in terms.iter().enumerate() {
        let prefix = typing && i == last && !was_quoted;
        let body = runs.join(" ").replace('"', "\"\"");
        parts.push(if prefix { format!("\"{body}\"*") } else { format!("\"{body}\"") });
        for (j, r) in runs.iter().enumerate() {
            words.push(QueryWord { text: r.clone(), prefix: prefix && j == runs.len() - 1 });
        }
    }
    Some(FtsQuery { match_expr: parts.join(" "), words })
}

impl Database {
    /// جست‌وجوی متن‌کامل؛ نتایج به ترتیب ارتباط (bm25). `offset/limit` برای صفحه‌بندی.
    /// اگر جست‌وجوی تازه‌تری شروع شود، این جست‌وجو با `Error::Cancelled` قطع می‌شود.
    pub fn search_fulltext(&self, query: &str, offset: usize, limit: usize) -> Result<FtsPage> {
        if !self.info().has_fts {
            return Err(Error::NoFullText);
        }
        let Some(q) = build_fts_query(query) else {
            return Ok(FtsPage { hits: vec![], has_more: false });
        };
        let limit = limit.clamp(1, 200);

        let my_gen = self.fts_generation.fetch_add(1, Ordering::SeqCst) + 1;
        let generation = Arc::clone(&self.fts_generation);

        self.with_conn(|conn| {
            conn.progress_handler(1000, Some(move || generation.load(Ordering::SeqCst) != my_gen))?;
            let result = (|| -> Result<FtsPage> {
                let ids: Vec<i64> = {
                    let mut st = conn.prepare_cached(
                        "SELECT rowid FROM words_fts WHERE words_fts MATCH ?1 ORDER BY rank LIMIT ?2 OFFSET ?3",
                    )?;
                    let rows = st.query_map(params![q.match_expr, (limit + 1) as i64, offset as i64], |r| r.get(0))?;
                    rows.collect::<rusqlite::Result<Vec<_>>>()?
                };
                let has_more = ids.len() > limit;
                let mut st = conn.prepare_cached("SELECT word, word_norm, meaning_text FROM words WHERE id = ?1")?;
                let mut hits = Vec::with_capacity(limit.min(ids.len()));
                for &id in ids.iter().take(limit) {
                    let (word, word_norm, text): (String, String, String) =
                        st.query_row(params![id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?;
                    hits.push(FtsHit { id, word, word_norm, snippet: make_snippet(&text, &q.words, SNIPPET_CHARS) });
                }
                Ok(FtsPage { hits, has_more })
            })();
            let _ = conn.progress_handler(1000, None::<fn() -> bool>);
            result
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn q(raw: &str) -> Option<String> {
        build_fts_query(raw).map(|x| x.match_expr)
    }

    #[test]
    fn terms_are_quoted_and_anded_and_last_is_prefix() {
        assert_eq!(q("خط نسخ").unwrap(), "\"خط\" \"نسخ\"*");
        assert_eq!(q("خط نسخ ").unwrap(), "\"خط\" \"نسخ\""); // فاصلهٔ آخر ⇒ دیگر پیشوندی نیست
    }

    #[test]
    fn fold_is_applied() {
        assert_eq!(q("آسیاب").unwrap(), "\"اسیاب\"*");
    }

    #[test]
    fn phrase_in_quotes() {
        assert_eq!(q("\"خط نسخ\"").unwrap(), "\"خط نسخ\"");
        assert_eq!(q("جای \"خط نسخ\" ").unwrap(), "\"جای\" \"خط نسخ\"");
        assert_eq!(q("\"خط نس").unwrap(), "\"خط نس\""); // گیومهٔ بسته‌نشده ⇒ عبارت
    }

    #[test]
    fn dangerous_input_is_neutralised() {
        for raw in ["AND OR NOT NEAR(", "a* -b +c ^d", "(x OR y) AND z", "col:val", "\"\"\"", "'; DROP TABLE words; --", "﴾ ﴿"] {
            if let Some(expr) = q(raw) {
                assert!(expr.starts_with('"'), "{expr}");
            }
        }
        assert_eq!(q("   "), None);
        assert_eq!(q("،؛ - ..."), None);
        assert_eq!(q(""), None);
    }

    #[test]
    fn hyphenated_piece_becomes_phrase_of_runs() {
        let x = build_fts_query("آب-آسیا ").unwrap();
        assert_eq!(x.match_expr, "\"اب اسیا\"");
        assert_eq!(x.words.len(), 2);
    }

    #[test]
    fn term_count_is_capped() {
        let raw = (0..40).map(|i| format!("w{i} ")).collect::<String>();
        assert_eq!(build_fts_query(&raw).unwrap().match_expr.matches('"').count(), MAX_TERMS * 2);
    }

    #[test]
    fn highlight_words_mark_prefix_only_on_last() {
        let x = build_fts_query("خط نس").unwrap();
        assert_eq!(x.words, vec![QueryWord { text: "خط".into(), prefix: false }, QueryWord { text: "نس".into(), prefix: true }]);
    }
}
