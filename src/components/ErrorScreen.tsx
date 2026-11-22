interface Props {
  message: string;
}

/** وقتی پایگاه داده باز نشد (نبود، خراب یا ناسازگار) */
export function ErrorScreen({ message }: Props) {
  return (
    <main className="fatal" role="alert">
      <h1>پایگاه دادهٔ لغت‌نامه آماده نیست</h1>
      <p className="fatal-msg">{message}</p>
      <p>
        اگر برنامه را خودتان از سورس ساخته‌اید، پایگاه را با این دستور بسازید و دوباره اجرا کنید:
      </p>
      <pre dir="ltr">python tools/build_db.py</pre>
      <p className="muted">اگر برنامه را با نصب‌کننده نصب کرده‌اید و این پیام را می‌بینید، لطفاً دوباره نصب کنید.</p>
    </main>
  );
}
