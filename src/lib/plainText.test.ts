import { describe, expect, it } from "vitest";
import type { EntryGroup } from "../types";
import { groupToPlainText, htmlToPlainText } from "./plainText";

describe("htmlToPlainText", () => {
  it("drops tags, keeps line breaks and decodes entities", () => {
    const html = '<span class="hlight">آب .</span> (اِ) x<br><font color="blue">◄</font> دریا.<br><br><br>y &lt;z&gt; &amp;';
    expect(htmlToPlainText(html)).toBe("آب . (اِ) x\n◄ دریا.\n\ny <z> &");
  });

  it("does not execute or keep script-like content as markup", () => {
    expect(htmlToPlainText('<img src=x onerror="window.__hacked=1">متن')).toBe("متن");
    expect((window as unknown as { __hacked?: number }).__hacked).toBeUndefined();
  });

  it("joins the entries of a group with a separator", () => {
    const g: EntryGroup = {
      wordNorm: "آب", word: "آب", total: 2, offset: 0, pageSize: 40,
      entries: [{ id: 1, word: "آب", html: "یک<br>دو" }, { id: 2, word: "آب", html: "سه" }],
    };
    expect(groupToPlainText(g)).toBe("یک\nدو\n\n———\n\nسه");
  });
});
