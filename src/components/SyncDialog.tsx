import { useEffect, useRef, useState } from "react";
import type { Api } from "../api/api";
import { describeError } from "../api/api";
import { faNumber } from "../lib/format";
import type { SkippedFile, SyncExportReport, SyncImportReport, SyncInfo } from "../types";

interface Props {
  open: boolean;
  onClose: () => void;
  api: Api;
  /** فهرست‌های تازهٔ این رایانه پس از «ورودی گرفتن»؛ برنامه نوار کناری را با آن‌ها به‌روز می‌کند */
  onImported: (lists: Pick<SyncImportReport, "history" | "favorites">) => void;
  /** فاصلهٔ تازه‌کردن وضعیت تا وقتی اولین ذخیرهٔ خودکار انجام نشده (میلی‌ثانیه؛ فقط تست‌ها عوضش می‌کنند) */
  pollMs?: number;
}

interface Message {
  tone: "ok" | "warn" | "error";
  text: string;
  /** جزئیات تغییرها */
  lines?: string[];
  /** نام فایل (چپ‌به‌راست) */
  file?: string;
  skipped?: SkippedFile[];
}

type Job = "pick" | "export" | "import" | "auto";

function describeAuto(info: SyncInfo): { text: string; error: boolean } {
  const last = info.lastAuto;
  if (last?.error) return { text: `ذخیرهٔ خودکار انجام نشد: ${last.error}`, error: true };
  if (last) {
    const time = new Date(last.at).toLocaleTimeString("fa-IR", { hour: "2-digit", minute: "2-digit" });
    return { text: `آخرین ذخیرهٔ خودکار: ${time}`, error: false };
  }
  return { text: "با اولین تغییر (یا چند ثانیه پس از روشن‌کردن) فایل همین رایانه خودکار نوشته می‌شود.", error: false };
}

function describeExport(r: SyncExportReport): Message {
  return {
    tone: "ok",
    text: `خروجی گرفته شد: ${faNumber(r.favorites)} علاقه‌مندی و ${faNumber(r.history)} واژه از تاریخچه ذخیره شد.`,
    file: r.file,
  };
}

function describeImport(r: SyncImportReport): Message {
  const skipped = r.skipped.length ? r.skipped : undefined;
  if (r.filesRead === 0) {
    return {
      tone: "warn",
      text: "فایلی از رایانه‌های دیگر در این پوشه پیدا نشد. اول در رایانهٔ دیگر «خروجی گرفتن» را بزنید و صبر کنید تا سرویس همگام‌سازی فایل را برساند.",
      skipped,
    };
  }
  const lines: string[] = [];
  const fav: string[] = [];
  if (r.favoritesAdded) fav.push(`${faNumber(r.favoritesAdded)} افزوده`);
  if (r.favoritesRemoved) fav.push(`${faNumber(r.favoritesRemoved)} برداشته‌شده`);
  if (fav.length) lines.push(`علاقه‌مندی‌ها: ${fav.join("، ")}`);
  const his: string[] = [];
  if (r.historyAdded) his.push(`${faNumber(r.historyAdded)} واژهٔ تازه`);
  if (r.historyRemoved) his.push(`${faNumber(r.historyRemoved)} مورد پاک‌شده`);
  if (his.length) lines.push(`تاریخچه: ${his.join("، ")}`);
  if (!lines.length) lines.push("همه‌چیز از قبل یکسان بود؛ تغییری لازم نشد.");
  return { tone: skipped ? "warn" : "ok", text: `ورودی گرفته شد (${faNumber(r.filesRead)} فایل از رایانه‌های دیگر).`, lines, skipped };
}

/** همگام‌سازی دستی علاقه‌مندی‌ها و تاریخچه از راه یک پوشه (خروجی/ورودی) */
export function SyncDialog({ open, onClose, api, onImported, pollMs = 2000 }: Props) {
  const [info, setInfo] = useState<SyncInfo | null>(null);
  const [busy, setBusy] = useState<Job | null>(null);
  const [message, setMessage] = useState<Message | null>(null);
  const pickRef = useRef<HTMLButtonElement>(null);

  useEffect(() => {
    if (!open) return;
    let alive = true;
    setMessage(null);
    api
      .getSyncInfo()
      .then((i) => alive && setInfo(i))
      .catch((e) => alive && setMessage({ tone: "error", text: describeError(e) }));
    pickRef.current?.focus();
    return () => {
      alive = false;
    };
  }, [open, api]);

  // تا وقتی اولین ذخیرهٔ خودکار انجام نشده، هر چند ثانیه وضعیت را تازه می‌کنیم تا نتیجه (زمان یا خطا) دیده شود
  const waitingForFirstAuto = open && !!info?.dir && info.autoExport && !info.lastAuto;
  useEffect(() => {
    if (!waitingForFirstAuto) return;
    let alive = true;
    const timer = setTimeout(() => {
      api
        .getSyncInfo()
        .then((i) => alive && setInfo(i))
        .catch(() => undefined);
    }, pollMs);
    return () => {
      alive = false;
      clearTimeout(timer);
    };
  }, [waitingForFirstAuto, info, pollMs, api]);

  const run = async (job: Job, work: () => Promise<Message | null>) => {
    if (busy) return;
    setBusy(job);
    setMessage(null);
    try {
      setMessage(await work());
    } catch (e) {
      setMessage({ tone: "error", text: describeError(e) });
    } finally {
      setBusy(null);
    }
  };

  const pick = () =>
    run("pick", async () => {
      const dir = await api.pickSyncFolder();
      if (dir) setInfo(await api.setSyncDir(dir));
      return null;
    });

  const toggleAuto = (enabled: boolean) =>
    run("auto", async () => {
      setInfo(await api.setAutoExport(enabled));
      return null;
    });

  const doExport = () => run("export", async () => describeExport(await api.syncExport()));

  const doImport = () =>
    run("import", async () => {
      const report = await api.syncImport();
      onImported({ history: report.history, favorites: report.favorites });
      return describeImport(report);
    });

  if (!open) return null;
  const dir = info?.dir ?? null;
  const locked = busy !== null;
  return (
    <div className="backdrop" onMouseDown={(e) => e.target === e.currentTarget && onClose()}>
      <div className="dialog" role="dialog" aria-modal="true" aria-labelledby="sync-title" aria-busy={locked}>
        <div className="dialog-head">
          <h2 id="sync-title">همگام‌سازی علاقه‌مندی‌ها و تاریخچه</h2>
          <button type="button" className="icon-btn" aria-label="بستن" onClick={onClose}>
            ×
          </button>
        </div>
        <div className="dialog-body">
          <p>
            علاقه‌مندی‌ها و تاریخچهٔ این رایانه را می‌شود در یک پوشه نگه داشت و در رایانهٔ دیگر وارد کرد. پوشه‌ای داخل Google Drive، OneDrive یا
            Dropbox انتخاب کنید تا خودِ آن سرویس فایل را به رایانه‌های دیگر برساند؛ خود برنامه همچنان هیچ تماس شبکه‌ای نمی‌گیرد.
          </p>
          <h3>پوشهٔ همگام‌سازی</h3>
          <div className="sync-folder">
            {dir ? (
              <span className="sync-path ltr" dir="ltr" title={dir}>
                {dir}
              </span>
            ) : (
              <span className="sync-path empty">هنوز پوشه‌ای انتخاب نشده است</span>
            )}
            <button ref={pickRef} type="button" className={dir ? "act" : "act primary"} disabled={locked} onClick={() => void pick()}>
              {dir ? "تغییر پوشه…" : "انتخاب پوشه…"}
            </button>
          </div>
          <label className="sync-auto">
            <input type="checkbox" checked={info?.autoExport ?? true} disabled={!dir || locked} onChange={(e) => void toggleAuto(e.target.checked)} />
            <span>ذخیرهٔ خودکار در پوشه پس از هر تغییر</span>
          </label>
          {dir && info?.autoExport && (
            <p className={`sync-auto-status${describeAuto(info).error ? " error" : ""}`}>{describeAuto(info).text}</p>
          )}
          <div className="sync-actions">
            <button type="button" className="act primary" disabled={!dir || locked} onClick={() => void doExport()}>
              خروجی گرفتن
            </button>
            <button type="button" className="act" disabled={!dir || locked} onClick={() => void doImport()}>
              ورودی گرفتن
            </button>
          </div>
          <p className="sync-hint">
            وقتی «ذخیرهٔ خودکار» روشن باشد، هر بار علاقه‌مندی‌ها یا تاریخچه عوض شود (و هنگام بستن برنامه) فایل همین رایانه خودش در پوشه نوشته
            می‌شود. «خروجی گرفتن» برای نوشتن دستی است. «ورودی گرفتن» فایل رایانه‌های دیگر را می‌خواند و با داده‌های اینجا ادغام می‌کند؛ این یکی
            همیشه دستی است. هر رایانه فقط فایل خودش را می‌نویسد، پس فایل‌ها با هم تداخل پیدا نمی‌کنند. برداشتن یک علاقه‌مندی یا پاک کردن تاریخچه هم با «ورودی
            گرفتن» به رایانه‌های دیگر می‌رسد (ساعت رایانه‌ها باید درست باشد).
          </p>
          {info && (
            <p className="sync-hint">
              فایل این رایانه: <span className="ltr">{info.fileName}</span>
            </p>
          )}
          <div aria-live="polite" aria-atomic="true">
            {busy && <p className="sync-busy">در حال انجام…</p>}
            {message && (
              <div className={`sync-result ${message.tone}`}>
                <p>{message.text}</p>
                {message.lines && (
                  <ul>
                    {message.lines.map((line) => (
                      <li key={line}>{line}</li>
                    ))}
                  </ul>
                )}
                {message.file && <p className="sync-file ltr">{message.file}</p>}
                {message.skipped && (
                  <>
                    <p>{faNumber(message.skipped.length)} فایل خوانده نشد:</p>
                    <ul>
                      {message.skipped.map((s) => (
                        <li key={s.file}>
                          <bdi className="ltr">{s.file}</bdi>: {s.reason}
                        </li>
                      ))}
                    </ul>
                  </>
                )}
              </div>
            )}
          </div>
        </div>
      </div>
    </div>
  );
}
