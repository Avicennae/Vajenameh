//! پاک‌سازی HTML معنی‌ها (`meaning_html`) پیش از نمایش.
//!
//! فهرست سفید: تگ‌های `span br font b i a`؛ از صفات فقط
//!   * `span[class=hlight]` (کلاس‌های دیگر حذف می‌شوند)
//!   * `span[dir=rtl|ltr|auto]`
//!   * `font[color]` فقط با چهار رنگی که در دیتا هست (`red blue darkgreen hotpink`؛ به حروف کوچک)
//!     تا رابط بتواند آن‌ها را با `font[color="red"] { color: var(--c-red) }` برای حالت تاریک تنظیم کند.
//! `a` بدون هیچ صفتی (href/onmouseover/… حذف) فقط متن را نگه می‌دارد. `script`/`style` همراه محتوایشان
//! حذف می‌شوند و تگ‌های غیرمجاز (مثل `div`، `img`) فقط خودشان برداشته می‌شوند.
//! تگ‌های بد‌تو‌در‌تو در دیتا (`<font><b>..</font></b>`) با پارسر HTML5 درست می‌شوند.

use std::borrow::Cow;
use std::collections::{HashMap, HashSet};
use std::sync::OnceLock;

use ammonia::Builder;

/// رنگ‌های مجاز (حروف کوچک) — دقیقاً همان‌هایی که در دیتای دهخدا پیدا شد.
pub const ALLOWED_COLORS: [&str; 4] = ["red", "blue", "darkgreen", "hotpink"];

fn canonical_color(value: &str) -> Option<&'static str> {
    let v = value.trim();
    ALLOWED_COLORS.iter().copied().find(|c| c.eq_ignore_ascii_case(v))
}

fn builder() -> &'static Builder<'static> {
    static BUILDER: OnceLock<Builder<'static>> = OnceLock::new();
    BUILDER.get_or_init(|| {
        let mut b = Builder::empty();
        b.tags(HashSet::from(["span", "br", "font", "b", "i", "a"]));
        b.tag_attributes(HashMap::from([("span", HashSet::from(["dir"])), ("font", HashSet::from(["color"]))]));
        b.allowed_classes(HashMap::from([("span", HashSet::from(["hlight"]))]));
        b.attribute_filter(|element, attribute, value| match (element, attribute) {
            // کلاس‌ها را خودمان فیلتر می‌کنیم (فقط hlight)؛ اگر نبود، کل صفت حذف می‌شود
            ("span", "class") => value.split_ascii_whitespace().any(|c| c == "hlight").then_some(Cow::Borrowed("hlight")),
            ("span", "dir") => {
                let v = value.trim().to_ascii_lowercase();
                matches!(v.as_str(), "rtl" | "ltr" | "auto").then(|| Cow::Owned(v))
            }
            ("font", "color") => canonical_color(value).map(Cow::Borrowed),
            _ => None,
        });
        b.link_rel(None);
        b.strip_comments(true);
        b.clean_content_tags(HashSet::from(["script", "style"]));
        b
    })
}

/// `meaning_html` را پاک‌سازی می‌کند.
pub fn sanitize_meaning_html(html: &str) -> String {
    builder().clean(html).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(h: &str) -> String {
        sanitize_meaning_html(h)
    }

    #[test]
    fn keeps_the_markup_used_by_the_data() {
        let h = "<span class=\"hlight\">آب .</span> (اِ) x<BR><font color=\"Blue\">◄</font> <b>y</b><i>z</i><br />";
        assert_eq!(s(h), "<span class=\"hlight\">آب .</span> (اِ) x<br><font color=\"blue\">◄</font> <b>y</b><i>z</i><br>");
    }

    #[test]
    fn all_four_data_colors_are_kept_and_lowercased() {
        for c in ["Red", "Blue", "DarkGreen", "HotPink", "RED", "darkgreen"] {
            let out = s(&format!("<font color=\"{c}\">x</font>"));
            assert_eq!(out, format!("<font color=\"{}\">x</font>", c.to_ascii_lowercase()));
        }
    }

    #[test]
    fn unknown_colors_and_other_font_attributes_are_dropped() {
        assert_eq!(s("<font color=\"#00ff00\" size=\"7\" face=\"x\">a</font>"), "<font>a</font>");
        assert_eq!(s("<font color=\"red;background:url(x)\">a</font>"), "<font>a</font>");
    }

    #[test]
    fn span_dir_values_are_checked() {
        assert_eq!(s("<span dir=\"RTL\">a</span>"), "<span dir=\"rtl\">a</span>");
        assert_eq!(s("<span dir=\"evil\">a</span>"), "<span>a</span>");
    }

    #[test]
    fn only_the_hlight_class_survives() {
        assert_eq!(s("<span class=\"hlight evil\">a</span>"), "<span class=\"hlight\">a</span>");
        assert_eq!(s("<span class=\"evil\">a</span>"), "<span>a</span>");
        assert_eq!(s("<span class=\"\\\"\">a</span>"), "<span>a</span>"); // رکوردهای خراب دیتا
    }

    #[test]
    fn scripts_handlers_styles_and_images_are_removed() {
        let h = "<span style=\"color:red\" onclick=\"evil()\">a</span><script>alert(1)</script><style>p{}</style><img src=x onerror=alert(2)>b";
        assert_eq!(s(h), "<span>a</span>b");
    }

    #[test]
    fn anchors_lose_every_attribute() {
        let h = "<a id=\"fn1\" href=\"javascript:alert(1)\" onmouseover=\"javascript:ShowTip('x')\">پانویس</a><a href=\"https://evil.example\">y</a>";
        assert_eq!(s(h), "<a>پانویس</a><a>y</a>");
        assert!(!s(h).contains("javascript") && !s(h).contains("http"));
    }

    #[test]
    fn disallowed_wrappers_keep_their_text() {
        assert_eq!(s("<div>a<p>b</p><table><tr><td>c</td></tr></table></div>"), "abc");
    }

    #[test]
    fn stray_closing_tags_from_the_dump_are_harmless() {
        assert_eq!(s("متن.<br /></font></div>"), "متن.<br>");
    }

    #[test]
    fn misnested_font_and_b_are_repaired() {
        assert_eq!(
            s("<font color=\"DarkGreen\"><b>[ ژَ ]</font></b> (اِ مصغر)"),
            "<font color=\"darkgreen\"><b>[ ژَ ]</b></font> (اِ مصغر)"
        );
    }

    #[test]
    fn comments_and_entities() {
        assert_eq!(s("a<!-- x -->b &lt;i&gt; &amp; &nbsp;"), "ab &lt;i&gt; &amp; &nbsp;");
    }

    #[test]
    fn output_is_stable_when_sanitized_twice() {
        let h = "<span class=\"hlight\" dir=\"rtl\" onclick=\"x\">٨٠</span><font color=\"Red\">q<b>w</font></b><a href=\"#\">z</a><script>1</script>";
        assert_eq!(s(&s(h)), s(h));
    }

    #[test]
    fn rtl_text_and_diacritics_pass_through_untouched() {
        let t = "غُرّه ٔ آن، مَزه «آب» ؛";
        assert_eq!(s(t), t);
    }
}
