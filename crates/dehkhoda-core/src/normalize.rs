//! نرمال‌سازی متن فارسی — آینهٔ دقیق `tools/fa_text.py`.
//!
//! هر دو پیاده‌سازی با `shared/normalization_vectors.json` (بردارهای دست‌نویس) و
//! `shared/normalization_golden.json` (هزاران رشتهٔ تصادفی با خروجی Python) تست می‌شوند.
//! اگر قاعده‌ای را عوض کردید، `NORM_VERSION` را در هر دو طرف بالا ببرید.
//!
//! * [`normalize`] — سطح ۱ (`word_norm`): یکسان‌سازی املا، حذف اعراب/کشیده/همزهٔ ترکیبی،
//!   ارقام ASCII، نیم‌فاصله و هر فاصله ⇒ یک فاصله. «آ» و «أ» و «إ» جدا می‌مانند.
//! * [`fold`] — سطح ۲ (`word_fold`): روی خروجی `normalize`، آ/أ/إ/ٱ → ا (هم‌طول).

/// نسخهٔ قواعد نرمال‌سازی؛ باید با `meta.norm_version` پایگاه داده برابر باشد.
pub const NORM_VERSION: u32 = 1;

/// نقش هر کاراکتر در نرمال‌سازی (یک جدول واحد؛ هم `normalize` و هم کمک‌تابع‌ها از آن استفاده می‌کنند).
enum Class {
    /// U+0653 (مدّ ترکیبی): بعد از «ا» ⇒ «آ»، وگرنه حذف
    Madda,
    /// جایگزینی با کاراکتر دیگر
    Map(char),
    /// حذف: علامت‌های ترکیبی عربی، الف خنجری، کشیده، کنترل‌های نامرئی
    Delete,
    /// فاصله (شامل ZWNJ)
    Space,
    Keep,
}

#[inline]
fn classify(c: char) -> Class {
    let cp = c as u32;
    match cp {
        0x0653 => Class::Madda, // باید قبل از بازهٔ حذف بیاید
        0x064A | 0x0649 => Class::Map('\u{06CC}'), // ي ى → ی
        0x0643 => Class::Map('\u{06A9}'),          // ك → ک
        0x0629 | 0x06C0 => Class::Map('\u{0647}'), // ة ۀ → ه
        0x0660..=0x0669 => Class::Map((b'0' + (cp - 0x0660) as u8) as char),
        0x06F0..=0x06F9 => Class::Map((b'0' + (cp - 0x06F0) as u8) as char),
        0x41..=0x5A => Class::Map((cp as u8 + 32) as char), // فقط A–Z
        0x064B..=0x065F
        | 0x0670
        | 0x0640
        | 0x00AD
        | 0x061C
        | 0x200D
        | 0x200E
        | 0x200F
        | 0xFEFF
        | 0x202A..=0x202E
        | 0x2066..=0x2069 => Class::Delete,
        0x09..=0x0D
        | 0x20
        | 0x85
        | 0xA0
        | 0x1680
        | 0x2000..=0x200C
        | 0x2028
        | 0x2029
        | 0x202F
        | 0x205F
        | 0x3000 => Class::Space,
        _ => Class::Keep,
    }
}

/// آیا `normalize` این کاراکتر را فاصله حساب می‌کند؟ (فاصله، تب، NBSP، ZWNJ، ...)
pub fn is_space(c: char) -> bool {
    matches!(classify(c), Class::Space)
}

/// آیا `normalize` این کاراکتر را بی‌صدا حذف می‌کند؟ (اعراب، کشیده، کنترل‌های نامرئی، ...)
pub fn is_ignored(c: char) -> bool {
    matches!(classify(c), Class::Delete | Class::Madda)
}

/// سطح ۱: «word_norm».
pub fn normalize(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match classify(c) {
            Class::Madda => {
                if out.ends_with('\u{0627}') {
                    out.pop();
                    out.push('\u{0622}');
                }
            }
            Class::Map(m) => out.push(m),
            Class::Delete => {}
            // فاصله‌ها (و ZWNJ) ⇒ یک فاصلهٔ واحد، بدون فاصلهٔ ابتدایی
            Class::Space => {
                if !out.is_empty() && !out.ends_with(' ') {
                    out.push(' ');
                }
            }
            Class::Keep => out.push(c),
        }
    }
    if out.ends_with(' ') {
        out.pop();
    }
    out
}

/// نگاشت تک‌کاراکتریِ fold (آ/أ/إ/ٱ → ا).
#[inline]
pub fn fold_char(c: char) -> char {
    match c {
        '\u{0622}' | '\u{0623}' | '\u{0625}' | '\u{0671}' => '\u{0627}',
        _ => c,
    }
}

/// سطح ۲: «word_fold» — روی متنِ از قبل normalize‌شده.
pub fn fold(text: &str) -> String {
    text.chars().map(fold_char).collect()
}

/// `(normalize(s), fold(normalize(s)))`
pub fn normalize_fold(text: &str) -> (String, String) {
    let n = normalize(text);
    let f = fold(&n);
    (n, f)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basics() {
        assert_eq!(normalize("\u{0643}\u{062A}\u{0627}\u{0628}"), "\u{06A9}\u{062A}\u{0627}\u{0628}");
        assert_eq!(normalize("  a\u{200C} b  "), "a b");
        assert_eq!(fold("\u{0622}\u{0628}"), "\u{0627}\u{0628}");
    }

    #[test]
    fn fold_keeps_char_count() {
        let s = normalize("\u{0622}\u{0623}\u{0625}\u{0671}x");
        assert_eq!(s.chars().count(), fold(&s).chars().count());
    }
}
