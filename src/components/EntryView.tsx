import { useEffect, useRef } from "react";
import { faNumber } from "../lib/format";
import type { DbInfo, EntryGroup } from "../types";

interface Props {
  group: EntryGroup | null;
  loading: boolean;
  /** مدخلی که باید به آن اسکرول شود (از نتیجهٔ متن‌کامل) */
  focusEntryId: number | null;
  isFavorite: boolean;
  onToggleFavorite: () => void;
  onCopy: () => void;
  onGoto: (offset: number) => void;
  info: DbInfo | null;
}

function Pager({ group, onGoto, position }: { group: EntryGroup; onGoto: (offset: number) => void; position: "top" | "bottom" }) {
  if (group.total <= group.pageSize) return null;
  const from = group.offset + 1;
  const to = group.offset + group.entries.length;
  const hasPrev = group.offset > 0;
  const hasNext = to < group.total;
  return (
    <nav className={`pager pager-${position}`} aria-label="صفحه‌های مدخل‌ها">
      <button type="button" disabled={!hasPrev} onClick={() => onGoto(Math.max(0, group.offset - group.pageSize))}>
        مدخل‌های قبلی
      </button>
      <span className="pager-info">
        مدخل‌های {faNumber(from)} تا {faNumber(to)} از {faNumber(group.total)}
      </span>
      <button type="button" disabled={!hasNext} onClick={() => onGoto(group.offset + group.pageSize)}>
        مدخل‌های بعدی
      </button>
    </nav>
  );
}

export function EntryView({ group, loading, focusEntryId, isFavorite, onToggleFavorite, onCopy, onGoto, info }: Props) {
  const rootRef = useRef<HTMLElement>(null);

  // گروه یا صفحهٔ تازه ⇒ از بالا؛ نتیجهٔ متن‌کامل ⇒ به همان مدخل
  const wordNorm = group?.wordNorm;
  const offset = group?.offset;
  useEffect(() => {
    const scroller = rootRef.current?.parentElement;
    if (focusEntryId !== null) {
      const el = document.getElementById(`entry-${focusEntryId}`);
      if (el) {
        el.scrollIntoView?.({ block: "start" });
        return;
      }
    }
    scroller?.scrollTo?.({ top: 0 });
  }, [wordNorm, offset, focusEntryId]);

  if (!group) {
    return (
      <section ref={rootRef} className="welcome" aria-label="خوش آمدید">
        <h1>لغت‌نامهٔ دهخدا</h1>
        <p>واژه‌ای را جست‌وجو کنید؛ نتایج هم‌زمان با تایپ می‌آیند.</p>
        <p className="muted">
          <kbd>Ctrl</kbd> + <kbd>K</kbd> برای رفتن به جست‌وجو، <kbd>↑</kbd> <kbd>↓</kbd> برای انتخاب و <kbd>Enter</kbd> برای باز کردن.
        </p>
        {info && (
          <p className="muted">
            {faNumber(info.rowCount)} مدخل · کاملاً آفلاین
          </p>
        )}
      </section>
    );
  }

  return (
    <article ref={rootRef} className={loading ? "entry-view loading" : "entry-view"} aria-busy={loading} aria-label={`مدخل ${group.word}`}>
      <header className="entry-head">
        <h1 className="headword">{group.word}</h1>
        {group.total > 1 && <span className="badge">{faNumber(group.total)} مدخل</span>}
        <div className="entry-actions">
          <button type="button" className={isFavorite ? "act on" : "act"} aria-pressed={isFavorite} title={isFavorite ? "برداشتن از علاقه‌مندی‌ها" : "افزودن به علاقه‌مندی‌ها"} onClick={onToggleFavorite}>
            <span aria-hidden="true">{isFavorite ? "★" : "☆"}</span> {isFavorite ? "نشان‌شده" : "نشان کردن"}
          </button>
          <button type="button" className="act" title="کپی متن معنی‌ها (این صفحه)" onClick={onCopy}>
            کپی
          </button>
        </div>
      </header>
      <Pager group={group} onGoto={onGoto} position="top" />
      {group.entries.map((e, i) => (
        <section key={e.id} id={`entry-${e.id}`} className={e.id === focusEntryId ? "entry focus" : "entry"}>
          {group.total > 1 && (
            <div className="entry-label">
              مدخل {faNumber(group.offset + i + 1)}
              {e.word !== group.word && <span className="entry-word"> · {e.word}</span>}
            </div>
          )}
          <div className="entry-body" dangerouslySetInnerHTML={{ __html: e.html }} />
        </section>
      ))}
      <Pager group={group} onGoto={onGoto} position="bottom" />
    </article>
  );
}
