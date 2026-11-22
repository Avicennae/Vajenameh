import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import { makeApi } from "../test/fakeApi";
import type { SyncExportReport, SyncImportReport, SyncInfo } from "../types";
import { SyncDialog } from "./SyncDialog";

const TITLE = "همگام‌سازی علاقه‌مندی‌ها و تاریخچه";
const info = (over: Partial<SyncInfo> = {}): SyncInfo => ({ dir: "/d", deviceName: "PC-A", fileName: "f.json", autoExport: true, lastAuto: null, ...over });
const withDir = { getSyncInfo: vi.fn(async () => info()) };

function setup(over: Parameters<typeof makeApi>[0] = {}, open = true) {
  const api = makeApi(over);
  const onClose = vi.fn();
  const onImported = vi.fn();
  const user = userEvent.setup();
  render(<SyncDialog open={open} onClose={onClose} api={api} onImported={onImported} />);
  return { api, user, onClose, onImported };
}

const btn = (name: string) => screen.getByRole("button", { name });

const report = (over: Partial<SyncImportReport> = {}): SyncImportReport => ({
  dir: "/d", filesRead: 1, skipped: [], favoritesAdded: 0, favoritesRemoved: 0, historyAdded: 0, historyRemoved: 0, favorites: [], history: [], ...over,
});

describe("SyncDialog", () => {
  it("renders nothing while closed", () => {
    setup({}, false);
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
  });

  it("starts without a folder: only choosing one is possible, and the choice is remembered", async () => {
    const { api, user } = setup();
    expect(screen.getByRole("dialog", { name: TITLE })).toBeInTheDocument();
    expect(await screen.findByText("هنوز پوشه‌ای انتخاب نشده است")).toBeInTheDocument();
    expect(btn("خروجی گرفتن")).toBeDisabled();
    expect(btn("ورودی گرفتن")).toBeDisabled();
    expect(btn("انتخاب پوشه…")).toHaveFocus();

    await user.click(btn("انتخاب پوشه…"));
    expect(api.setSyncDir).toHaveBeenCalledWith("/home/me/Drive/Dehkhoda");
    expect(await screen.findByText("/home/me/Drive/Dehkhoda")).toBeInTheDocument();
    expect(btn("خروجی گرفتن")).toBeEnabled();
    expect(btn("ورودی گرفتن")).toBeEnabled();
    expect(btn("تغییر پوشه…")).toBeInTheDocument();
    expect(screen.getByText("dehkhoda-sync-PC-A-1a2b3c4d.json")).toBeInTheDocument(); // نام فایل این رایانه
  });

  it("keeps things as they were when the folder picker is cancelled", async () => {
    const { api, user } = setup({ pickSyncFolder: vi.fn(async () => null) });
    await user.click(btn("انتخاب پوشه…"));
    expect(api.pickSyncFolder).toHaveBeenCalledTimes(1);
    expect(api.setSyncDir).not.toHaveBeenCalled();
    expect(btn("خروجی گرفتن")).toBeDisabled();
    expect(document.querySelector(".sync-result")).toBeNull(); // نه پیام موفقیت، نه خطا
  });

  it("shows the remembered folder when opened", async () => {
    setup(withDir);
    expect(await screen.findByText("/d")).toBeInTheDocument();
    expect(btn("خروجی گرفتن")).toBeEnabled();
  });

  it("exports and reports the counts and the file name", async () => {
    const { api, user } = setup({
      ...withDir,
      syncExport: vi.fn(async (): Promise<SyncExportReport> => ({ dir: "/d", file: "dehkhoda-sync-PC-A-1a2b3c4d.json", favorites: 12, history: 300 })),
    });
    await screen.findByText("/d");
    await user.click(btn("خروجی گرفتن"));
    expect(api.syncExport).toHaveBeenCalledTimes(1);
    expect(await screen.findByText(/۱۲ علاقه‌مندی و ۳۰۰ واژه از تاریخچه/)).toBeInTheDocument();
    expect(screen.getByText("dehkhoda-sync-PC-A-1a2b3c4d.json")).toBeInTheDocument();
  });

  it("imports, hands the merged lists to the app and summarizes what changed", async () => {
    const favorites = [{ wordNorm: "آب", word: "آب", at: 5 }];
    const history = [{ wordNorm: "کتاب", word: "کتاب", at: 6 }];
    const { user, onImported } = setup({
      ...withDir,
      syncImport: vi.fn(async () => report({ filesRead: 2, favoritesAdded: 3, favoritesRemoved: 1, historyAdded: 12, favorites, history })),
    });
    await screen.findByText("/d");
    await user.click(btn("ورودی گرفتن"));
    expect(await screen.findByText(/ورودی گرفته شد \(۲ فایل/)).toBeInTheDocument();
    expect(screen.getByText("علاقه‌مندی‌ها: ۳ افزوده، ۱ برداشته‌شده")).toBeInTheDocument();
    expect(screen.getByText("تاریخچه: ۱۲ واژهٔ تازه")).toBeInTheDocument();
    expect(onImported).toHaveBeenCalledWith({ history, favorites });
  });

  it("explains when other computers' files are missing or nothing changed", async () => {
    let next = report({ filesRead: 0 });
    const { user } = setup({ ...withDir, syncImport: vi.fn(async () => next) });
    await screen.findByText("/d");
    await user.click(btn("ورودی گرفتن"));
    expect(await screen.findByText(/فایلی از رایانه‌های دیگر در این پوشه پیدا نشد/)).toBeInTheDocument();
    next = report({ filesRead: 1 });
    await user.click(btn("ورودی گرفتن"));
    expect(await screen.findByText(/همه‌چیز از قبل یکسان بود/)).toBeInTheDocument();
    expect(screen.queryByText(/پیدا نشد/)).not.toBeInTheDocument(); // پیام قبلی عوض شده است
  });

  it("lists the files that could not be read, each with its reason", async () => {
    const { user } = setup({
      ...withDir,
      syncImport: vi.fn(async () => report({ filesRead: 1, skipped: [{ file: "dehkhoda-sync-x.json", reason: "فایل خراب یا ناشناخته است" }] })),
    });
    await screen.findByText("/d");
    await user.click(btn("ورودی گرفتن"));
    expect(await screen.findByText("۱ فایل خوانده نشد:")).toBeInTheDocument();
    expect(screen.getByText("dehkhoda-sync-x.json")).toBeInTheDocument();
    expect(screen.getByText(/فایل خراب یا ناشناخته است/)).toBeInTheDocument();
  });

  it("shows errors from the core instead of failing silently, and unlocks the buttons again", async () => {
    const { user } = setup({ ...withDir, syncExport: vi.fn(async () => Promise.reject("پوشه پیدا نشد: /d")) });
    await screen.findByText("/d");
    await user.click(btn("خروجی گرفتن"));
    expect(await screen.findByText("پوشه پیدا نشد: /d")).toBeInTheDocument();
    expect(btn("خروجی گرفتن")).toBeEnabled();
  });

  it("shows an error when the state cannot even be loaded (e.g. outside Tauri)", async () => {
    setup({ getSyncInfo: vi.fn(async () => Promise.reject("این صفحه بیرون از پوستهٔ Tauri باز شده است")) });
    expect(await screen.findByText(/بیرون از پوستهٔ Tauri/)).toBeInTheDocument();
    expect(btn("خروجی گرفتن")).toBeDisabled();
  });

  it("runs one job at a time", async () => {
    let release: (r: SyncExportReport) => void = () => {};
    const pending = new Promise<SyncExportReport>((r) => (release = r));
    const { api, user } = setup({ ...withDir, syncExport: vi.fn(() => pending) });
    await screen.findByText("/d");
    await user.click(btn("خروجی گرفتن"));
    expect(btn("خروجی گرفتن")).toBeDisabled();
    expect(btn("ورودی گرفتن")).toBeDisabled();
    expect(btn("تغییر پوشه…")).toBeDisabled();
    expect(screen.getByText("در حال انجام…")).toBeInTheDocument();
    await user.click(btn("خروجی گرفتن"));
    expect(api.syncExport).toHaveBeenCalledTimes(1);
    release({ dir: "/d", file: "f.json", favorites: 1, history: 0 });
    expect(await screen.findByText(/۱ علاقه‌مندی و ۰ واژه/)).toBeInTheDocument();
    expect(btn("خروجی گرفتن")).toBeEnabled();
  });

  describe("auto-save", () => {
    const box = () => screen.getByRole("checkbox", { name: "ذخیرهٔ خودکار در پوشه پس از هر تغییر" });

    it("is on by default, needs a folder, and can be switched off and on", async () => {
      const { api, user } = setup();
      await screen.findByText("هنوز پوشه‌ای انتخاب نشده است");
      expect(box()).toBeChecked();
      expect(box()).toBeDisabled(); // بدون پوشه
      await user.click(screen.getByRole("button", { name: "انتخاب پوشه…" }));
      await screen.findByText("/home/me/Drive/Dehkhoda");
      expect(box()).toBeEnabled();
      await user.click(box());
      expect(api.setAutoExport).toHaveBeenLastCalledWith(false);
      await vi.waitFor(() => expect(box()).not.toBeChecked());
      expect(screen.queryByText(/ذخیرهٔ خودکار انجام نشد|آخرین ذخیرهٔ خودکار|با اولین تغییر/)).not.toBeInTheDocument(); // خاموش: وضعیتی نشان داده نمی‌شود
      await user.click(box());
      expect(api.setAutoExport).toHaveBeenLastCalledWith(true);
      await vi.waitFor(() => expect(box()).toBeChecked());
    });

    it("shows the time of the last automatic save", async () => {
      setup({ getSyncInfo: vi.fn(async () => info({ lastAuto: { at: Date.UTC(2026, 0, 1, 10, 30), error: null } })) });
      expect(await screen.findByText(/آخرین ذخیرهٔ خودکار:/)).toBeInTheDocument();
    });

    it("shows why the last automatic save failed", async () => {
      setup({ getSyncInfo: vi.fn(async () => info({ lastAuto: { at: 1, error: "پوشه پیدا نشد: /d" } })) });
      const line = await screen.findByText("ذخیرهٔ خودکار انجام نشد: پوشه پیدا نشد: /d");
      expect(line).toHaveClass("error");
    });

    it("keeps refreshing the status until the first automatic save has happened", async () => {
      const calls = vi.fn();
      const getSyncInfo = vi.fn(async () => {
        calls();
        return info(calls.mock.calls.length >= 3 ? { lastAuto: { at: Date.now(), error: null } } : {});
      });
      const api = makeApi({ getSyncInfo });
      render(<SyncDialog open onClose={vi.fn()} api={api} onImported={vi.fn()} pollMs={15} />);
      expect(await screen.findByText(/آخرین ذخیرهٔ خودکار:/)).toBeInTheDocument();
      expect(getSyncInfo.mock.calls.length).toBeGreaterThanOrEqual(3);
      const n = getSyncInfo.mock.calls.length;
      await new Promise((r) => setTimeout(r, 80)); // وضعیت آمده؛ دیگر پرسیدن لازم نیست
      expect(getSyncInfo.mock.calls.length).toBe(n);
    });
  });

  it("closes with the × button and by clicking the backdrop", async () => {
    const { user, onClose } = setup();
    await user.click(btn("بستن"));
    expect(onClose).toHaveBeenCalledTimes(1);
    await user.click(screen.getByRole("dialog").parentElement as HTMLElement);
    expect(onClose).toHaveBeenCalledTimes(2);
  });
});
