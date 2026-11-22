import { useVirtualizer } from "@tanstack/react-virtual";
import { useEffect, useRef, type ReactNode } from "react";

interface Props<T> {
  id: string;
  label: string;
  items: readonly T[];
  activeIndex: number;
  rowHeight: number;
  getKey: (item: T) => string;
  renderRow: (item: T, index: number) => ReactNode;
  onPick: (index: number) => void;
  /** وقتی نزدیک انتهای فهرست رسیدیم (برای بارگذاری صفحهٔ بعد) */
  onNearEnd?: () => void;
}

/** شناسهٔ گزینهٔ index‌ام؛ برای aria-activedescendant در فیلد جست‌وجو */
export const optionId = (listId: string, index: number) => `${listId}-opt-${index}`;

/** فهرست مجازی‌شده (فقط ردیف‌های دیدنی ساخته می‌شوند) با فوکوس منطقی روی فیلد جست‌وجو */
export function VirtualList<T>({ id, label, items, activeIndex, rowHeight, getKey, renderRow, onPick, onNearEnd }: Props<T>) {
  const parentRef = useRef<HTMLDivElement>(null);
  const virt = useVirtualizer({
    count: items.length,
    getScrollElement: () => parentRef.current,
    estimateSize: () => rowHeight,
    getItemKey: (i) => getKey(items[i]),
    overscan: 8,
  });

  useEffect(() => {
    if (activeIndex >= 0 && activeIndex < items.length) virt.scrollToIndex(activeIndex, { align: "auto" });
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [activeIndex, items.length]);

  const rows = virt.getVirtualItems();
  const lastVisible = rows.length ? rows[rows.length - 1].index : -1;
  useEffect(() => {
    if (onNearEnd && items.length > 0 && lastVisible >= items.length - 6) onNearEnd();
  }, [lastVisible, items.length, onNearEnd]);

  return (
    <div ref={parentRef} className="vlist" role="listbox" id={id} aria-label={label}>
      <div style={{ height: virt.getTotalSize(), position: "relative" }}>
        {rows.map((v) => (
          <div
            key={v.key}
            id={optionId(id, v.index)}
            role="option"
            aria-selected={v.index === activeIndex}
            className={v.index === activeIndex ? "vrow active" : "vrow"}
            style={{ position: "absolute", top: 0, left: 0, right: 0, height: v.size, transform: `translateY(${v.start}px)` }}
            onMouseDown={(e) => e.preventDefault() /* فوکوس در فیلد جست‌وجو بماند */}
            onClick={() => onPick(v.index)}
          >
            {renderRow(items[v.index], v.index)}
          </div>
        ))}
      </div>
    </div>
  );
}
