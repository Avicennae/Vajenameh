/** اندیس را در بازهٔ فهرست نگه می‌دارد؛ فهرست خالی ⇒ ‎-۱ */
export function clampIndex(index: number, length: number): number {
  if (length <= 0) return -1;
  return Math.min(Math.max(index, 0), length - 1);
}

/** حرکت با فلش‌ها (بدون چرخش): از بالا/پایین فهرست بیرون نمی‌رود */
export function moveIndex(current: number, delta: number, length: number): number {
  if (length <= 0) return -1;
  if (current < 0) return delta >= 0 ? 0 : length - 1;
  return clampIndex(current + delta, length);
}
