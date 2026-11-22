"""
پارسر دامپ‌های MySQL/MariaDB (فرمت phpMyAdmin) برای مخزن Dehkhoda-SQL.

بدون نیاز به MySQL و بدون هیچ وابستگی بیرونی. این فایل یک «توکنایزر» است، نه
regex ی که کل دامپ را یک‌جا ببلعد: دامپ ترم به ترم خوانده می‌شود، رشته‌های
quoted با escape های MySQL (\\' \\" \\\\ \\n ...) و '' درست باز می‌شوند، و هر جا
چیزی غیرمنتظره بود به‌جای رد شدن بی‌صدا، SqlParseError با موقعیت دقیق می‌دهد.
"""
from __future__ import annotations

import gzip
import re
from pathlib import Path
from typing import Iterator, Optional, Sequence, Union

__all__ = ["SqlParseError", "iter_inserts", "iter_word_rows", "read_sql_text", "unescape_mysql"]


class SqlParseError(ValueError):
    """خطای پارس؛ پیام شامل شمارهٔ خط/ستون و بخشی از متن اطراف است."""


# ---------------------------------------------------------------------------
# escape های MySQL
# ---------------------------------------------------------------------------
_SIMPLE_ESCAPES = {
    "0": "\x00",
    "b": "\b",
    "n": "\n",
    "r": "\r",
    "t": "\t",
    "Z": "\x1a",
}
_UNESCAPE_RE = re.compile(r"\\(.)|''", re.S)


def _unescape_match(m: "re.Match[str]") -> str:
    if m.group(0) == "''":
        return "'"
    ch = m.group(1)
    if ch in _SIMPLE_ESCAPES:
        return _SIMPLE_ESCAPES[ch]
    if ch in ("%", "_"):  # MySQL: \% و \_ با بک‌اسلش باقی می‌مانند
        return "\\" + ch
    return ch  # \' \" \\ و هر کاراکتر دیگر: خود کاراکتر


def unescape_mysql(raw: str) -> str:
    """محتوای بین دو quote (بدون خود quote ها) را به رشتهٔ واقعی تبدیل می‌کند."""
    if "\\" not in raw and "''" not in raw:
        return raw
    return _UNESCAPE_RE.sub(_unescape_match, raw)


# ---------------------------------------------------------------------------
# توکن‌ها
# ---------------------------------------------------------------------------
_WS_RE = re.compile(r"\s*")
_INSERT_RE = re.compile(
    r"INSERT\s+(?:LOW_PRIORITY\s+|DELAYED\s+|HIGH_PRIORITY\s+)?(?:IGNORE\s+)?INTO\s+"
    r"(?:`(?P<tbl>[^`]+)`|(?P<tbl2>[A-Za-z_][\w$]*))\s*"
    r"(?:\((?P<cols>[^)]*)\)\s*)?VALUES\b",
    re.I,
)
# یک مقدار: NULL | عدد | 'رشته'  (به‌همراه فاصله‌های دور و جداکنندهٔ بعدی , یا ) )
_VALUE_RE = re.compile(
    r"\s*(?:(?P<null>NULL)|(?P<num>[-+]?\d+(?:\.\d+)?(?:[eE][-+]?\d+)?)"
    r"|'(?P<str>[^'\\]*(?:(?:\\.|'')[^'\\]*)*)')\s*(?P<sep>[,)])",
    re.S | re.I,
)
# برای پرش از روی دستورهای غیر INSERT: اولین چیز مهم بعدی
_SKIP_RE = re.compile(r"""[;'"`#]|--[ \t\r\n]|/\*""")
_QUOTED_RES = {
    "'": re.compile(r"'[^'\\]*(?:(?:\\.|'')[^'\\]*)*'", re.S),
    '"': re.compile(r'"[^"\\]*(?:(?:\\.|"")[^"\\]*)*"', re.S),
    "`": re.compile(r"`[^`]*(?:``[^`]*)*`", re.S),
}


def _where(text: str, pos: int) -> str:
    line = text.count("\n", 0, pos) + 1
    col = pos - (text.rfind("\n", 0, pos) + 1) + 1
    snippet = text[max(0, pos - 40): pos + 60].replace("\n", "⏎")
    return f"خط {line}، ستون {col}: …{snippet}…"


def _skip_comment_or_statement(text: str, pos: int) -> int:
    """از pos (ابتدای یک دستور یا کامنت غیر INSERT) تا پایان آن می‌پرد."""
    n = len(text)
    if text.startswith("--", pos) or text.startswith("#", pos):
        nl = text.find("\n", pos)
        return n if nl < 0 else nl + 1
    if text.startswith("/*", pos):
        end = text.find("*/", pos + 2)
        if end < 0:
            raise SqlParseError("کامنت /* … */ بسته نشده است. " + _where(text, pos))
        end += 2
        # /*!40101 ... */;  ← سمی‌کولن اختیاریِ بعد از آن
        end = _WS_RE.match(text, end).end()
        if end < n and text[end] == ";":
            end += 1
        return end
    # دستور عادی: تا اولین ';' بیرون از quote ها و کامنت‌ها
    while True:
        m = _SKIP_RE.search(text, pos)
        if not m:
            return n
        tok = m.group(0)
        if tok == ";":
            return m.end()
        if tok in _QUOTED_RES:
            q = _QUOTED_RES[tok].match(text, m.start())
            if not q:
                raise SqlParseError("رشتهٔ quoted بسته نشده است. " + _where(text, m.start()))
            pos = q.end()
        elif tok == "/*":
            end = text.find("*/", m.start() + 2)
            if end < 0:
                raise SqlParseError("کامنت /* … */ بسته نشده است. " + _where(text, m.start()))
            pos = end + 2
        else:  # '--' یا '#'
            nl = text.find("\n", m.start())
            pos = n if nl < 0 else nl + 1


def _parse_columns(raw: Optional[str]) -> Optional[tuple]:
    if raw is None:
        return None
    return tuple(c.strip().strip("`").strip() for c in raw.split(",") if c.strip())


def iter_inserts(text: str) -> Iterator[tuple]:
    """
    همهٔ ردیف‌های همهٔ دستورهای INSERT را برمی‌گرداند: (table, columns|None, values)
    values تاپلی از int/float/str/None است.
    """
    pos, n = 0, len(text)
    if text.startswith("\ufeff"):
        pos = 1
    while True:
        pos = _WS_RE.match(text, pos).end()
        if pos >= n:
            return
        m = _INSERT_RE.match(text, pos)
        if not m:
            pos = _skip_comment_or_statement(text, pos)
            continue
        table = m.group("tbl") or m.group("tbl2")
        columns = _parse_columns(m.group("cols"))
        pos = m.end()
        # --- ردیف‌ها ---
        while True:
            pos = _WS_RE.match(text, pos).end()
            if pos >= n or text[pos] != "(":
                raise SqlParseError("انتظار '(' برای شروع ردیف می‌رفت. " + _where(text, min(pos, n)))
            pos += 1
            values = []
            while True:
                vm = _VALUE_RE.match(text, pos)
                if not vm:
                    raise SqlParseError("مقدار نامعتبر یا رشتهٔ بسته‌نشده. " + _where(text, pos))
                if vm.group("null") is not None:
                    values.append(None)
                elif vm.group("num") is not None:
                    s = vm.group("num")
                    values.append(float(s) if any(c in s for c in ".eE") else int(s))
                else:
                    values.append(unescape_mysql(vm.group("str")))
                pos = vm.end()
                if vm.group("sep") == ")":
                    break
            yield table, columns, tuple(values)
            pos = _WS_RE.match(text, pos).end()
            if pos >= n:
                return  # فایل بدون ';' پایانی
            if text[pos] == ",":
                pos += 1
                continue
            if text[pos] == ";":
                pos += 1
                break
            raise SqlParseError("بعد از ردیف انتظار ',' یا ';' می‌رفت. " + _where(text, pos))


def read_sql_text(path: Union[str, Path]) -> str:
    """فایل .sql یا .sql.gz را به‌صورت متن UTF-8 (سخت‌گیرانه) می‌خواند."""
    p = Path(path)
    opener = gzip.open if p.suffix == ".gz" else open
    with opener(p, "rt", encoding="utf-8", errors="strict", newline="") as f:  # type: ignore[call-overload]
        return f.read()


def iter_word_rows(
    source: Union[str, Path, None] = None,
    *,
    text: Optional[str] = None,
    table: str = "words",
    expected_columns: Sequence[str] = ("id", "word", "meaning"),
) -> Iterator[tuple]:
    """
    ردیف‌های جدول words را به‌صورت (id:int, word:str, meaning:str) برمی‌گرداند.
    ترتیب ستون‌ها از سرتیتر INSERT خوانده می‌شود، پس جابه‌جایی ستون‌ها مشکلی ایجاد نمی‌کند.
    """
    if text is None:
        if source is None:
            raise TypeError("source یا text لازم است")
        text = read_sql_text(source)
    order: Optional[list] = None
    for tbl, cols, vals in iter_inserts(text):
        if tbl != table:
            continue
        if cols is None:
            cols = tuple(expected_columns)
        if order is None or len(order) != len(cols):
            missing = [c for c in expected_columns if c not in cols]
            if missing:
                raise SqlParseError(f"ستون‌های {missing} در INSERT پیدا نشد؛ ستون‌ها: {cols}")
            order = [cols.index(c) for c in expected_columns]
        if len(vals) != len(cols):
            raise SqlParseError(f"تعداد مقادیر ({len(vals)}) با تعداد ستون‌ها ({len(cols)}) نمی‌خواند")
        rid, word, meaning = (vals[i] for i in order)
        if not isinstance(rid, int) or not isinstance(word, str) or not isinstance(meaning, str):
            raise SqlParseError(
                f"نوع مقدار غیرمنتظره: id={rid!r}, word={type(word).__name__}, meaning={type(meaning).__name__}"
            )
        yield rid, word, meaning
