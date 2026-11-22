import { useEffect } from "react";

interface Props {
  notice: { id: number; text: string } | null;
  onDismiss: () => void;
}

/** پیام کوتاه و گذرا؛ خودش بعد از چند ثانیه بسته می‌شود */
export function Toast({ notice, onDismiss }: Props) {
  const id = notice?.id;
  useEffect(() => {
    if (id === undefined) return;
    const t = setTimeout(onDismiss, 3600);
    return () => clearTimeout(t);
  }, [id, onDismiss]);
  return (
    <div className="toast-region" role="status" aria-live="polite">
      {notice && (
        <div className="toast" key={notice.id}>
          {notice.text}
        </div>
      )}
    </div>
  );
}
