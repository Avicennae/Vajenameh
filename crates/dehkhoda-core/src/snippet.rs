//! ساخت «خلاصهٔ نتیجه» برای جست‌وجوی متن‌کامل.
//!
//! فهرست FTS روی متنِ fold‌شده ساخته شده (آ/أ/إ → ا)، ولی خلاصه از `meaning_text` اصلی ساخته می‌شود
//! (تا «آ» در خلاصه دیده شود). چون fold کاراکتر‌به‌کاراکتر و هم‌طول است، جای تطابق‌ها در دو متن یکی است.

use serde::Serialize;

use crate::normalize::fold_char;

/// یک واژهٔ جست‌وجو برای هایلایت: متن fold‌شده و اینکه پیشوندی است (واژهٔ آخرِ در حال تایپ).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QueryWord {
    pub text: String,
    pub prefix: bool,
}

/// یک تکه از خلاصه؛ `hit = true` یعنی باید هایلایت شود.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct Segment {
    pub text: String,
    pub hit: bool,
}

fn is_word_char(c: char) -> bool {
    c.is_alphanumeric()
}

fn push_segment(out: &mut Vec<Segment>, text: String, hit: bool) {
    if text.is_empty() {
        return;
    }
    if let Some(last) = out.last_mut() {
        if last.hit == hit {
            last.text.push_str(&text);
            return;
        }
    }
    out.push(Segment { text, hit });
}

/// جای همهٔ تطابق‌ها (نیم‌باز، به اندیس کاراکتر) را برمی‌گرداند؛ مرتب و ادغام‌شده.
fn find_spans(folded: &[char], words: &[QueryWord]) -> Vec<(usize, usize)> {
    let n = folded.len();
    let mut spans: Vec<(usize, usize)> = Vec::new();
    for w in words {
        let tok: Vec<char> = w.text.chars().collect();
        if tok.is_empty() || tok.len() > n {
            continue;
        }
        let mut i = 0;
        while i + tok.len() <= n {
            let starts_word = i == 0 || !is_word_char(folded[i - 1]);
            if starts_word && folded[i..i + tok.len()] == tok[..] {
                let end = i + tok.len();
                if w.prefix || end == n || !is_word_char(folded[end]) {
                    spans.push((i, end));
                    i = end;
                    continue;
                }
            }
            i += 1;
        }
    }
    spans.sort_unstable();
    let mut merged: Vec<(usize, usize)> = Vec::with_capacity(spans.len());
    for (s, e) in spans {
        match merged.last_mut() {
            Some(last) if s <= last.1 => last.1 = last.1.max(e),
            _ => merged.push((s, e)),
        }
    }
    merged
}

/// خلاصه‌ای حدوداً `max_chars` کاراکتری حول اولین تطابق می‌سازد. اگر تطابقی پیدا نشد، ابتدای متن.
pub fn make_snippet(text: &str, words: &[QueryWord], max_chars: usize) -> Vec<Segment> {
    let chars: Vec<char> = text.chars().collect();
    let n = chars.len();
    let folded: Vec<char> = chars.iter().map(|&c| fold_char(c)).collect();
    let spans = find_spans(&folded, words);

    let mut out: Vec<Segment> = Vec::new();
    let (first_start, first_end) = match spans.first() {
        Some(&s) => s,
        None => {
            let end = n.min(max_chars);
            push_segment(&mut out, chars[..end].iter().collect(), false);
            if end < n {
                push_segment(&mut out, "…".to_string(), false);
            }
            return out;
        }
    };

    // آغاز پنجره: کمی قبل از اولین تطابق، روی مرز کلمه
    let lead = max_chars / 3;
    let mut start = first_start.saturating_sub(lead);
    if start > 0 {
        if let Some(p) = (start..first_start).find(|&k| chars[k] == ' ') {
            start = p + 1;
        }
    }
    // پایان پنجره: روی مرز کلمه، ولی همیشه شامل اولین تطابق
    let mut end = (start + max_chars).min(n);
    if end < n {
        if let Some(p) = (first_end.min(end)..end).rev().find(|&k| chars[k] == ' ') {
            end = p;
        }
    }
    end = end.max(first_end.min(n));

    if start > 0 {
        push_segment(&mut out, "…".to_string(), false);
    }
    let mut cursor = start;
    for &(s, e) in spans.iter().filter(|&&(s, e)| e > start && s < end) {
        let (s, e) = (s.max(start), e.min(end));
        if s > cursor {
            push_segment(&mut out, chars[cursor..s].iter().collect(), false);
        }
        push_segment(&mut out, chars[s..e].iter().collect(), true);
        cursor = e;
    }
    if cursor < end {
        push_segment(&mut out, chars[cursor..end].iter().collect(), false);
    }
    if end < n {
        push_segment(&mut out, "…".to_string(), false);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn w(t: &str, prefix: bool) -> QueryWord {
        QueryWord { text: t.to_string(), prefix }
    }
    fn joined(s: &[Segment]) -> String {
        s.iter().map(|x| x.text.as_str()).collect()
    }
    fn hits(s: &[Segment]) -> Vec<&str> {
        s.iter().filter(|x| x.hit).map(|x| x.text.as_str()).collect()
    }

    #[test]
    fn highlights_whole_words_only() {
        let s = make_snippet("خط و نسخ و خطاط", &[w("خط", false)], 100);
        assert_eq!(joined(&s), "خط و نسخ و خطاط");
        assert_eq!(hits(&s), vec!["خط"]); // داخل «خطاط» هایلایت نمی‌شود
    }

    #[test]
    fn prefix_word_highlights_only_the_typed_part_and_keeps_madda() {
        // متن اصلی «آسیاب» دارد، ولی ورودی (fold‌شده) «اسیا» است
        let s = make_snippet("جای آسیاب و آسیا", &[w("اسیا", true)], 100);
        assert_eq!(hits(&s), vec!["آسیا", "آسیا"]);
        assert_eq!(joined(&s), "جای آسیاب و آسیا");
    }

    #[test]
    fn non_prefix_requires_word_end() {
        let s = make_snippet("آسیاب", &[w("اسیا", false)], 100);
        assert!(hits(&s).is_empty());
    }

    #[test]
    fn window_and_ellipses_on_long_text() {
        let long = format!("{} هدف {}", "الف ".repeat(100), "ب ".repeat(100));
        let s = make_snippet(&long, &[w("هدف", false)], 60);
        let text = joined(&s);
        assert!(text.starts_with('…') && text.ends_with('…'), "{text}");
        assert_eq!(hits(&s), vec!["هدف"]);
        assert!(text.chars().count() <= 70, "{}", text.chars().count());
    }

    #[test]
    fn no_match_returns_head() {
        let s = make_snippet("یک متن کوتاه", &[w("زرافه", false)], 100);
        assert_eq!(s, vec![Segment { text: "یک متن کوتاه".into(), hit: false }]);
        let long = "الف ".repeat(100);
        let s = make_snippet(&long, &[w("زرافه", false)], 20);
        assert!(joined(&s).ends_with('…'));
    }

    #[test]
    fn multiple_words_and_merging() {
        let s = make_snippet("خط نسخ", &[w("خط", false), w("نسخ", false)], 100);
        assert_eq!(hits(&s), vec!["خط", "نسخ"]);
        let s = make_snippet("ab", &[w("a", true), w("ab", false)], 100);
        assert_eq!(s, vec![Segment { text: "ab".into(), hit: true }]);
    }

    #[test]
    fn empty_inputs() {
        assert!(make_snippet("", &[w("الف", false)], 50).is_empty());
        assert_eq!(joined(&make_snippet("abc", &[], 50)), "abc");
    }
}
