import { useEffect, useRef } from "react";
import { faNumber } from "../lib/format";
import type { Settings, Theme } from "../types";
import { MAX_FONT_SCALE, MIN_FONT_SCALE } from "../types";

interface Props {
  settings: Settings;
  open: boolean;
  onOpenChange: (open: boolean) => void;
  onTheme: (theme: Theme) => void;
  onFont: (dir: 1 | -1 | 0) => void;
}

const THEMES: { value: Theme; label: string }[] = [
  { value: "system", label: "خودکار" },
  { value: "light", label: "روشن" },
  { value: "dark", label: "تاریک" },
];

/** منوی کوچک ظاهر: تم و اندازهٔ متن */
export function SettingsMenu({ settings, open, onOpenChange, onTheme, onFont }: Props) {
  const rootRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!open) return;
    const onDown = (e: MouseEvent) => {
      if (rootRef.current && !rootRef.current.contains(e.target as Node)) onOpenChange(false);
    };
    document.addEventListener("mousedown", onDown);
    return () => document.removeEventListener("mousedown", onDown);
  }, [open, onOpenChange]);

  return (
    <div className="menu-wrap" ref={rootRef}>
      <button
        type="button"
        className="tool-btn"
        aria-haspopup="dialog"
        aria-expanded={open}
        title="ظاهر و اندازهٔ متن"
        onClick={() => onOpenChange(!open)}
      >
        <span aria-hidden="true">Aa</span>
        <span className="sr-only">ظاهر و اندازهٔ متن</span>
      </button>
      {open && (
        <div className="menu" role="dialog" aria-label="ظاهر و اندازهٔ متن">
          <div className="menu-row">
            <span className="menu-label">تم</span>
            <div className="seg" role="radiogroup" aria-label="تم">
              {THEMES.map((t) => (
                <button key={t.value} type="button" role="radio" aria-checked={settings.theme === t.value} className={settings.theme === t.value ? "on" : ""} onClick={() => onTheme(t.value)}>
                  {t.label}
                </button>
              ))}
            </div>
          </div>
          <div className="menu-row">
            <span className="menu-label">اندازهٔ متن</span>
            <div className="stepper">
              <button type="button" className="icon-btn" aria-label="کوچک‌تر" title="کوچک‌تر (Ctrl + −)" disabled={settings.fontScale <= MIN_FONT_SCALE} onClick={() => onFont(-1)}>
                −
              </button>
              <button type="button" className="stepper-value" title="بازنشانی (Ctrl + 0)" onClick={() => onFont(0)}>
                {faNumber(Math.round(settings.fontScale * 100))}٪
              </button>
              <button type="button" className="icon-btn" aria-label="بزرگ‌تر" title="بزرگ‌تر (Ctrl + +)" disabled={settings.fontScale >= MAX_FONT_SCALE} onClick={() => onFont(1)}>
                +
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
