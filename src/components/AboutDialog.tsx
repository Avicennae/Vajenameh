import { useEffect, useRef } from "react";
import { faNumber, formatDate, shortCommit } from "../lib/format";
import type { DbInfo } from "../types";

interface Props {
  open: boolean;
  onClose: () => void;
  info: DbInfo | null;
}

const SHORTCUTS: [string, string][] = [
  ["Ctrl + K", "رفتن به جست‌وجو"],
  ["↑  ↓", "انتخاب نتیجه"],
  ["Enter", "باز کردن نتیجهٔ انتخاب‌شده"],
  ["Esc", "برگشت به جست‌وجو؛ دوباره: پاک کردن"],
  ["Ctrl + +", "بزرگ‌تر کردن متن"],
  ["Ctrl + −", "کوچک‌تر کردن متن"],
  ["Ctrl + 0", "اندازهٔ پیش‌فرض متن"],
];

export function AboutDialog({ open, onClose, info }: Props) {
  const closeRef = useRef<HTMLButtonElement>(null);
  useEffect(() => {
    if (open) closeRef.current?.focus();
  }, [open]);
  if (!open) return null;
  return (
    <div className="backdrop" onMouseDown={(e) => e.target === e.currentTarget && onClose()}>
      <div className="dialog" role="dialog" aria-modal="true" aria-labelledby="about-title">
        <div className="dialog-head">
          <h2 id="about-title">دربارهٔ برنامه</h2>
          <button ref={closeRef} type="button" className="icon-btn" aria-label="بستن" onClick={onClose}>
            ×
          </button>
        </div>
        <div className="dialog-body">
          <p className="source">لغت‌نامهٔ دهخدا، مؤسسهٔ لغت‌نامهٔ دهخدا، دانشگاه تهران</p>
          <p>
            نسخهٔ آفلاین برای مطالعه؛ همهٔ جست‌وجوها روی همین رایانه انجام می‌شود و برنامه هیچ تماس شبکه‌ای برقرار نمی‌کند.
            این برنامه یک ابزار مستقل و غیررسمی است و وابسته به مؤسسهٔ لغت‌نامهٔ دهخدا یا دانشگاه تهران نیست.
          </p>
          <dl className="facts">
            <dt>نسخهٔ برنامه</dt>
            <dd dir="ltr" className="ltr">{__APP_VERSION__}</dd>
            {info && (
              <>
                <dt>تعداد مدخل‌ها</dt>
                <dd>{faNumber(info.rowCount)} ({faNumber(info.distinctWords)} سرواژهٔ متمایز)</dd>
                <dt>ساخت پایگاه</dt>
                <dd>{formatDate(info.builtAt)}</dd>
                <dt>منبع داده</dt>
                <dd dir="ltr" className="ltr">
                  nimah79/Dehkhoda-SQL @ {shortCommit(info.sourceCommit)}
                </dd>
              </>
            )}
            <dt>فونت</dt>
            <dd>وزیرمتن (Vazirmatn)، مجوز SIL Open Font License 1.1</dd>
          </dl>
          <h3>میان‌برها</h3>
          <dl className="keys">
            {SHORTCUTS.map(([k, d]) => (
              <div key={k} className="key-row">
                <dt>
                  <kbd>{k}</kbd>
                </dt>
                <dd>{d}</dd>
              </div>
            ))}
          </dl>
        </div>
      </div>
    </div>
  );
}
