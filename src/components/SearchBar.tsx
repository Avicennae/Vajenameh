import type { KeyboardEvent, RefObject } from "react";
import type { Mode } from "../hooks/useDictionary";

interface Props {
  value: string;
  onChange: (value: string) => void;
  onClear: () => void;
  mode: Mode;
  onModeChange: (mode: Mode) => void;
  ftsAvailable: boolean;
  inputRef: RefObject<HTMLInputElement | null>;
  /** شناسهٔ فهرست نتایج و گزینهٔ فعال (برای صفات ARIA) */
  listId: string;
  activeOptionId?: string;
  loading: boolean;
  onKeyDown: (e: KeyboardEvent<HTMLInputElement>) => void;
}

export function SearchBar({ value, onChange, onClear, mode, onModeChange, ftsAvailable, inputRef, listId, activeOptionId, loading, onKeyDown }: Props) {
  return (
    <div className="search">
      <div className="search-field">
        <input
          ref={inputRef}
          className="search-input"
          type="text"
          role="combobox"
          aria-label={mode === "word" ? "جست‌وجوی واژه" : "جست‌وجو در متن معنی‌ها"}
          aria-autocomplete="list"
          aria-expanded={value.trim() !== ""}
          aria-controls={listId}
          aria-activedescendant={activeOptionId}
          placeholder={mode === "word" ? "جست‌وجوی واژه…" : "جست‌وجو در متن معنی‌ها… (برای عبارت دقیق، آن را در گیومه بگذارید)"}
          value={value}
          dir="rtl"
          autoFocus
          autoComplete="off"
          autoCorrect="off"
          spellCheck={false}
          onChange={(e) => onChange(e.target.value)}
          onKeyDown={onKeyDown}
        />
        <div className="search-trailing">
          {loading && <span className="spinner" aria-hidden="true" />}
          {value && (
            <button type="button" className="icon-btn clear-btn" aria-label="پاک کردن" title="پاک کردن (Esc)" onClick={onClear}>
              ×
            </button>
          )}
          <kbd className="search-hint" aria-hidden="true">
            Ctrl K
          </kbd>
        </div>
      </div>
      <div className="seg" role="radiogroup" aria-label="نوع جست‌وجو">
        <button type="button" role="radio" aria-checked={mode === "word"} className={mode === "word" ? "on" : ""} onClick={() => onModeChange("word")}>
          واژه
        </button>
        <button
          type="button"
          role="radio"
          aria-checked={mode === "fulltext"}
          className={mode === "fulltext" ? "on" : ""}
          disabled={!ftsAvailable}
          title={ftsAvailable ? "جست‌وجو در متن همهٔ معنی‌ها" : "این پایگاه بدون جست‌وجوی متن‌کامل ساخته شده است"}
          onClick={() => onModeChange("fulltext")}
        >
          متن کامل
        </button>
      </div>
    </div>
  );
}
