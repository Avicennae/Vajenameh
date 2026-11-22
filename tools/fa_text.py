"""
نرمال‌سازی متن فارسی و تبدیل HTML به متن خالص.

این ماژول «مرجع» رفتار نرمال‌سازی است. نسخهٔ Rust (crates/dehkhoda-core/src/normalize.rs)
باید دقیقاً همین رفتار را داشته باشد؛ هر دو با فایل مشترک shared/normalization_vectors.json
تست می‌شوند. اگر قاعده‌ای را تغییر دادید، NORM_VERSION را بالا ببرید تا برنامه از ناسازگاری
بین دیتابیس و کد خبردار شود.

دو سطح داریم:
  normalize(s)  → سطح ۱ («word_norm»): املاهای عربی/فارسی یکسان، اعراب و کشیده حذف،
                   ارقام ASCII، ZWNJ و هر نوع فاصله ⇒ یک فاصله؛ آ و أ و إ همچنان جدا می‌مانند
                   تا «املای دقیق» قابل تشخیص باشد.
  fold(s)       → سطح ۲ («word_fold»): روی خروجی normalize، آ/أ/إ/ٱ → ا. فقط برای «پیدا کردن»؛
                   رتبه‌بندی با سطح ۱ انجام می‌شود (املای دقیق بالاتر).
"""
from __future__ import annotations

import html as _html
import re

NORM_VERSION = 1

# --- نگاشت تک‌کاراکتری -----------------------------------------------------
_MAP = {
    0x064A: "\u06CC",  # ي → ی
    0x0649: "\u06CC",  # ى (الف مقصوره) → ی        [افزودهٔ ما]
    0x0643: "\u06A9",  # ك → ک
    0x0629: "\u0647",  # ة → ه
    0x06C0: "\u0647",  # ۀ (ه + همزه، یک‌تکه) → ه  [افزودهٔ ما؛ هم‌ارز «ه» + U+0654]
}
for _i in range(10):
    _MAP[0x0660 + _i] = chr(0x30 + _i)  # ارقام عربی-هندی
    _MAP[0x06F0 + _i] = chr(0x30 + _i)  # ارقام فارسی
for _c in range(ord("A"), ord("Z") + 1):
    _MAP[_c] = chr(_c + 32)  # فقط A–Z ← عمداً ASCII؛ تا Python و Rust یکسان بمانند

# --- حذف ------------------------------------------------------------------
# همهٔ علامت‌های ترکیبی عربی U+064B..U+065F (اعراب، تشدید، سکون، همزهٔ روی حرف U+0654، ...)
# به‌جز U+0653 که ابتدا برای ساخت «آ» از «ا + ٓ» بررسی می‌شود.
_DELETE = set(range(0x064B, 0x0660)) - {0x0653}
_DELETE |= {0x0670, 0x0640}                      # الف خنجری، کشیده
_DELETE |= {0x00AD, 0x061C, 0x200D, 0x200E, 0x200F, 0xFEFF}
_DELETE |= set(range(0x202A, 0x202F)) | set(range(0x2066, 0x206A))  # کنترل‌های bidi

# --- فاصله‌ها: همه ⇒ ' ' -------------------------------------------------------
_SPACES = {0x09, 0x0A, 0x0B, 0x0C, 0x0D, 0x20, 0x85, 0xA0, 0x1680, 0x2028, 0x2029, 0x202F, 0x205F, 0x3000}
_SPACES |= set(range(0x2000, 0x200C))            # فاصله‌های U+2000..U+200B (ZWSP هم)
_SPACES |= {0x200C}                              # نیم‌فاصله (ZWNJ) ≡ فاصله

_TABLE: dict = {c: None for c in _DELETE}
_TABLE.update({c: " " for c in _SPACES})
_TABLE.update(_MAP)
_TABLE[0x0653] = "\u0653"  # بعد از translate بررسی می‌شود
_MULTI_SPACE = re.compile(" {2,}")

_FOLD_TABLE = {0x0622: "\u0627", 0x0623: "\u0627", 0x0625: "\u0627", 0x0671: "\u0627"}


def normalize(text: str) -> str:
    """سطح ۱ — «word_norm» و متن جست‌وجو."""
    s = text.translate(_TABLE)
    if "\u0653" in s:
        s = s.replace("\u0627\u0653", "\u0622").replace("\u0653", "")
    if "  " in s:
        s = _MULTI_SPACE.sub(" ", s)
    return s.strip(" ")


def fold(text: str) -> str:
    """سطح ۲ — روی متنِ از قبل normalize‌شده: آ/أ/إ/ٱ → ا (کاراکتر به کاراکتر، هم‌طول)."""
    return text.translate(_FOLD_TABLE)


def normalize_fold(text: str) -> tuple:
    n = normalize(text)
    return n, fold(n)


# --- HTML → متن ---------------------------------------------------------------
# تگ‌های «بلوکی» به فاصله تبدیل می‌شوند (تا کلمات به هم نچسبند)، بقیه حذف می‌شوند.
_BLOCK_TAG = re.compile(r"<\s*/?\s*(?:br|p|div|li|ul|ol|tr|td|th|table|hr|h[1-6])\b[^<>]*>", re.I)
_ANY_TAG = re.compile(r"<[^<>]*>")


def html_to_text(markup: str) -> str:
    """تگ‌ها را برمی‌دارد و موجودیت‌های HTML (&lt; &amp; &nbsp; ...) را باز می‌کند."""
    s = _BLOCK_TAG.sub(" ", markup)
    s = _ANY_TAG.sub("", s)
    return _html.unescape(s)


def meaning_to_search_text(markup: str) -> str:
    """meaning_html → متن خالص نرمال‌شده (ستون meaning_text)."""
    return normalize(html_to_text(markup))
