import { faNumber } from "../lib/format";
import type { ListItem, Mode, SearchStatus, Tab } from "../hooks/useDictionary";
import { VirtualList } from "./VirtualList";

interface Props {
  listId: string;
  tab: Tab;
  onTab: (tab: Tab) => void;
  historyCount: number;
  favoritesCount: number;
  mode: Mode;
  status: SearchStatus;
  hasMore: boolean;
  items: readonly ListItem[];
  active: number;
  onPick: (index: number) => void;
  onNearEnd: () => void;
  onClearHistory: () => void;
}

const TABS: { id: Tab; label: string }[] = [
  { id: "results", label: "نتایج" },
  { id: "history", label: "تاریخچه" },
  { id: "favorites", label: "علاقه‌مندی‌ها" },
];

function statusText(tab: Tab, status: SearchStatus, count: number, hasMore: boolean): string {
  if (tab === "history") return count ? "آخرین واژه‌هایی که دیده‌اید" : "هنوز واژه‌ای نگشوده‌اید.";
  if (tab === "favorites") return count ? "واژه‌های نشان‌شده" : "با ستارهٔ بالای هر مدخل، آن را اینجا نگه دارید.";
  switch (status) {
    case "idle": return "برای شروع، واژه‌ای بنویسید.";
    case "short": return "حداقل دو حرف بنویسید.";
    case "error": return "جست‌وجو ناموفق بود.";
    case "loading": return "در حال جست‌وجو…";
    case "done": return count ? `${faNumber(count)}${hasMore ? "+" : ""} نتیجه` : "نتیجه‌ای پیدا نشد.";
  }
}

export function Sidebar(p: Props) {
  const fts = p.tab === "results" && p.mode === "fulltext";
  return (
    <aside className="side" aria-label="فهرست">
      <div className="tabs" role="tablist" aria-label="بخش‌ها">
        {TABS.map((t) => {
          const n = t.id === "history" ? p.historyCount : t.id === "favorites" ? p.favoritesCount : 0;
          return (
            <button key={t.id} type="button" role="tab" aria-selected={p.tab === t.id} className={p.tab === t.id ? "tab on" : "tab"} onClick={() => p.onTab(t.id)}>
              {t.label}
              {n > 0 && <span className="tab-count">{faNumber(n)}</span>}
            </button>
          );
        })}
      </div>
      <div className="side-status" aria-live="polite">
        <span>{statusText(p.tab, p.status, p.items.length, p.hasMore)}</span>
        {p.tab === "history" && p.historyCount > 0 && (
          <button type="button" className="link-btn" onClick={p.onClearHistory}>
            پاک کردن همه
          </button>
        )}
      </div>
      <VirtualList
        id={p.listId}
        label={p.tab === "results" ? "نتایج جست‌وجو" : p.tab === "history" ? "تاریخچه" : "علاقه‌مندی‌ها"}
        items={p.items}
        activeIndex={p.active}
        rowHeight={fts ? 92 : 44}
        getKey={(it) => it.key}
        onPick={p.onPick}
        onNearEnd={fts ? p.onNearEnd : undefined}
        renderRow={(it) =>
          fts ? (
            <div className="row row-fts">
              <span className="row-word">{it.word}</span>
              <span className="row-snippet">
                {it.snippet?.map((s, i) => (s.hit ? <mark key={i}>{s.text}</mark> : <span key={i}>{s.text}</span>))}
              </span>
            </div>
          ) : (
            <div className="row">
              <span className="row-word">{it.word}</span>
              {it.count !== undefined && it.count > 1 && (
                <span className="row-count" title={`${faNumber(it.count)} مدخل هم‌املا`}>
                  {faNumber(it.count)}
                </span>
              )}
            </div>
          )
        }
      />
    </aside>
  );
}
