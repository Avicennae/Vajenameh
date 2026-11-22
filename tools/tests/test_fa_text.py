import json
import os
import random
import sys
import unittest

HERE = os.path.dirname(__file__)
sys.path.insert(0, os.path.join(HERE, ".."))

from fa_text import NORM_VERSION, fold, html_to_text, meaning_to_search_text, normalize  # noqa: E402

VECTORS_PATH = os.path.join(HERE, "..", "..", "shared", "normalization_vectors.json")


def load_vectors():
    with open(VECTORS_PATH, encoding="utf-8") as f:
        return json.load(f)


class VectorTests(unittest.TestCase):
    def test_version_matches_vectors(self):
        self.assertEqual(load_vectors()["norm_version"], NORM_VERSION)

    def test_all_vectors(self):
        cases = load_vectors()["cases"]
        self.assertGreaterEqual(len(cases), 30)
        for c in cases:
            with self.subTest(c["name"]):
                n = normalize(c["in"])
                self.assertEqual(n, c["norm"], "norm")
                self.assertEqual(fold(n), c["fold"], "fold")


class PropertyTests(unittest.TestCase):
    ALPHABET = list("آاأإٱبپتكيىةۀهیکگ ٔ\u0653\u064e\u0650\u0651\u0670\u0640\u200c\u200d\u200e\u200f\u00a0\t\n-AZaz09٠٩۰۹،؛") + ["\U0001F600", "e\u0301"]

    def test_idempotent_and_length_preserving_fold(self):
        rnd = random.Random(7)
        for _ in range(5000):
            s = "".join(rnd.choice(self.ALPHABET) for _ in range(rnd.randint(0, 14)))
            n = normalize(s)
            self.assertEqual(normalize(n), n, repr(s))
            f = fold(n)
            self.assertEqual(fold(f), f)
            self.assertEqual(len(f), len(n))  # برای ساخت snippet در Rust لازم است
            self.assertNotIn("  ", n)
            self.assertEqual(n, n.strip(" "))

    def test_output_has_no_removed_characters(self):
        rnd = random.Random(8)
        banned = set("\u064b\u064c\u064d\u064e\u064f\u0650\u0651\u0652\u0654\u0670\u0640\u200c\u200d\u200e\u200f\u00a0\t\n\u0643\u064a\u0649\u0629\u06c0")
        for _ in range(3000):
            s = "".join(rnd.choice(self.ALPHABET) for _ in range(rnd.randint(0, 14)))
            self.assertFalse(banned & set(normalize(s)), repr(s))

    def test_exact_spelling_vs_folded(self):
        # «اب» باید «آب» را در سطح fold پیدا کند ولی در سطح normalize با آن برابر نباشد
        self.assertEqual(fold(normalize("اب")), fold(normalize("آب")))
        self.assertNotEqual(normalize("اب"), normalize("آب"))


class HtmlTests(unittest.TestCase):
    def test_strips_tags_and_unescapes(self):
        h = '<span class="hlight">آ.</span> (حرف ) x<br />y<BR>z &lt;b&gt; &amp; &nbsp;w'
        self.assertEqual(html_to_text(h), "آ. (حرف ) x y z <b> & \u00a0w")

    def test_block_tags_do_not_glue_words(self):
        self.assertEqual(html_to_text("a<br/>b<div>c</div>d"), "a b c d")

    def test_inline_tags_do_not_split_words(self):
        self.assertEqual(html_to_text("ک<b>تاب</b>"), "کتاب")

    def test_real_data_pattern(self):
        h = '<font color=\"DarkGreen\"><b>[ ژَ ]</font></b> (اِ مصغر) غُرّه ٔ آن.<br /></font></div>'
        self.assertEqual(meaning_to_search_text(h), "[ ژ ] (ا مصغر) غره آن.")




class GoldenTests(unittest.TestCase):
    def test_committed_golden_is_up_to_date(self):
        import gen_golden

        with open(gen_golden.OUT, encoding="utf-8") as f:
            committed = f.read()
        self.assertEqual(committed, gen_golden.render(gen_golden.generate()),
                         "shared/normalization_golden.json is stale: run `python tools/gen_golden.py`")


if __name__ == "__main__":
    unittest.main()
