import gzip
import os
import random
import sys
import tempfile
import unittest

sys.path.insert(0, os.path.join(os.path.dirname(__file__), ".."))

from dehkhoda_sql import SqlParseError, iter_inserts, iter_word_rows, unescape_mysql  # noqa: E402


def mysql_escape(s: str, rnd: random.Random) -> str:
    """مثل mysqldump/phpMyAdmin: رشته را quoted و escape می‌کند (گاهی ' را دوتایی می‌نویسد)."""
    out = []
    for ch in s:
        if ch == "\\":
            out.append("\\\\")
        elif ch == "'":
            out.append("''" if rnd.random() < 0.3 else "\\'")
        elif ch == '"':
            out.append('\\"' if rnd.random() < 0.7 else '"')
        elif ch == "\n":
            out.append("\\n")
        elif ch == "\r":
            out.append("\\r")
        elif ch == "\t":
            out.append("\\t" if rnd.random() < 0.5 else "\t")
        elif ch == "\x00":
            out.append("\\0")
        elif ch == "\x1a":
            out.append("\\Z")
        else:
            out.append(ch)
    return "'" + "".join(out) + "'"


class UnescapeTests(unittest.TestCase):
    def test_basic_escapes(self):
        self.assertEqual(unescape_mysql(r"a\'b"), "a'b")
        self.assertEqual(unescape_mysql(r'a\"b'), 'a"b')
        self.assertEqual(unescape_mysql(r"a\\b"), "a\\b")
        self.assertEqual(unescape_mysql(r"x\ny\rz\t"), "x\ny\rz\t")
        self.assertEqual(unescape_mysql(r"\0\Z\b"), "\x00\x1a\b")
        self.assertEqual(unescape_mysql("it''s"), "it's")

    def test_backslash_before_quote_is_one_escape(self):
        # \\ بعد از آن ' : دو کاراکتر واقعی (\ و ')
        self.assertEqual(unescape_mysql("\\\\\\'"), "\\'")

    def test_like_wildcards_keep_backslash(self):
        self.assertEqual(unescape_mysql(r"100\%"), r"100\%")
        self.assertEqual(unescape_mysql(r"a\_b"), r"a\_b")

    def test_unknown_escape_drops_backslash(self):
        self.assertEqual(unescape_mysql(r"\q"), "q")


class ParserTests(unittest.TestCase):
    def rows(self, sql):
        return list(iter_inserts(sql))

    def test_multi_row_insert(self):
        sql = "INSERT INTO `words` (`id`, `word`, `meaning`) VALUES\n(1, 'a', 'x'),\n(2, 'b', 'y');\n"
        self.assertEqual(
            self.rows(sql),
            [("words", ("id", "word", "meaning"), (1, "a", "x")), ("words", ("id", "word", "meaning"), (2, "b", "y"))],
        )

    def test_phpmyadmin_style_escaped_html(self):
        sql = "INSERT INTO `words` (`id`, `word`, `meaning`) VALUES\n(7, 'آب', '<span class=\\\"hlight\\\">آب.</span><br />x');"
        (_, _, vals), = self.rows(sql)
        self.assertEqual(vals, (7, "آب", '<span class="hlight">آب.</span><br />x'))

    def test_tricky_content_inside_strings(self):
        tricky = "a'),(2,'b'); INSERT INTO `words` VALUES (3,'c','d'); -- /* \\ ; ( )"
        sql = "INSERT INTO `words` (`id`,`word`,`meaning`) VALUES (1, %s, 'm');" % mysql_escape(tricky, random.Random(1))
        (_, _, vals), = self.rows(sql)
        self.assertEqual(vals, (1, tricky, "m"))

    def test_other_statements_are_skipped(self):
        sql = (
            "-- phpMyAdmin SQL Dump\n-- INSERT INTO `words` VALUES (99,'no','no');\n"
            "SET SQL_MODE = \"NO_AUTO_VALUE_ON_ZERO\";\n/*!40101 SET NAMES utf8mb4 */;\n"
            "CREATE TABLE `words` (\n  `id` int(255) NOT NULL COMMENT 'a;b'\n) ENGINE=InnoDB;\n"
            "INSERT INTO `words` (`id`, `word`, `meaning`) VALUES (1, 'a', 'b');\n"
            "ALTER TABLE `words`\n  ADD PRIMARY KEY (`id`);\nCOMMIT;\n"
        )
        self.assertEqual([r[2] for r in self.rows(sql)], [(1, "a", "b")])

    def test_numbers_and_null(self):
        sql = "INSERT INTO t VALUES (-5, NULL, 1.5, 'x'), (6, null, 2e3, '');"
        self.assertEqual([r[2] for r in self.rows(sql)], [(-5, None, 1.5, "x"), (6, None, 2000.0, "")])

    def test_multiple_insert_statements(self):
        sql = "INSERT INTO t (a) VALUES (1),(2);\nINSERT INTO t (a) VALUES (3);"
        self.assertEqual([r[2][0] for r in self.rows(sql)], [1, 2, 3])

    def test_missing_trailing_semicolon_at_eof(self):
        self.assertEqual(len(self.rows("INSERT INTO t (a) VALUES (1),(2)")), 2)

    def test_unterminated_string_raises_with_position(self):
        with self.assertRaises(SqlParseError) as cm:
            self.rows("INSERT INTO t (a) VALUES\n(1),\n(2, 'oops);")
        self.assertIn("خط 3", str(cm.exception))

    def test_garbage_between_rows_raises(self):
        with self.assertRaises(SqlParseError):
            self.rows("INSERT INTO t (a) VALUES (1) (2);")

    def test_unclosed_comment_raises(self):
        with self.assertRaises(SqlParseError):
            self.rows("/* never closed")

    def test_fuzz_roundtrip(self):
        rnd = random.Random(20260615)
        alphabet = list("ab ,;()'\"\\\n\r\t\x00\x1a%_-/*#`") + list("آبپکگیه\u200c\u064e") + ["<br />", "&lt;", "INSERT INTO "]
        expected, parts = [], []
        for i in range(1, 1500):
            word = "".join(rnd.choice(alphabet) for _ in range(rnd.randint(0, 12)))
            meaning = "".join(rnd.choice(alphabet) for _ in range(rnd.randint(0, 80)))
            expected.append((i, word, meaning))
            parts.append("(%d, %s, %s)" % (i, mysql_escape(word, rnd), mysql_escape(meaning, rnd)))
        # چند INSERT جدا با اندازه‌های متفاوت
        sql, k = "", 0
        while k < len(parts):
            step = rnd.randint(1, 400)
            sql += "INSERT INTO `words` (`id`, `word`, `meaning`) VALUES\n" + ",\n".join(parts[k:k + step]) + ";\n\n-- c\n"
            k += step
        got = list(iter_word_rows(text=sql))
        self.assertEqual(len(got), len(expected))
        self.assertEqual(got, expected)


class WordRowTests(unittest.TestCase):
    def test_column_order_follows_header(self):
        sql = "INSERT INTO `words` (`meaning`, `id`, `word`) VALUES ('M', 5, 'W');"
        self.assertEqual(list(iter_word_rows(text=sql)), [(5, "W", "M")])

    def test_missing_column_raises(self):
        with self.assertRaises(SqlParseError):
            list(iter_word_rows(text="INSERT INTO `words` (`id`, `word`) VALUES (1, 'a');"))

    def test_other_tables_ignored(self):
        sql = "INSERT INTO `other` (`id`,`word`,`meaning`) VALUES (1,'a','b');INSERT INTO `words` (`id`,`word`,`meaning`) VALUES (2,'c','d');"
        self.assertEqual(list(iter_word_rows(text=sql)), [(2, "c", "d")])

    def test_reads_gzip_utf8(self):
        sql = "INSERT INTO `words` (`id`, `word`, `meaning`) VALUES (1, 'آب', 'سلام\\n');"
        with tempfile.TemporaryDirectory() as d:
            p = os.path.join(d, "x.sql.gz")
            with gzip.open(p, "wt", encoding="utf-8") as f:
                f.write(sql)
            self.assertEqual(list(iter_word_rows(p)), [(1, "آب", "سلام\n")])


if __name__ == "__main__":
    unittest.main()
