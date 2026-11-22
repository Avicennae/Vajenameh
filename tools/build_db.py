#!/usr/bin/env python3
"""
ساخت پایگاه دادهٔ SQLite لغت‌نامهٔ دهخدا از دامپ‌های مخزن Dehkhoda-SQL.

    python tools/build_db.py                      # دانلود (در صورت نیاز) + ساخت + بررسی
    python tools/build_db.py --no-fts             # بدون جست‌وجوی متن‌کامل (فایل کوچک‌تر)
    python tools/build_db.py --update             # مخزن داده را به‌روز می‌کند (git pull)
    python tools/build_db.py --src DIR --out FILE # مسیرهای دلخواه

خروجی پیش‌فرض: src-tauri/resources/dehkhoda.sqlite (همان فایلی که با نصب‌کننده همراه می‌شود).
فقط به Python 3.9+ نیاز دارد (کتابخانه‌های استاندارد)؛ نیازی به MySQL نیست.
"""
from __future__ import annotations

import argparse
import datetime as dt
import io
import os
import re
import shutil
import sqlite3
import subprocess
import sys
import time
import zipfile
from pathlib import Path
from typing import List, Optional

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

from dehkhoda_sql import iter_word_rows  # noqa: E402
from fa_text import NORM_VERSION, fold, meaning_to_search_text, normalize  # noqa: E402

REPO_URL = "https://github.com/nimah79/Dehkhoda-SQL"
ZIP_URL = REPO_URL + "/archive/HEAD.zip"
SCHEMA_VERSION = 2
FALLBACK_EXPECTED_ROWS = 312507
EXPECTED_FILES = 33
PAGE_SIZE = 16384  # ردیف‌های ~۲ کیلوبایتی؛ صفحهٔ بزرگ‌تر یعنی هدررفت کمتر و فایل کوچک‌تر

ROOT = Path(__file__).resolve().parent.parent

SCHEMA = """
CREATE TABLE words (
  id           INTEGER PRIMARY KEY,
  word         TEXT NOT NULL,   -- سرواژهٔ اصلی، همان‌طور که در دامپ است
  word_norm    TEXT NOT NULL,   -- normalize(word): سطح ۱ (آ جدا می‌ماند)
  word_fold    TEXT NOT NULL,   -- fold(word_norm): سطح ۲ (آ/أ/إ/ٱ → ا)
  meaning_html TEXT NOT NULL,   -- معنی با HTML اصلی
  meaning_text TEXT NOT NULL    -- متن خالص نرمال‌شده (بدون تگ)
);
CREATE TABLE meta (key TEXT PRIMARY KEY, value TEXT NOT NULL) WITHOUT ROWID;
"""
# جدول FTS5 «بدون محتوا» (contentless): فقط نمایه ذخیره می‌شود، نه یک کپی دیگر از متن.
# متن ایندکس‌شده fold(meaning_text) است تا «اب» هم «آب» را پیدا کند؛ rowid همان words.id است.
FTS_SCHEMA = """
CREATE VIRTUAL TABLE words_fts USING fts5(t, content='', tokenize='unicode61 remove_diacritics 0');
"""
# «سرواژه‌های متمایز»: یک ردیف به‌ازای هر word_norm (مدخل‌های هم‌املا یکی می‌شوند).
# جست‌وجوی سرواژه (کامل/پیشوندی/شامل) فقط روی این جدول کوچک (~۱۵ مگابایت) انجام می‌شود، نه روی
# جدول بزرگ words؛ ایندکس trigram هم «شامل‌شدن» را بدون پیمایش کامل ممکن می‌کند.
HEADS_SCHEMA = """
CREATE TABLE heads (
  id        INTEGER PRIMARY KEY,  -- کوچک‌ترین words.id میان مدخل‌های هم‌املا
  word      TEXT NOT NULL,        -- نوشتار اصلی همان مدخل
  word_norm TEXT NOT NULL,
  word_fold TEXT NOT NULL,
  cnt       INTEGER NOT NULL      -- تعداد مدخل‌های هم‌املا
);
"""
HEADS_FILL = """
INSERT INTO heads (id, word, word_norm, word_fold, cnt)
  SELECT MIN(id), word, word_norm, word_fold, COUNT(*) FROM words GROUP BY word_norm;
CREATE UNIQUE INDEX idx_heads_norm ON heads(word_norm);
CREATE INDEX idx_heads_fold ON heads(word_fold, word_norm, word, cnt);
CREATE VIRTUAL TABLE heads_trgm USING fts5(w, content='', tokenize='trigram');
INSERT INTO heads_trgm(rowid, w) SELECT id, word_fold FROM heads;
INSERT INTO heads_trgm(heads_trgm) VALUES('optimize');
"""
INDEX_SCHEMA = """
CREATE INDEX idx_words_norm ON words(word_norm);
"""


def log(msg: str = "") -> None:
    print(msg, flush=True)


def human(n: float) -> str:
    return f"{n / 1048576:,.1f} MB"


# ---------------------------------------------------------------------------
# دریافت داده
# ---------------------------------------------------------------------------
def list_sources(src: Path) -> List[Path]:
    if not src.is_dir():
        return []
    return sorted(list(src.glob("*.sql.gz")) + list(src.glob("*.sql")))


def ensure_source(src: Path, update: bool = False) -> None:
    """اگر پوشهٔ داده نبود (یا --update)، مخزن را با git یا (بدون git) با دانلود ZIP می‌گیرد."""
    if list_sources(src) and not update:
        return
    if shutil.which("git"):
        if (src / ".git").exists():
            log(f"[data] git pull in {src}")
            subprocess.run(["git", "-C", str(src), "pull", "--ff-only"], check=True)
        else:
            if src.exists() and any(src.iterdir()):
                raise SystemExit(f"Folder {src} exists and is not empty/not a git repo; remove it or pass --src.")
            src.parent.mkdir(parents=True, exist_ok=True)
            log(f"[data] git clone {REPO_URL} -> {src}")
            subprocess.run(["git", "clone", "--depth", "1", REPO_URL, str(src)], check=True)
        return
    import urllib.request

    log(f"[data] git not found; downloading {ZIP_URL}")
    with urllib.request.urlopen(ZIP_URL, timeout=300) as resp:
        data = resp.read()
    src.mkdir(parents=True, exist_ok=True)
    with zipfile.ZipFile(io.BytesIO(data)) as zf:
        for info in zf.infolist():
            name = os.path.basename(info.filename)
            if name.endswith(".sql.gz") or name.lower() == "readme.md":
                (src / name).write_bytes(zf.read(info))


def source_commit(src: Path) -> str:
    if shutil.which("git") and (src / ".git").exists():
        try:
            return subprocess.run(
                ["git", "-C", str(src), "rev-parse", "HEAD"], capture_output=True, text=True, check=True
            ).stdout.strip()
        except Exception:
            pass
    return "unknown"


def expected_rows_from_readme(src: Path) -> Optional[int]:
    for name in ("README.md", "readme.md", "README"):
        p = src / name
        if p.is_file():
            m = re.search(r"contains\s+([\d,.\u066c\u066b]+)\s+words", p.read_text(encoding="utf-8", errors="replace"), re.I)
            if m:
                return int(re.sub(r"\D", "", m.group(1)))
    return None


# ---------------------------------------------------------------------------
# ساخت
# ---------------------------------------------------------------------------
class BuildResult:
    def __init__(self) -> None:
        self.rows = 0
        self.per_file: List[tuple] = []
        self.distinct_words = 0
        self.expected: Optional[int] = None
        self.count_matches = True
        self.size_before_vacuum = 0
        self.size_final = 0
        self.warnings: List[str] = []
        self.out: Optional[Path] = None


def build(
    src: Path,
    out: Path,
    *,
    with_fts: bool = True,
    expected: Optional[int] = None,
    limit_files: Optional[int] = None,
    source_url: str = REPO_URL,
) -> BuildResult:
    files = list_sources(src)
    if limit_files:
        files = files[:limit_files]
    if not files:
        raise SystemExit(f"No *.sql.gz files found in {src}")
    res = BuildResult()
    res.out = out
    if expected is None:
        expected = expected_rows_from_readme(src) or FALLBACK_EXPECTED_ROWS
    res.expected = expected
    if not limit_files and len(files) != EXPECTED_FILES:
        res.warnings.append(f"expected {EXPECTED_FILES} dump files, found {len(files)}")

    out.parent.mkdir(parents=True, exist_ok=True)
    tmp = out.with_name(out.name + ".tmp")
    final_tmp = out.with_name(out.name + ".new")
    for p in (tmp, final_tmp):
        if p.exists():
            p.unlink()

    # اتصال در هر حالتی (موفق، SystemExit، هر exception دیگر) بسته می‌شود؛
    # در ویندوز فایلِ باز را نمی‌شود پاک کرد.
    conn = sqlite3.connect(tmp)
    try:
        try:
            conn.execute("CREATE VIRTUAL TABLE temp._probe USING fts5(x, tokenize='trigram')")
            conn.execute("DROP TABLE temp._probe")
        except sqlite3.OperationalError as e:
            raise SystemExit(f"This Python's SQLite ({sqlite3.sqlite_version}) lacks FTS5 trigram support (needs >= 3.34): {e}")
        conn.execute(f"PRAGMA page_size={PAGE_SIZE}")
        conn.execute("PRAGMA journal_mode=OFF")
        conn.execute("PRAGMA synchronous=OFF")
        conn.execute("PRAGMA cache_size=-200000")
        conn.executescript(SCHEMA)
        conn.executescript(HEADS_SCHEMA)
        if with_fts:
            conn.executescript(FTS_SCHEMA)

        t_start = time.time()
        seen_ids = set()
        empty_norm = 0
        commit = source_commit(src)
        for f in files:
            t0 = time.time()
            word_rows, fts_rows = [], []
            for rid, word, meaning in iter_word_rows(f):
                if rid in seen_ids:
                    raise SystemExit(f"Duplicate id {rid} in {f.name}")
                seen_ids.add(rid)
                wn = normalize(word)
                if not wn:
                    empty_norm += 1
                mt = meaning_to_search_text(meaning)
                word_rows.append((rid, word, wn, fold(wn), meaning, mt))
                if with_fts:
                    fts_rows.append((rid, fold(mt)))
            conn.executemany("INSERT INTO words VALUES (?,?,?,?,?,?)", word_rows)
            if with_fts:
                conn.executemany("INSERT INTO words_fts(rowid, t) VALUES (?,?)", fts_rows)
            conn.commit()
            res.per_file.append((f.name, len(word_rows)))
            res.rows += len(word_rows)
            log(f"[{len(res.per_file):>2}/{len(files)}] {f.name:<18} {len(word_rows):>7,} rows  ({time.time() - t0:5.1f}s)")
        if empty_norm:
            res.warnings.append(f"{empty_norm} headwords normalize to an empty string")

        log("[index] creating indexes + distinct-headword table (heads) + trigram index ...")
        conn.executescript(INDEX_SCHEMA)
        conn.executescript(HEADS_FILL)
        if with_fts:
            log("[fts]   optimizing FTS5 index (merge segments) ...")
            conn.execute("INSERT INTO words_fts(words_fts) VALUES('optimize')")
        res.distinct_words = conn.execute("SELECT COUNT(*) FROM heads").fetchone()[0]
        res.count_matches = res.rows == expected

        meta = {
            "schema_version": str(SCHEMA_VERSION),
            "norm_version": str(NORM_VERSION),
            "has_fts": "1" if with_fts else "0",
            "row_count": str(res.rows),
            "distinct_words": str(res.distinct_words),
            "readme_row_count": str(expected),
            "source_url": source_url,
            "source_commit": commit,
            "built_at": dt.datetime.now(dt.timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ"),
            "page_size": str(PAGE_SIZE),
        }
        conn.executemany("INSERT INTO meta VALUES (?,?)", sorted(meta.items()))
        conn.execute("PRAGMA user_version=%d" % SCHEMA_VERSION)
        conn.execute("ANALYZE")
        conn.commit()
    finally:
        conn.close()
    res.size_before_vacuum = tmp.stat().st_size

    log(f"[vacuum] VACUUM INTO {final_tmp.name} (from {human(res.size_before_vacuum)}) ...")
    conn = sqlite3.connect(tmp)
    try:
        conn.execute("VACUUM INTO '%s'" % str(final_tmp).replace("'", "''"))
    finally:
        conn.close()
    tmp.unlink()
    os.replace(final_tmp, out)
    res.size_final = out.stat().st_size
    log(f"[done]   {out}  {human(res.size_final)}   total {time.time() - t_start:,.0f}s")
    return res


def print_report(res: BuildResult) -> None:
    log("")
    log("=== Report ===")
    log(f"files               : {len(res.per_file)}")
    log(f"rows                : {res.rows:,}")
    log(f"README says         : {res.expected:,}  ->  " + ("MATCH" if res.count_matches else f"MISMATCH (difference {res.rows - res.expected:+,})"))
    log(f"distinct headwords  : {res.distinct_words:,}  (normalized; {res.rows - res.distinct_words:,} rows share a spelling with another)")
    log(f"size before VACUUM  : {human(res.size_before_vacuum)}")
    log(f"size final          : {human(res.size_final)}   ({res.out})")
    for w in res.warnings:
        log(f"WARNING: {w}")


def main(argv: Optional[List[str]] = None) -> int:
    for stream in (sys.stdout, sys.stderr):
        try:
            stream.reconfigure(encoding="utf-8", errors="replace")  # type: ignore[attr-defined]
        except Exception:
            pass
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--src", type=Path, default=ROOT / "data" / "Dehkhoda-SQL", help="پوشهٔ دامپ‌ها (پیش‌فرض: data/Dehkhoda-SQL)")
    ap.add_argument("--out", type=Path, default=ROOT / "src-tauri" / "resources" / "dehkhoda.sqlite")
    ap.add_argument("--update", action="store_true", help="مخزن داده را به‌روز کن (git pull)")
    ap.add_argument("--no-download", action="store_true", help="هرگز دانلود نکن")
    ap.add_argument("--no-fts", action="store_true", help="جدول FTS5 را نساز (فایل کوچک‌تر، بدون جست‌وجوی متن‌کامل)")
    ap.add_argument("--expected", type=int, default=None, help="تعداد ردیف مورد انتظار (پیش‌فرض: عدد README مخزن)")
    ap.add_argument("--strict", action="store_true", help="اگر تعداد ردیف‌ها با README نمی‌خواند با کد خطا خارج شو")
    ap.add_argument("--limit-files", type=int, default=None, help="فقط n فایل اول (برای آزمایش سریع)")
    ap.add_argument("--no-verify", action="store_true", help="بررسی نهایی (tools/verify_db.py) را اجرا نکن")
    args = ap.parse_args(argv)

    if not args.no_download:
        ensure_source(args.src, update=args.update)
    res = build(args.src, args.out, with_fts=not args.no_fts, expected=args.expected, limit_files=args.limit_files)
    print_report(res)

    problems: List[str] = []
    if not args.no_verify:
        import verify_db

        log("")
        log("=== Verification ===")
        problems = verify_db.verify(args.out, expected_rows=res.rows, verbose=True)
        for p in problems:
            log(f"PROBLEM: {p}")
        if not problems:
            log("all checks passed")
    if problems:
        return 1
    if args.strict and not res.count_matches:
        return 3
    return 0


if __name__ == "__main__":
    sys.exit(main())
