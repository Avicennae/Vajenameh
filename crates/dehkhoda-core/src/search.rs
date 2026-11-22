//! جست‌وجوی سرواژه‌ها و دریافت مدخل‌ها.
//!
//! ترتیب نتایج: ۱) تطابق کامل ۲) پیشوندی ۳) شامل‌شدن؛ در هر مرحله «املای دقیق» بالاتر است
//! (برای ورودی «اب»، سرواژهٔ «اب» قبل از «آب» می‌آید و بالعکس) و سپس کوتاه‌ترها.
//! سرواژه‌های هم‌املا (چند ردیف با یک `word_norm`) در فهرست یک‌بار می‌آیند (با شمارنده) و
//! صفحهٔ معنی همهٔ آن‌ها را با هم نشان می‌دهد.

use rusqlite::{params, Row};
use serde::Serialize;

use crate::normalize::{fold, normalize};
use crate::sanitize::sanitize_meaning_html;
use crate::{Database, Result};

/// حداکثر طول ورودی جست‌وجو (به کاراکتر)؛ ورودی‌های عجیب و بلند هزینهٔ بیخود نمی‌سازند.
pub const MAX_QUERY_CHARS: usize = 200;
pub const DEFAULT_LIMIT: usize = 50;
/// حداکثر مدخل در یک صفحهٔ گروه هم‌املا (۹۹٫۳٪ سرواژه‌ها کوچک‌تر از این‌اند و کامل نمایش داده می‌شوند)
pub const GROUP_PAGE_SIZE: usize = 40;

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum MatchKind {
    Exact,
    Prefix,
    Contains,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct WordHit {
    /// کوچک‌ترین id میان مدخل‌های هم‌املا
    pub id: i64,
    pub word: String,
    pub word_norm: String,
    /// تعداد مدخل‌های هم‌املا (≥ ۱)
    pub count: u32,
    pub kind: MatchKind,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Entry {
    pub id: i64,
    pub word: String,
    /// HTML پاک‌سازی‌شده (فقط span/br/font/b/i/a و کلاس‌های مجاز)
    pub html: String,
}

/// مدخل‌های هم‌املا. گروه‌های بزرگ (مثلاً «احمد» با ۳٬۰۱۲ مدخل) صفحه‌بندی می‌شوند.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct EntryGroup {
    pub word_norm: String,
    pub word: String,
    /// تعداد کل مدخل‌های گروه
    pub total: u32,
    /// اندیس نخستین مدخل این صفحه (همیشه مضرب اندازهٔ صفحه)
    pub offset: u32,
    /// اندازهٔ صفحه (برای دکمه‌های «قبلی/بعدی»)
    pub page_size: u32,
    pub entries: Vec<Entry>,
}

// همهٔ مراحل روی جدول کوچک `heads` (یک ردیف به‌ازای هر word_norm) اجرا می‌شوند؛ نه روی words.
const SQL_EXACT: &str = "SELECT id, word, word_norm, cnt FROM heads \
     WHERE word_fold = ?1 \
     ORDER BY (word_norm = ?2) DESC, word_norm LIMIT ?3";

// word_fold > qf  ⇒ کاملاً برابر نیست؛ < qf+U+10FFFF ⇒ با qf شروع می‌شود
const SQL_PREFIX: &str = "SELECT id, word, word_norm, cnt FROM heads \
     WHERE word_fold > ?1 AND word_fold < ?2 \
     ORDER BY (substr(word_norm, 1, ?3) = ?4) DESC, length(word_norm), word_norm LIMIT ?5";

// «شامل»، ۳ کاراکتر و بیشتر: ایندکس trigram. instr > 1 ⇒ با آن شروع نمی‌شود (در مرحلهٔ پیشوندی آمده)،
// و رد کردن نتایج احتمالاً کاذب را هم انجام می‌دهد.
const SQL_CONTAINS_TRGM: &str = "SELECT h.id, h.word, h.word_norm, h.cnt FROM heads_trgm \
     JOIN heads h ON h.id = heads_trgm.rowid \
     WHERE heads_trgm MATCH ?1 AND instr(h.word_fold, ?2) > 1 \
     ORDER BY length(h.word_norm), h.word_norm LIMIT ?3";

// «شامل»، دقیقاً ۲ کاراکتر (trigram کمتر از ۳ کاراکتر را نمی‌شناسد): پیمایش جدول فشردهٔ heads
const SQL_CONTAINS_SCAN: &str = "SELECT id, word, word_norm, cnt FROM heads \
     WHERE instr(word_fold, ?1) > 1 \
     ORDER BY length(word_norm), word_norm LIMIT ?2";

fn truncate_chars(s: &str, max: usize) -> &str {
    match s.char_indices().nth(max) {
        Some((i, _)) => &s[..i],
        None => s,
    }
}

fn hit_from_row(row: &Row<'_>, kind: MatchKind) -> rusqlite::Result<WordHit> {
    Ok(WordHit { id: row.get(0)?, word: row.get(1)?, word_norm: row.get(2)?, count: row.get::<_, i64>(3)? as u32, kind })
}

impl Database {
    /// جست‌وجوی سرواژه. ورودی خام کاربر است (نرمال‌سازی همین‌جا انجام می‌شود).
    pub fn search_words(&self, query: &str, limit: usize) -> Result<Vec<WordHit>> {
        let limit = limit.clamp(1, 200);
        let qn = normalize(truncate_chars(query, MAX_QUERY_CHARS));
        if qn.is_empty() {
            return Ok(Vec::new());
        }
        let qf = fold(&qn);
        let qlen = qn.chars().count() as i64;
        let upper = format!("{qf}\u{10FFFF}");

        self.with_conn(|conn| {
            let mut hits: Vec<WordHit> = Vec::with_capacity(limit);

            let mut st = conn.prepare_cached(SQL_EXACT)?;
            for r in st.query_map(params![qf, qn, limit as i64], |row| hit_from_row(row, MatchKind::Exact))? {
                hits.push(r?);
            }

            if hits.len() < limit {
                let mut st = conn.prepare_cached(SQL_PREFIX)?;
                let remaining = (limit - hits.len()) as i64;
                for r in st.query_map(params![qf, upper, qlen, qn, remaining], |row| hit_from_row(row, MatchKind::Prefix))? {
                    hits.push(r?);
                }
            }

            if hits.len() < limit && qlen >= 2 {
                let remaining = (limit - hits.len()) as i64;
                let contains = |row: &Row<'_>| hit_from_row(row, MatchKind::Contains);
                if qlen >= 3 {
                    let phrase = format!("\"{}\"", qf.replace('"', "\"\""));
                    let mut st = conn.prepare_cached(SQL_CONTAINS_TRGM)?;
                    for r in st.query_map(params![phrase, qf, remaining], contains)? {
                        hits.push(r?);
                    }
                } else {
                    let mut st = conn.prepare_cached(SQL_CONTAINS_SCAN)?;
                    for r in st.query_map(params![qf, remaining], contains)? {
                        hits.push(r?);
                    }
                }
            }
            Ok(hits)
        })
    }

    /// مدخل‌های هم‌املا با `key` (ورودی خام یا همان `word_norm` نتیجه) — HTML پاک‌سازی‌شده.
    /// اگر املای دقیق نبود، با fold تلاش می‌کند («اب» → «آب»).
    ///
    /// `focus_id`: اگر مدخلی از همین گروه باشد، صفحه‌ای که آن را دارد برگردانده می‌شود (برای نتایج متن‌کامل)؛
    /// وگرنه صفحهٔ شروع‌شده از `offset`.
    pub fn get_entry_group(&self, key: &str, focus_id: Option<i64>, offset: usize) -> Result<Option<EntryGroup>> {
        self.get_entry_group_paged(key, focus_id, offset, GROUP_PAGE_SIZE)
    }

    pub fn get_entry_group_paged(&self, key: &str, focus_id: Option<i64>, offset: usize, page_size: usize) -> Result<Option<EntryGroup>> {
        let page_size = page_size.max(1);
        let n = normalize(truncate_chars(key, MAX_QUERY_CHARS));
        if n.is_empty() {
            return Ok(None);
        }
        self.with_conn(|conn| {
            let head = |norm: &str| -> Option<(String, String, i64)> {
                conn.prepare_cached("SELECT word_norm, word, cnt FROM heads WHERE word_norm = ?1")
                    .ok()?
                    .query_row(params![norm], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
                    .ok()
            };
            // املای دقیق، وگرنه همان سرواژه با fold (کوچک‌ترین id)
            let found = head(&n).or_else(|| {
                let alt: String = conn
                    .prepare_cached("SELECT word_norm FROM heads WHERE word_fold = ?1 ORDER BY id LIMIT 1")
                    .ok()?
                    .query_row(params![fold(&n)], |r| r.get(0))
                    .ok()?;
                head(&alt)
            });
            let Some((word_norm, word, total)) = found else { return Ok(None) };
            let total = total.max(0) as usize;

            let mut start = offset;
            if let Some(fid) = focus_id {
                // موقعیت مدخل در گروه = تعداد مدخل‌های هم‌املای قبل از آن (فقط اگر واقعاً عضو گروه باشد)
                let pos: Option<i64> = conn
                    .prepare_cached(
                        "SELECT (SELECT COUNT(*) FROM words WHERE word_norm = ?1 AND id < ?2) \
                         WHERE EXISTS (SELECT 1 FROM words WHERE id = ?2 AND word_norm = ?1)",
                    )?
                    .query_row(params![word_norm, fid], |r| r.get(0))
                    .ok();
                if let Some(p) = pos {
                    start = p as usize;
                }
            }
            let start = start.min(total.saturating_sub(1)) / page_size * page_size;

            let mut st = conn.prepare_cached(
                "SELECT id, word, meaning_html FROM words WHERE word_norm = ?1 ORDER BY id LIMIT ?2 OFFSET ?3",
            )?;
            let rows = st.query_map(params![word_norm, page_size as i64, start as i64], |r| {
                Ok(Entry { id: r.get(0)?, word: r.get(1)?, html: sanitize_meaning_html(&r.get::<_, String>(2)?) })
            })?;
            let entries = rows.collect::<rusqlite::Result<Vec<_>>>()?;
            Ok(Some(EntryGroup { word_norm, word, total: total as u32, offset: start as u32, page_size: page_size as u32, entries }))
        })
    }
}
