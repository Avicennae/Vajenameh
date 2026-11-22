#!/usr/bin/env python3
"""
تولید shared/normalization_golden.json: چند هزار رشتهٔ تصادفی (قطعی، با seed ثابت) به‌همراه
خروجی normalize/fold نسخهٔ Python. تست‌های Rust همین فایل را می‌خوانند و باید دقیقاً همان
خروجی را بدهند. اگر قواعد نرمال‌ساز را عوض کردید این اسکریپت را دوباره اجرا کنید.
"""
import json
import os
import random
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
from fa_text import NORM_VERSION, fold, normalize  # noqa: E402

OUT = os.path.join(HERE, "..", "shared", "normalization_golden.json")
SEED = 20261115
COUNT = 4000


def alphabet():
    a = list("اآأإٱبپتثجچحخدذرزژسشصضطظعغفقكکگلمنهوؤيیىئءةۀ")
    a += [chr(c) for c in range(0x064B, 0x0656)] + ["\u0670", "\u0640", "\u0654", "\u0653"]
    a += ["\u200c", "\u200d", "\u200e", "\u200f", "\u00a0", "\t", "\n", " ", " ", " ", "\u2003", "\u200b", "\u3000", "\ufeff", "\u00ad", "\u061c"]
    a += [chr(c) for c in range(0x202A, 0x202F)] + [chr(c) for c in range(0x2066, 0x206A)]
    a += [chr(0x06F0 + i) for i in range(10)] + [chr(0x0660 + i) for i in range(10)] + list("0123456789")
    a += list("ABCxyzXYZ") + ["É", "é", "e\u0301", "\U0001F600", "ß", "İ"]
    a += list("-،؛!\"'()[]:.؟")
    return a


def generate():
    rnd = random.Random(SEED)
    al = alphabet()
    cases = []
    for _ in range(COUNT):
        s = "".join(rnd.choice(al) for _ in range(rnd.randint(0, 24)))
        n = normalize(s)
        cases.append({"in": s, "norm": n, "fold": fold(n)})
    return {"norm_version": NORM_VERSION, "seed": SEED, "cases": cases}


def render(data) -> str:
    return json.dumps(data, ensure_ascii=True, indent=0, separators=(",", ":")) + "\n"


if __name__ == "__main__":
    with open(OUT, "w", encoding="utf-8") as f:
        f.write(render(generate()))
    print("wrote", os.path.normpath(OUT), COUNT, "cases")
