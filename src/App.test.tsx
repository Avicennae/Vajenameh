import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import App from "./App";
import { bootstrap, makeApi, makeGroup } from "./test/fakeApi";
import { DEFAULT_SETTINGS } from "./types";

const FAST = { debounceWordMs: 5, debounceFtsMs: 5 };

function setup(over: Parameters<typeof makeApi>[0] = {}, bootOver: Parameters<typeof bootstrap>[0] = {}, options = FAST) {
  const api = makeApi(over);
  const user = userEvent.setup();
  const view = render(<App api={api} boot={bootstrap(bootOver)} options={options} />);
  const input = () => screen.getByRole("combobox");
  return { api, user, input, ...view };
}

const options = () => screen.queryAllByRole("option");
const names = () => options().map((o) => o.textContent ?? "");

describe("search as you type", () => {
  it("shows a welcome screen first and focuses the search box", () => {
    const { input } = setup();
    expect(screen.getByRole("heading", { name: "لغت‌نامهٔ دهخدا", level: 1 })).toBeInTheDocument();
    expect(screen.getByText(/۳۱۲٬۵۰۷ مدخل/)).toBeInTheDocument();
    expect(input()).toHaveFocus();
  });

  it("searches once (debounced) and lists results with homograph counts", async () => {
    const { api, user, input } = setup({}, {}, { debounceWordMs: 120, debounceFtsMs: 120 }); // بلندتر از فاصلهٔ دو ضربهٔ کلید
    await user.type(input(), "آب");
    await waitFor(() => expect(options()).toHaveLength(3));
    expect(api.searchWords).toHaveBeenCalledTimes(1);
    expect(api.searchWords).toHaveBeenCalledWith("آب");
    expect(names()[0]).toContain("آب");
    expect(names()[0]).toContain("۳"); // سه مدخل هم‌املا
    expect(screen.getByText("۳ نتیجه")).toBeInTheDocument();
  });

  it("explains empty and cleared states", async () => {
    const { user, input } = setup();
    await user.type(input(), "زرافه");
    expect(await screen.findByText("نتیجه‌ای پیدا نشد.")).toBeInTheDocument();
    await user.clear(input());
    expect(await screen.findByText("برای شروع، واژه‌ای بنویسید.")).toBeInTheDocument();
  });

  it("ignores a slow response that arrives after a newer query", async () => {
    let releaseSlow: (v: never[]) => void = () => {};
    const slow = new Promise<never[]>((r) => (releaseSlow = r));
    const { api, user, input } = setup({
      searchWords: vi.fn(async (q: string) => {
        if (q === "آ") return slow as never;
        return [{ id: 9, word: "آباد", wordNorm: "آباد", count: 1, kind: "prefix" as const }];
      }),
    });
    await user.type(input(), "آ");
    await waitFor(() => expect(api.searchWords).toHaveBeenCalledWith("آ"));
    await user.type(input(), "ب");
    await waitFor(() => expect(names()).toEqual(["آباد"]));
    releaseSlow([]); // پاسخ دیررسید و کهنه
    await new Promise((r) => setTimeout(r, 30));
    expect(names()).toEqual(["آباد"]);
  });
});

describe("keyboard", () => {
  it("arrows select, Enter opens, and the visit lands in the history", async () => {
    const { api, user, input } = setup();
    await user.type(input(), "آب");
    await waitFor(() => expect(options()).toHaveLength(3));
    expect(options()[0]).toHaveAttribute("aria-selected", "true");

    await user.keyboard("{ArrowDown}");
    expect(options()[1]).toHaveAttribute("aria-selected", "true");
    expect(input()).toHaveAttribute("aria-activedescendant", options()[1].id);
    await user.keyboard("{ArrowDown}{ArrowDown}{ArrowDown}"); // از انتها رد نمی‌شود
    expect(options()[2]).toHaveAttribute("aria-selected", "true");
    await user.keyboard("{ArrowUp}");
    expect(options()[1]).toHaveAttribute("aria-selected", "true");

    await user.keyboard("{Enter}");
    expect(await screen.findByRole("heading", { name: "آباد", level: 1 })).toBeInTheDocument();
    expect(api.getEntryGroup).toHaveBeenCalledWith("آباد", { focusId: undefined, offset: 0 });
    expect(api.recordVisit).toHaveBeenCalledWith({ wordNorm: "آباد", word: "آباد" });

    await user.click(screen.getByRole("tab", { name: /تاریخچه/ }));
    expect(names()).toEqual(["آباد"]);
  });

  it("Enter opens the first result straight away", async () => {
    const { api, user, input } = setup();
    await user.type(input(), "کتا");
    await waitFor(() => expect(options()).toHaveLength(1));
    await user.keyboard("{Enter}");
    await screen.findByRole("heading", { name: "کتاب", level: 1 });
    expect(api.getEntryGroup).toHaveBeenCalledTimes(1);
  });

  it("«type then Enter» right away still opens the first result once it arrives", async () => {
    let release: () => void = () => {};
    const gate = new Promise<void>((r) => (release = r));
    const { api, user, input } = setup({
      searchWords: vi.fn(async () => {
        await gate; // نتایج دیر می‌رسند
        return [{ id: 1, word: "آباد", wordNorm: "آباد", count: 1, kind: "prefix" as const }];
      }),
    });
    await user.type(input(), "آبا{Enter}");
    expect(api.getEntryGroup).not.toHaveBeenCalled();
    release();
    await screen.findByRole("heading", { name: "آباد", level: 1 });
    expect(api.getEntryGroup).toHaveBeenCalledTimes(1);
  });

  it("does not open anything on Enter when the query has no result", async () => {
    const { api, user, input } = setup();
    await user.type(input(), "زرافه{Enter}");
    await screen.findByText("نتیجه‌ای پیدا نشد.");
    expect(api.getEntryGroup).not.toHaveBeenCalled();
  });

  it("clicking a result opens it without stealing focus from the search box", async () => {
    const { user, input } = setup();
    await user.type(input(), "آب");
    await waitFor(() => expect(options()).toHaveLength(3));
    await user.click(options()[2]);
    await screen.findByRole("heading", { name: "آبادان", level: 1 });
    expect(input()).toHaveFocus();
  });

  it("Ctrl+K focuses and selects the search text", async () => {
    const { user, input } = setup();
    await user.type(input(), "آب");
    await user.tab(); // فوکوس از فیلد بیرون می‌رود
    expect(input()).not.toHaveFocus();
    await user.keyboard("{Control>}k{/Control}");
    expect(input()).toHaveFocus();
    expect((input() as HTMLInputElement).selectionStart).toBe(0);
    expect((input() as HTMLInputElement).selectionEnd).toBe(2);
  });

  it("Esc goes back to the search box first, then clears it", async () => {
    const { user, input } = setup();
    await user.type(input(), "آب");
    await waitFor(() => expect(options()).toHaveLength(3));
    await user.tab(); // فوکوس از فیلد خارج می‌شود
    expect(input()).not.toHaveFocus();
    await user.keyboard("{Escape}");
    expect(input()).toHaveFocus();
    expect(input()).toHaveValue("آب");
    await user.keyboard("{Escape}");
    expect(input()).toHaveValue("");
    await waitFor(() => expect(options()).toHaveLength(0));
  });

  it("Esc closes the About dialog before anything else", async () => {
    const { user, input } = setup();
    await user.type(input(), "آب");
    await user.click(screen.getByRole("button", { name: "درباره" }));
    expect(screen.getByRole("dialog", { name: "دربارهٔ برنامه" })).toBeInTheDocument();
    await user.keyboard("{Escape}");
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    expect(input()).toHaveValue("آب"); // هنوز پاک نشده
  });

  it("works with a Persian keyboard layout (matches the physical key, not the letter)", () => {
    const { input } = setup();
    (document.activeElement as HTMLElement).blur();
    expect(input()).not.toHaveFocus();
    // کلید فیزیکی K وقتی صفحه‌کلید فارسی فعال است نویسهٔ «ن» تولید می‌کند
    window.dispatchEvent(new KeyboardEvent("keydown", { key: "ن", code: "KeyK", ctrlKey: true, bubbles: true }));
    expect(input()).toHaveFocus();
  });
});

describe("entry page", () => {
  it("renders the sanitized html and the homograph label", async () => {
    const { user, input } = setup({ getEntryGroup: vi.fn(async () => makeGroup("آب", 3)) });
    await user.type(input(), "آب{Enter}");
    await screen.findByRole("heading", { name: "آب", level: 1 });
    expect(screen.getByText("۳ مدخل")).toBeInTheDocument();
    expect(screen.getByText("مدخل ۲")).toBeInTheDocument();
    const entry = document.querySelector(".entry-body")!;
    expect(entry.querySelector(".hlight")).toHaveTextContent("آب .");
    expect(entry.querySelector('font[color="red"]')).toHaveTextContent("سعدی .");
  });

  it("pages huge homograph groups without re-recording the visit", async () => {
    const { api, user, input } = setup();
    await user.type(input(), "آب{Enter}");
    expect(await screen.findAllByText("مدخل‌های ۱ تا ۴۰ از ۱۰۰")).toHaveLength(2); // بالا و پایین صفحه
    expect(screen.getAllByRole("navigation", { name: "صفحه‌های مدخل‌ها" })).toHaveLength(2);
    const next = screen.getAllByRole("button", { name: "مدخل‌های بعدی" })[0];
    const prev = screen.getAllByRole("button", { name: "مدخل‌های قبلی" })[0];
    expect(prev).toBeDisabled();
    await user.click(next);
    expect(await screen.findAllByText("مدخل‌های ۴۱ تا ۸۰ از ۱۰۰")).toHaveLength(2);
    expect(api.getEntryGroup).toHaveBeenLastCalledWith("آب", { focusId: undefined, offset: 40 });
    expect(api.recordVisit).toHaveBeenCalledTimes(1);
    await user.click(screen.getAllByRole("button", { name: "مدخل‌های بعدی" })[1]);
    expect(await screen.findAllByText("مدخل‌های ۸۱ تا ۱۰۰ از ۱۰۰")).toHaveLength(2);
    expect(screen.getAllByRole("button", { name: "مدخل‌های بعدی" })[0]).toBeDisabled();
  });

  it("copies the plain text of the page", async () => {
    const { user, input } = setup();
    await user.type(input(), "کتا{Enter}");
    await screen.findByRole("heading", { name: "کتاب", level: 1 });
    await user.click(screen.getByRole("button", { name: "کپی" }));
    expect(await navigator.clipboard.readText()).toBe("کتاب . معنی شمارهٔ 1\nسعدی .");
    expect(await screen.findByText("متن کپی شد.")).toBeInTheDocument();
  });

  it("reports a missing entry instead of crashing", async () => {
    const { user, input } = setup({ getEntryGroup: vi.fn(async () => null) });
    await user.type(input(), "کتا{Enter}");
    expect(await screen.findByText("مدخلی با این نوشتار پیدا نشد.")).toBeInTheDocument();
  });
});

describe("favorites and history", () => {
  it("toggles a favorite and lists it in the favorites tab", async () => {
    const { api, user, input } = setup();
    await user.type(input(), "کتا{Enter}");
    const star = await screen.findByRole("button", { name: /نشان کردن/ });
    expect(star).toHaveAttribute("aria-pressed", "false");
    await user.click(star);
    expect(api.toggleFavorite).toHaveBeenCalledWith({ wordNorm: "کتاب", word: "کتاب" });
    await waitFor(() => expect(screen.getByRole("button", { name: /نشان‌شده/ })).toHaveAttribute("aria-pressed", "true"));
    await user.click(screen.getByRole("tab", { name: /علاقه‌مندی‌ها/ }));
    expect(names()).toEqual(["کتاب"]);
    // دوباره بزنیم: برداشته می‌شود
    await user.click(screen.getByRole("button", { name: /نشان‌شده/ }));
    await waitFor(() => expect(options()).toHaveLength(0));
  });

  it("starts with the saved history and can clear it", async () => {
    const { api, user } = setup({}, { history: [{ wordNorm: "کتاب", word: "کتاب", at: 1 }, { wordNorm: "آب", word: "آب", at: 0 }] });
    await user.click(screen.getByRole("tab", { name: /تاریخچه/ }));
    expect(names()).toEqual(["کتاب", "آب"]);
    await user.click(screen.getByRole("button", { name: "پاک کردن همه" }));
    expect(api.clearHistory).toHaveBeenCalled();
    await waitFor(() => expect(options()).toHaveLength(0));
  });

  it("opens an item from the history list with the keyboard", async () => {
    const { user, input } = setup({}, { history: [{ wordNorm: "کتاب", word: "کتاب", at: 1 }, { wordNorm: "آب", word: "آب", at: 0 }] });
    await user.click(screen.getByRole("tab", { name: /تاریخچه/ }));
    await user.click(input());
    await user.keyboard("{ArrowDown}{Enter}");
    await screen.findByRole("heading", { name: "آب", level: 1 });
  });

  it("tells the user when the save fails but keeps the page", async () => {
    const { user, input } = setup({ recordVisit: vi.fn(async () => Promise.reject("disk full")) });
    await user.type(input(), "کتا{Enter}");
    await screen.findByRole("heading", { name: "کتاب", level: 1 });
    expect(await screen.findByText("ذخیرهٔ تاریخچه ممکن نشد.")).toBeInTheDocument();
  });
});

describe("full-text mode", () => {
  it("switches mode, requires two characters, shows highlighted snippets and opens the hit's entry", async () => {
    const { api, user, input } = setup();
    await user.click(screen.getByRole("radio", { name: "متن کامل" }));
    expect(api.saveSettings).toHaveBeenLastCalledWith(expect.objectContaining({ fulltext: true }));

    await user.type(input(), "ا");
    expect(await screen.findByText("حداقل دو حرف بنویسید.")).toBeInTheDocument();
    expect(api.searchFulltext).not.toHaveBeenCalled();

    await user.type(input(), "ب");
    await waitFor(() => expect(options()).toHaveLength(2));
    expect(api.searchFulltext).toHaveBeenCalledWith("اب", 0);
    const marks = document.querySelectorAll(".row-snippet mark");
    expect(marks[0]).toHaveTextContent("اب");

    await user.keyboard("{Enter}");
    await screen.findByRole("heading", { name: "آسیاب", level: 1 });
    expect(api.getEntryGroup).toHaveBeenCalledWith("آسیاب", { focusId: 205, offset: 0 });
  });

  it("is disabled when the database was built without FTS", () => {
    setup({}, { info: { ...bootstrap().info!, hasFts: false } });
    expect(screen.getByRole("radio", { name: "متن کامل" })).toBeDisabled();
  });

  it("restores the saved mode", async () => {
    const { user, input } = setup({}, { settings: { ...DEFAULT_SETTINGS, fulltext: true } });
    expect(screen.getByRole("radio", { name: "متن کامل" })).toHaveAttribute("aria-checked", "true");
    await user.type(input(), "خط");
    await waitFor(() => expect(options()).toHaveLength(2));
  });

  it("ignores 'cancelled' errors from the core silently", async () => {
    const { user, input } = setup({ searchFulltext: vi.fn(async () => Promise.reject("cancelled")) }, { settings: { ...DEFAULT_SETTINGS, fulltext: true } });
    await user.type(input(), "خط");
    await new Promise((r) => setTimeout(r, 40));
    expect(screen.queryByRole("status")?.textContent ?? "").not.toContain("cancelled");
  });
});

describe("appearance", () => {
  it("changes and saves the theme, and follows the system for 'auto'", async () => {
    const { api, user } = setup();
    await user.click(screen.getByRole("button", { name: "ظاهر و اندازهٔ متن" }));
    const menu = screen.getByRole("dialog", { name: "ظاهر و اندازهٔ متن" });
    await user.click(within(menu).getByRole("radio", { name: "تاریک" }));
    expect(document.documentElement).toHaveAttribute("data-theme", "dark");
    expect(api.saveSettings).toHaveBeenLastCalledWith(expect.objectContaining({ theme: "dark" }));
    expect(api.setWindowTheme).toHaveBeenLastCalledWith("dark");
    await user.click(within(menu).getByRole("radio", { name: "روشن" }));
    expect(document.documentElement).toHaveAttribute("data-theme", "light");
    await user.click(within(menu).getByRole("radio", { name: "خودکار" }));
    expect(document.documentElement).not.toHaveAttribute("data-theme");
    expect(api.setWindowTheme).toHaveBeenLastCalledWith("system");
  });

  it("scales the reading text with the buttons and Ctrl +/-/0, within limits", async () => {
    const { api, user } = setup();
    const scale = () => (document.querySelector(".app") as HTMLElement).style.getPropertyValue("--reading-scale");
    expect(scale()).toBe("1");
    await user.keyboard("{Control>}={/Control}");
    expect(scale()).toBe("1.1");
    await user.keyboard("{Control>}={/Control}{Control>}={/Control}");
    expect(scale()).toBe("1.3");
    await user.keyboard("{Control>}-{/Control}");
    expect(scale()).toBe("1.2");
    await user.keyboard("{Control>}0{/Control}");
    expect(scale()).toBe("1");
    for (let i = 0; i < 15; i++) await user.keyboard("{Control>}={/Control}");
    expect(scale()).toBe("2");
    expect(api.saveSettings).toHaveBeenLastCalledWith(expect.objectContaining({ fontScale: 2 }));
    for (let i = 0; i < 20; i++) await user.keyboard("{Control>}-{/Control}");
    expect(scale()).toBe("0.8");
  });

  it("applies a saved dark theme on start", async () => {
    setup({}, { settings: { ...DEFAULT_SETTINGS, theme: "dark" } });
    await waitFor(() => expect(document.documentElement).toHaveAttribute("data-theme", "dark"));
  });

  it("keeps the menu keyboard-closable", async () => {
    const { user } = setup();
    await user.click(screen.getByRole("button", { name: "ظاهر و اندازهٔ متن" }));
    expect(screen.getByRole("dialog", { name: "ظاهر و اندازهٔ متن" })).toBeInTheDocument();
    await user.keyboard("{Escape}");
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
  });
});

describe("sync dialog", () => {
  const TITLE = "همگام‌سازی علاقه‌مندی‌ها و تاریخچه";

  it("opens from the toolbar and Esc closes it before anything else", async () => {
    const { user, input } = setup();
    await user.type(input(), "آب");
    await user.click(screen.getByRole("button", { name: "همگام‌سازی" }));
    expect(screen.getByRole("dialog", { name: TITLE })).toBeInTheDocument();
    await user.keyboard("{Escape}");
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    expect(input()).toHaveValue("آب"); // هنوز پاک نشده
  });

  it("an import refreshes the sidebar lists and their counters", async () => {
    const favorites = [{ wordNorm: "آب", word: "آب", at: 1 }, { wordNorm: "کتاب", word: "کتاب", at: 2 }];
    const history = [{ wordNorm: "آباد", word: "آباد", at: 3 }];
    const { user } = setup({
      getSyncInfo: vi.fn(async () => ({ dir: "/d", deviceName: "PC", fileName: "f.json", autoExport: true, lastAuto: null })),
      syncImport: vi.fn(async () => ({
        dir: "/d", filesRead: 1, skipped: [], favoritesAdded: 2, favoritesRemoved: 0, historyAdded: 1, historyRemoved: 0, favorites, history,
      })),
    });
    await user.click(screen.getByRole("button", { name: "همگام‌سازی" }));
    await screen.findByText("/d");
    await user.click(screen.getByRole("button", { name: "ورودی گرفتن" }));
    expect(await screen.findByText(/ورودی گرفته شد/)).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "بستن" }));
    expect(screen.getByRole("tab", { name: /علاقه‌مندی‌ها.*۲/ })).toBeInTheDocument();
    expect(screen.getByRole("tab", { name: /تاریخچه.*۱/ })).toBeInTheDocument();
    await user.click(screen.getByRole("tab", { name: /علاقه‌مندی‌ها/ }));
    expect(names()).toEqual(["آب", "کتاب"]);
  });
});

describe("about and errors", () => {
  it("shows the attribution, database facts and shortcuts", async () => {
    const { user } = setup();
    await user.click(screen.getByRole("button", { name: "درباره" }));
    const dlg = screen.getByRole("dialog", { name: "دربارهٔ برنامه" });
    expect(within(dlg).getByText("لغت‌نامهٔ دهخدا، مؤسسهٔ لغت‌نامهٔ دهخدا، دانشگاه تهران")).toBeInTheDocument();
    expect(within(dlg).getByText(/۳۱۲٬۵۰۷/)).toBeInTheDocument();
    expect(within(dlg).getByText(/185ed68/)).toBeInTheDocument();
    expect(within(dlg).getByText(/Vazirmatn/)).toBeInTheDocument();
    expect(within(dlg).getByText("Ctrl + K")).toBeInTheDocument();
    expect(within(dlg).getByText(/هیچ تماس شبکه‌ای/)).toBeInTheDocument();
  });

  it("shows a helpful screen when the database cannot be opened", () => {
    const api = makeApi();
    render(<App api={api} boot={bootstrap({ info: null, dbError: "فایل پایگاه داده پیدا نشد: /x/dehkhoda.sqlite" })} options={FAST} />);
    expect(screen.getByRole("alert")).toHaveTextContent("پایگاه دادهٔ لغت‌نامه آماده نیست");
    expect(screen.getByText(/dehkhoda.sqlite/)).toBeInTheDocument();
    expect(screen.getByText("python tools/build_db.py")).toBeInTheDocument();
    expect(screen.queryByRole("combobox")).not.toBeInTheDocument();
  });

  it("surfaces warnings from the user-data store (corrupt file) once at start", async () => {
    setup({}, { warnings: ["فایل history.json خراب بود"] });
    expect(await screen.findByText("فایل history.json خراب بود")).toBeInTheDocument();
  });

  it("shows search errors without breaking the list", async () => {
    const { user, input } = setup({ searchWords: vi.fn(async () => Promise.reject(new Error("خطای پایگاه"))) });
    await user.type(input(), "آب");
    // روی رایانهٔ کند دو جست‌وجو پشت‌سرهم اجرا می‌شود و با شروع دومی پیام اولی لحظه‌ای برداشته می‌شود؛
    // پس یک‌به‌یک منتظر پیام‌ها نمی‌مانیم، منتظر لحظه‌ای می‌مانیم که هر دو با هم در صفحه‌اند.
    await waitFor(() => {
      expect(screen.getByText("خطای پایگاه")).toBeInTheDocument();
      expect(screen.getByText("جست‌وجو ناموفق بود.")).toBeInTheDocument(); // پس از آخرین جست‌وجو
    });
  });
});
