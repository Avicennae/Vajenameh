/// <reference types="vite/client" />

interface ImportMetaEnv {
  /** با VITE_MOCK=1 رابط بدون Tauri و با داده‌های نمونه اجرا می‌شود (برای توسعه و تست مرورگری). */
  readonly VITE_MOCK?: string;
}
declare const __APP_VERSION__: string;
