import contextlib
import io
import os
import shutil
import sqlite3
import sys
import tempfile
import unittest
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent))

import build_db  # noqa: E402
import verify_db  # noqa: E402

FIXTURE = HERE / "fixtures" / "mini"
COMMITTED = HERE.parent.parent / "crates" / "dehkhoda-core" / "tests" / "fixtures" / "mini.sqlite"


def quiet_build(*a, **kw):
    with contextlib.redirect_stdout(io.StringIO()):
        return build_db.build(*a, **kw)


class BuildTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.tmp = Path(tempfile.mkdtemp())
        cls.out = cls.tmp / "mini.sqlite"
        cls.res = quiet_build(FIXTURE, cls.out, expected=29)
        cls.conn = sqlite3.connect(cls.out)

    @classmethod
    def tearDownClass(cls):
        cls.conn.close()
        shutil.rmtree(cls.tmp, ignore_errors=True)

    def one(self, sql, *args):
        return self.conn.execute(sql, args).fetchone()

    def test_counts_and_report(self):
        self.assertEqual(self.res.rows, 29)
        self.assertEqual([n for _, n in self.res.per_file], [26, 3])
        self.assertTrue(self.res.count_matches)
        self.assertEqual(self.one("SELECT COUNT(*) FROM words")[0], 29)

    def test_schema_columns_in_order(self):
        cols = [r[1] for r in self.conn.execute("PRAGMA table_info(words)")]
        self.assertEqual(cols, ["id", "word", "word_norm", "word_fold", "meaning_html", "meaning_text"])
        names = {r[0] for r in self.conn.execute("SELECT name FROM sqlite_master")}
        self.assertTrue({"words", "meta", "words_fts", "heads", "heads_trgm", "idx_words_norm", "idx_heads_norm", "idx_heads_fold"} <= names)

    def test_normalized_columns(self):
        self.assertEqual(self.one("SELECT word_norm, word_fold FROM words WHERE id=1"), ("آب", "اب"))
        self.assertEqual(self.one("SELECT word_norm, word_fold FROM words WHERE id=4"), ("اب", "اب"))
        self.assertEqual(self.one("SELECT word_norm FROM words WHERE id=10")[0], "بنی امیه")
        self.assertEqual(self.one("SELECT word_norm FROM words WHERE id=9")[0], "ابن مقله")
        self.assertEqual(self.one("SELECT word_norm FROM words WHERE id=27")[0], "کوه")  # ك عربی → ک

    def test_homographs_kept_as_separate_rows(self):
        self.assertEqual(self.one("SELECT COUNT(*) FROM words WHERE word_norm='آب'")[0], 3)
        self.assertEqual(self.res.distinct_words, 29 - 2)

    def test_html_unescaped_and_preserved(self):
        html = self.one("SELECT meaning_html FROM words WHERE id=1")[0]
        self.assertIn('<span class="hlight">آب .</span>', html)
        self.assertIn('<font color="Red">', html)
        self.assertNotIn('\\"', html)

    def test_meaning_text_is_plain_and_normalized(self):
        t = self.one("SELECT meaning_text FROM words WHERE id=2")[0]
        self.assertNotIn("<", t)
        self.assertIn("غره آن", t)  # «غُرّه ٔ آن»: اعراب، تشدید و U+0654 حذف شد
        t28 = self.one("SELECT meaning_text FROM words WHERE id=28")[0]
        self.assertEqual(t28, "ده. (عدد) 10 و 10 و 10.")
        t27 = self.one("SELECT meaning_text FROM words WHERE id=27")[0]
        self.assertEqual(t27, "کوه. (ا) برآمدگی زمین، علی و موسی.")

    def test_sql_escapes_roundtrip(self):
        self.assertEqual(self.one("SELECT meaning_html FROM words WHERE id=29")[0], "it's a \\ test; 100\\% done' ok<br />")

    def test_fts_matches(self):
        def ids(q):
            return sorted(r[0] for r in self.conn.execute("SELECT rowid FROM words_fts WHERE words_fts MATCH ?", (q,)))

        self.assertEqual(ids('"خطاط"'), [9])
        self.assertEqual(ids('"اسیاب"'), [8])          # fold: «آسیاب» با «اسیاب» پیدا می‌شود
        self.assertEqual(ids('"آسیاب"'), [])           # ورودی باید قبل از جست‌وجو fold شود
        self.assertEqual(ids('"خط" "نسخ"'), [9, 12])   # AND
        self.assertEqual(ids('"خط نسخ"'), [9])         # عبارت
        self.assertEqual(ids('"جای" "نگهداری"'), [12, 22])
        self.assertEqual(ids('"اسیا"*'), [7, 8])       # پیشوندی
        self.assertEqual(ids('"ده"'), [28])

    def test_heads_table_groups_homographs(self):
        rows = {r[0]: r for r in self.conn.execute("SELECT word_norm, id, word, word_fold, cnt FROM heads")}
        self.assertEqual(len(rows), 27)
        self.assertEqual(rows["آب"], ("آب", 1, "آب", "اب", 3))
        self.assertEqual(rows["اب"], ("اب", 4, "اب", "اب", 1))
        self.assertEqual(rows["بنی امیه"][2], "بنی امیّه")  # نوشتار اصلی حفظ می‌شود
        self.assertEqual(rows["کوه"][1:3], (27, "كوه"))

    def test_trigram_substring_search_on_heads(self):
        def ids(q):
            return sorted(r[0] for r in self.conn.execute("SELECT rowid FROM heads_trgm WHERE heads_trgm MATCH ?", (q,)))

        self.assertEqual(ids('"بار"'), [19, 20, 21, 22])         # بار، باران، ابار، انبار
        self.assertEqual(ids('"تاب"'), [11, 12, 13])             # کتاب، کتابخانه، کتابفروش
        self.assertEqual(ids('"ب اس"'), [7])                     # فاصله هم جزو رشته است: «اب اسیا»
        self.assertEqual(ids('"زرافه"'), [])

    def test_meta(self):
        meta = dict(self.conn.execute("SELECT key, value FROM meta"))
        self.assertEqual(meta["row_count"], "29")
        self.assertEqual(meta["has_fts"], "1")
        self.assertEqual(meta["norm_version"], "1")
        self.assertEqual(meta["schema_version"], "2")
        self.assertTrue(meta["built_at"].endswith("Z"))
        self.assertEqual(self.one("PRAGMA user_version")[0], 2)

    def test_vacuumed_and_not_wal(self):
        self.assertEqual(self.one("PRAGMA freelist_count")[0], 0)
        self.assertEqual(self.one("PRAGMA journal_mode")[0].lower(), "delete")
        self.assertFalse(Path(str(self.out) + ".tmp").exists())
        self.assertFalse(Path(str(self.out) + ".new").exists())

    def test_verify_passes(self):
        self.assertEqual(verify_db.verify(self.out, expected_rows=29), [])

    def test_verify_detects_tampering(self):
        bad = self.tmp / "bad.sqlite"
        shutil.copy(self.out, bad)
        c = sqlite3.connect(bad)
        c.execute("UPDATE words SET word_norm='x' WHERE id=3")
        c.commit()
        c.close()
        problems = verify_db.verify(bad)
        self.assertTrue(any("normalization mismatch" in p for p in problems), problems)

    def test_verify_detects_wrong_expected_count(self):
        self.assertTrue(any("expected" in p for p in verify_db.verify(self.out, expected_rows=30)))


class BuildVariantTests(unittest.TestCase):
    def test_count_mismatch_is_reported(self):
        with tempfile.TemporaryDirectory() as d:
            res = quiet_build(FIXTURE, Path(d) / "x.sqlite", expected=30)
            self.assertFalse(res.count_matches)

    def test_no_fts(self):
        with tempfile.TemporaryDirectory() as d:
            out = Path(d) / "x.sqlite"
            quiet_build(FIXTURE, out, with_fts=False, expected=29)
            with contextlib.closing(sqlite3.connect(out)) as c:
                names = {r[0] for r in c.execute("SELECT name FROM sqlite_master")}
                self.assertNotIn("words_fts", names)
                self.assertEqual(dict(c.execute("SELECT key, value FROM meta"))["has_fts"], "0")
            self.assertEqual(verify_db.verify(out, expected_rows=29), [])

    def test_limit_files(self):
        with tempfile.TemporaryDirectory() as d:
            res = quiet_build(FIXTURE, Path(d) / "x.sqlite", expected=26, limit_files=1)
            self.assertEqual(res.rows, 26)

    def test_duplicate_ids_abort(self):
        with tempfile.TemporaryDirectory() as d:
            src = Path(d) / "src"
            src.mkdir()
            (src / "a.sql").write_text("INSERT INTO `words` (`id`,`word`,`meaning`) VALUES (1,'a','b'),(1,'c','d');", encoding="utf-8")
            with self.assertRaises(SystemExit):
                quiet_build(src, Path(d) / "x.sqlite", expected=2)

    def test_expected_rows_read_from_readme(self):
        with tempfile.TemporaryDirectory() as d:
            (Path(d) / "README.md").write_text("# X\nIt contains 312507 words.\n", encoding="utf-8")
            self.assertEqual(build_db.expected_rows_from_readme(Path(d)), 312507)
            (Path(d) / "README.md").write_text("It contains 1,234 words.", encoding="utf-8")
            self.assertEqual(build_db.expected_rows_from_readme(Path(d)), 1234)


@unittest.skipUnless(COMMITTED.exists(), "committed fixture DB not generated yet")
class CommittedFixtureTests(unittest.TestCase):
    def test_committed_fixture_matches_a_fresh_build(self):
        with tempfile.TemporaryDirectory() as d:
            out = Path(d) / "fresh.sqlite"
            quiet_build(FIXTURE, out, expected=29)
            q = "SELECT id, word, word_norm, word_fold, meaning_html, meaning_text FROM words ORDER BY id"
            with contextlib.closing(sqlite3.connect(out)) as ca, \
                 contextlib.closing(sqlite3.connect(COMMITTED)) as cb:
                a = ca.execute(q).fetchall()
                b = cb.execute(q).fetchall()
            self.assertEqual(a, b, "tests/fixtures/mini.sqlite is stale; regenerate it (see README)")


if __name__ == "__main__":
    unittest.main()
