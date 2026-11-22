#!/usr/bin/env python3
"""
بررسی سلامت و سازگاری یک پایگاه ساخته‌شده توسط build_db.py.

    python tools/verify_db.py [مسیر فایل]    (پیش‌فرض: src-tauri/resources/dehkhoda.sqlite)

از تابع verify() هم می‌توان در تست‌ها استفاده کرد؛ خروجی فهرست مشکلات است (خالی = سالم).
"""
from __future__ import annotations

import os
import re
import sqlite3
import sys
from pathlib import Path
from typing import List, Optional

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

from fa_text import NORM_VERSION, fold, normalize  # noqa: E402

ROOT = Path(__file__).resolve().parent.parent
_TOKEN = re.compile(r"[^\W_]+", re.UNICODE)


def verify(path, expected_rows: Optional[int] = None, verbose: bool = False) -> List[str]:
    problems: List[str] = []

    def say(msg: str) -> None:
        if verbose:
            print("  " + msg, flush=True)

    uri = "file:" + str(Path(path).resolve()).replace("\\", "/").replace("?", "%3f") + "?mode=ro"
    conn = sqlite3.connect(uri, uri=True)
    try:
        qc = conn.execute("PRAGMA quick_check").fetchone()[0]
        say(f"quick_check: {qc}")
        if qc != "ok":
            problems.append(f"quick_check failed: {qc}")

        jm = conn.execute("PRAGMA journal_mode").fetchone()[0]
        if jm.lower() == "wal":
            problems.append("database is in WAL mode (needs -wal/-shm files; read-only install dirs would break)")

        meta = dict(conn.execute("SELECT key, value FROM meta"))
        say("meta: " + ", ".join(f"{k}={v}" for k, v in meta.items() if k in ("schema_version", "norm_version", "has_fts", "row_count", "source_commit", "built_at")))
        if meta.get("norm_version") != str(NORM_VERSION):
            problems.append(f"norm_version in DB ({meta.get('norm_version')}) != fa_text.NORM_VERSION ({NORM_VERSION})")

        n = conn.execute("SELECT COUNT(*) FROM words").fetchone()[0]
        say(f"words rows: {n:,}")
        if str(n) != meta.get("row_count"):
            problems.append(f"row_count in meta ({meta.get('row_count')}) != COUNT(*) ({n})")
        if expected_rows is not None and n != expected_rows:
            problems.append(f"row count {n} != expected {expected_rows}")

        # ستون‌های نرمال‌شده باید دقیقاً با نرمال‌ساز مرجع بخوانند
        bad_norm = bad_fold = bad_text = empty_html = empty_text = 0
        first_bad = None
        for rid, word, wn, wf, mt, html_len in conn.execute(
            "SELECT id, word, word_norm, word_fold, meaning_text, length(meaning_html) FROM words"
        ):
            if normalize(word) != wn:
                bad_norm += 1
                first_bad = first_bad or (rid, "word_norm")
            if fold(wn) != wf:
                bad_fold += 1
                first_bad = first_bad or (rid, "word_fold")
            if normalize(mt) != mt:
                bad_text += 1
                first_bad = first_bad or (rid, "meaning_text")
            if not html_len:
                empty_html += 1
            if not mt:
                empty_text += 1
        say(f"recomputed word_norm/word_fold/meaning_text for all rows: mismatches {bad_norm}/{bad_fold}/{bad_text}; empty html {empty_html}, empty text {empty_text}")
        if bad_norm or bad_fold or bad_text:
            problems.append(f"normalization mismatch (norm {bad_norm}, fold {bad_fold}, text {bad_text}); first at {first_bad}")

        # ایندکس‌ها واقعاً استفاده می‌شوند
        plan = " ".join(r[3] for r in conn.execute("EXPLAIN QUERY PLAN SELECT id, word, word_norm, cnt FROM heads WHERE word_fold > 'a' AND word_fold < 'b'"))
        say("plan(prefix on heads.word_fold): " + plan)
        if "idx_heads_fold" not in plan:
            problems.append("prefix query does not use idx_heads_fold")
        plan = " ".join(r[3] for r in conn.execute("EXPLAIN QUERY PLAN SELECT id FROM words WHERE word_norm = 'a'"))
        if "idx_words_norm" not in plan:
            problems.append("exact query does not use idx_words_norm")

        # جدول سرواژه‌های متمایز باید دقیقاً با words هم‌خوان باشد
        n_heads = conn.execute("SELECT COUNT(*) FROM heads").fetchone()[0]
        n_distinct = conn.execute("SELECT COUNT(DISTINCT word_norm) FROM words").fetchone()[0]
        say(f"heads rows: {n_heads:,} (distinct word_norm in words: {n_distinct:,})")
        if n_heads != n_distinct or str(n_heads) != meta.get("distinct_words"):
            problems.append(f"heads has {n_heads} rows, words has {n_distinct} distinct word_norm, meta says {meta.get('distinct_words')}")
        diff = conn.execute(
            "SELECT COUNT(*) FROM (SELECT word_norm, COUNT(*) c, MIN(id) m FROM words GROUP BY word_norm) g "
            "LEFT JOIN heads h ON h.word_norm = g.word_norm WHERE h.id IS NOT g.m OR h.cnt IS NOT g.c"
        ).fetchone()[0]
        if diff:
            problems.append(f"{diff} heads rows disagree with words (min id / count)")
        trgm_docs = conn.execute("SELECT COUNT(*) FROM heads_trgm_docsize").fetchone()[0]
        if trgm_docs != n_heads:
            problems.append(f"trigram index has {trgm_docs} documents but heads has {n_heads} rows")
        row = conn.execute("SELECT id, word_fold FROM heads WHERE length(word_fold) >= 6 ORDER BY id LIMIT 1 OFFSET ?", (n_heads // 2,)).fetchone()
        if row:
            sub = row[1][1:5]
            ids = [r[0] for r in conn.execute("SELECT rowid FROM heads_trgm WHERE heads_trgm MATCH ?", ('"%s"' % sub.replace('"', '""'),))]
            say(f"trigram sample: substring {sub!r} of head {row[0]} -> {len(ids)} matches")
            if row[0] not in ids:
                problems.append(f"trigram query for {sub!r} did not return head {row[0]}")

        if meta.get("has_fts") == "1":
            docs = conn.execute("SELECT COUNT(*) FROM words_fts_docsize").fetchone()[0]
            say(f"FTS docs: {docs:,}")
            if docs != n:
                problems.append(f"FTS has {docs} documents but words has {n} rows")
            # نمونه: توکنی از وسط دیتا باید خودِ آن سند را پیدا کند
            row = conn.execute("SELECT id, meaning_text FROM words WHERE id >= ? ORDER BY id LIMIT 1", (max(1, n // 2),)).fetchone()
            if row:
                rid, mt = row
                toks = sorted(_TOKEN.findall(fold(mt)), key=len, reverse=True)
                if toks:
                    hit = conn.execute('SELECT rowid FROM words_fts WHERE words_fts MATCH ? LIMIT 5000', ('"%s"' % toks[0].replace('"', '""'),)).fetchall()
                    ok = any(h[0] == rid for h in hit)
                    say(f"FTS sample: token {toks[0]!r} of id {rid} -> {len(hit)} hits, contains id: {ok}")
                    if not ok and len(hit) < 5000:
                        problems.append(f"FTS sample query for id {rid} did not return it")
    finally:
        conn.close()
    return problems


def main() -> int:
    for stream in (sys.stdout, sys.stderr):
        try:
            stream.reconfigure(encoding="utf-8", errors="replace")  # type: ignore[attr-defined]
        except Exception:
            pass
    path = sys.argv[1] if len(sys.argv) > 1 else ROOT / "src-tauri" / "resources" / "dehkhoda.sqlite"
    print(f"Verifying {path}", flush=True)
    problems = verify(path, verbose=True)
    for p in problems:
        print("PROBLEM:", p)
    print("OK" if not problems else f"{len(problems)} problem(s)")
    return 1 if problems else 0


if __name__ == "__main__":
    sys.exit(main())
