//! تاریخچه، علاقه‌مندی‌ها و تنظیمات — در فایل‌های JSON جدا در پوشهٔ دادهٔ کاربر (نه داخل پایگاه اصلی).
//!
//! * نوشتن اتمیک: اول `x.json.tmp`، بعد جایگزینی؛ قطع برق وسط نوشتن فایل نیمه‌کاره نمی‌سازد.
//! * فایل خراب نمی‌ریزد و دور انداخته هم نمی‌شود: به `x.corrupt-<زمان>.json` تغییر نام می‌دهد و با
//!   مقدار پیش‌فرض ادامه می‌دهیم (و یک هشدار برای رابط ثبت می‌شود).
//! * برای همگام‌سازی دستی بین دستگاه‌ها ([`crate::sync`]) هر برداشتنِ علاقه‌مندی یا واژهٔ تاریخچه یک
//!   «گور» ([`Tombstone`]) با زمان خودش به‌جا می‌گذارد و «پاک کردن تاریخچه» یک زمان (`clearedAt`).
//!   هر رویداد تازه زمانی اکیداً جلوتر از هر چیزی می‌گیرد که این دستگاه تا آن لحظه می‌دانسته؛ پس قاعدهٔ
//!   «آخرین تغییر برنده است» حتی با ساعت کمی عقب‌مانده هم درست کار می‌کند. فایل‌های قدیمی (بدون این
//!   فیلدها) همان‌طور خوانده می‌شوند و فایل‌های تازه برای نسخه‌های قدیمی هم خواندنی‌اند.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{de::DeserializeOwned, Deserialize, Serialize};

use crate::sync::{favorites_diff, history_diff, merge_favorites, merge_history, new_device_id, AutoShared, MergeCounts};
use crate::Result;

pub const MAX_HISTORY: usize = 500;
/// سقف «گور»ها (برداشته‌شده‌های اخیر) در هر فهرست؛ قدیمی‌ترها دور ریخته می‌شوند.
pub const MAX_TOMBSTONES: usize = 5_000;
pub const MIN_FONT_SCALE: f32 = 0.8;
pub const MAX_FONT_SCALE: f32 = 2.0;
const FORMAT_VERSION: u32 = 1;

/// زمان فعلی به میلی‌ثانیهٔ یونیکس.
pub fn now_ms() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as i64).unwrap_or(0)
}

fn is_zero(v: &i64) -> bool {
    *v == 0
}

fn no_tombstones(v: &&[Tombstone]) -> bool {
    v.is_empty()
}

/// زمان یک رویداد تازه: اکیداً بعد از `known` (آخرین چیزی که این دستگاه دربارهٔ آن واژه می‌داند)،
/// حتی اگر ساعت سیستم عقب رفته باشد.
fn after(at: i64, known: i64) -> i64 {
    at.max(known.saturating_add(1))
}

/// ارجاع به یک سرواژه: کلید گروه (`word_norm`) و نوشتار اصلی برای نمایش.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct WordRef {
    pub word_norm: String,
    pub word: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct HistoryItem {
    pub word_norm: String,
    pub word: String,
    /// میلی‌ثانیهٔ یونیکس
    pub at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct FavoriteItem {
    pub word_norm: String,
    pub word: String,
    pub at: i64,
}

/// «گور» یک موردِ برداشته‌شده؛ برای اینکه همگام‌سازی بین دستگاه‌ها حذف را هم منتقل کند.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Tombstone {
    pub word_norm: String,
    /// زمان برداشتن (میلی‌ثانیهٔ یونیکس)
    pub at: i64,
}

/// علاقه‌مندی‌ها به‌همراه گورها (جدیدترین اول).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct FavoritesState {
    #[serde(default)]
    pub items: Vec<FavoriteItem>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub removed: Vec<Tombstone>,
}

/// تاریخچه به‌همراه گورها و زمان آخرین «پاک کردن» (جدیدترین اول).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct HistoryState {
    #[serde(default)]
    pub items: Vec<HistoryItem>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub removed: Vec<Tombstone>,
    /// ورودی‌هایی که زمانشان از این عدد جلوتر نباشد پاک‌شده حساب می‌شوند (۰ = هرگز پاک نشده)
    #[serde(default, skip_serializing_if = "is_zero")]
    pub cleared_at: i64,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum Theme {
    #[default]
    System,
    Light,
    Dark,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    pub theme: Theme,
    /// ضریب اندازهٔ متن معنی (۱٫۰ = پیش‌فرض)
    pub font_scale: f32,
    /// آخرین حالت جست‌وجو: false = سرواژه، true = متن‌کامل
    pub fulltext: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Settings { theme: Theme::System, font_scale: 1.0, fulltext: false }
    }
}

impl Settings {
    /// مقدارهای نامعتبر (NaN، خارج از بازه) را درست می‌کند.
    pub fn sanitized(mut self) -> Self {
        self.font_scale = if self.font_scale.is_finite() { self.font_scale.clamp(MIN_FONT_SCALE, MAX_FONT_SCALE) } else { 1.0 };
        self
    }
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct HistoryFile {
    version: u32,
    items: Vec<HistoryItem>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    removed: Vec<Tombstone>,
    #[serde(default, skip_serializing_if = "is_zero")]
    cleared_at: i64,
}

#[derive(Serialize, Deserialize)]
struct FavoritesFile {
    version: u32,
    items: Vec<FavoriteItem>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    removed: Vec<Tombstone>,
}

/// نسخهٔ قرضی برای نوشتن (بدون کپی کردن فهرست)
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct HistoryFileRef<'a> {
    version: u32,
    items: &'a [HistoryItem],
    #[serde(skip_serializing_if = "no_tombstones")]
    removed: &'a [Tombstone],
    #[serde(skip_serializing_if = "is_zero")]
    cleared_at: i64,
}

#[derive(Serialize)]
struct FavoritesFileRef<'a> {
    version: u32,
    items: &'a [FavoriteItem],
    #[serde(skip_serializing_if = "no_tombstones")]
    removed: &'a [Tombstone],
}

#[derive(Serialize, Deserialize)]
struct SettingsFile {
    version: u32,
    #[serde(flatten)]
    settings: Settings,
}

/// تنظیم همگام‌سازی: شناسهٔ این دستگاه و پوشهٔ انتخاب‌شده (فایل `sync.json`).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
struct SyncConfig {
    version: u32,
    device_id: Option<String>,
    dir: Option<String>,
    /// ذخیرهٔ خودکار پس از هر تغییر؛ نبودنِ مقدار = روشن
    auto_export: Option<bool>,
}

struct Inner {
    history: HistoryState,
    favorites: FavoritesState,
    settings: Settings,
    sync: SyncConfig,
}

pub struct UserStore {
    dir: PathBuf,
    inner: Mutex<Inner>,
    warnings: Vec<String>,
    /// نوشتنِ فایل همگام‌سازی (دستی یا خودکار) یکی‌یکی انجام می‌شود تا دو نوشتنِ هم‌زمان فایل موقت را خراب نکنند
    export_lock: Mutex<()>,
    /// هماهنگی با نخِ «ذخیرهٔ خودکار» ([`crate::sync`])
    auto: Arc<AutoShared>,
}

impl Drop for UserStore {
    fn drop(&mut self) {
        self.auto.stop(); // نخِ ذخیرهٔ خودکار (اگر راه افتاده) تمام شود
    }
}

pub(crate) fn write_atomic(path: &Path, bytes: &[u8]) -> Result<()> {
    let mut tmp = path.as_os_str().to_owned();
    tmp.push(".tmp");
    let tmp = PathBuf::from(tmp);
    fs::write(&tmp, bytes)?;
    fs::rename(&tmp, path)?;
    Ok(())
}

/// فایل را می‌خواند؛ نبود فایل = پیش‌فرض بی‌صدا، خراب بودن = کنار گذاشتن + هشدار.
fn load_or_default<T: DeserializeOwned>(path: &Path, warnings: &mut Vec<String>, default: impl FnOnce() -> T) -> T {
    let bytes = match fs::read(path) {
        Ok(b) => b,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return default(),
        Err(e) => {
            warnings.push(format!("خواندن {} ممکن نشد: {e}", path.display()));
            return default();
        }
    };
    match serde_json::from_slice::<T>(&bytes) {
        Ok(v) => v,
        Err(e) => {
            let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("data");
            let backup = path.with_file_name(format!("{stem}.corrupt-{}.json", now_ms()));
            let moved = fs::rename(path, &backup).is_ok();
            warnings.push(format!(
                "فایل {} خراب بود ({e}); {}",
                path.display(),
                if moved { format!("نسخهٔ خراب در {} نگه داشته شد", backup.display()) } else { "با مقدار پیش‌فرض ادامه می‌دهیم".into() }
            ));
            default()
        }
    }
}

fn tombstone_at(list: &[Tombstone], word_norm: &str) -> Option<i64> {
    list.iter().filter(|t| t.word_norm == word_norm).map(|t| t.at).max()
}

/// گور تازه را جلوی فهرست می‌گذارد (گور قبلی همان واژه جایش را می‌دهد) و فهرست را محدود نگه می‌دارد.
fn push_tombstone(list: &mut Vec<Tombstone>, word_norm: String, at: i64) {
    list.retain(|t| t.word_norm != word_norm);
    list.insert(0, Tombstone { word_norm, at });
    list.truncate(MAX_TOMBSTONES);
}

impl UserStore {
    pub fn open(dir: impl Into<PathBuf>) -> Result<Self> {
        let dir = dir.into();
        fs::create_dir_all(&dir)?;
        let mut warnings = Vec::new();
        let history: HistoryFile = load_or_default(&dir.join("history.json"), &mut warnings, || HistoryFile {
            version: FORMAT_VERSION,
            items: vec![],
            removed: vec![],
            cleared_at: 0,
        });
        let favorites: FavoritesFile = load_or_default(&dir.join("favorites.json"), &mut warnings, || FavoritesFile {
            version: FORMAT_VERSION,
            items: vec![],
            removed: vec![],
        });
        let settings: SettingsFile = load_or_default(&dir.join("settings.json"), &mut warnings, || SettingsFile {
            version: FORMAT_VERSION,
            settings: Settings::default(),
        });
        let sync: SyncConfig = load_or_default(&dir.join("sync.json"), &mut warnings, SyncConfig::default);
        let mut history = HistoryState { items: history.items, removed: history.removed, cleared_at: history.cleared_at.max(0) };
        history.items.truncate(MAX_HISTORY);
        history.removed.truncate(MAX_TOMBSTONES);
        let mut favorites = FavoritesState { items: favorites.items, removed: favorites.removed };
        favorites.removed.truncate(MAX_TOMBSTONES);
        let auto = Arc::new(AutoShared::new());
        auto.set_enabled(sync.dir.is_some() && sync.auto_export.unwrap_or(true));
        Ok(UserStore {
            dir,
            inner: Mutex::new(Inner { history, favorites, settings: settings.settings.sanitized(), sync }),
            warnings,
            export_lock: Mutex::new(()),
            auto,
        })
    }

    /// هشدارهای زمان باز کردن (مثلاً فایل خراب)؛ رابط می‌تواند نشان دهد.
    pub fn warnings(&self) -> &[String] {
        &self.warnings
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    fn lock(&self) -> MutexGuard<'_, Inner> {
        self.inner.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn save_history(&self, h: &HistoryState) -> Result<()> {
        let bytes = serde_json::to_vec_pretty(&HistoryFileRef { version: FORMAT_VERSION, items: &h.items, removed: &h.removed, cleared_at: h.cleared_at })?;
        write_atomic(&self.dir.join("history.json"), &bytes)
    }

    fn save_favorites(&self, f: &FavoritesState) -> Result<()> {
        let bytes = serde_json::to_vec_pretty(&FavoritesFileRef { version: FORMAT_VERSION, items: &f.items, removed: &f.removed })?;
        write_atomic(&self.dir.join("favorites.json"), &bytes)
    }

    fn save_sync_config(&self, cfg: &SyncConfig) -> Result<()> {
        let mut cfg = cfg.clone();
        cfg.version = FORMAT_VERSION;
        let bytes = serde_json::to_vec_pretty(&cfg)?;
        write_atomic(&self.dir.join("sync.json"), &bytes)
    }

    // ---- تاریخچه (جدیدترین اول، بدون تکرار) ----
    pub fn history(&self) -> Vec<HistoryItem> {
        self.lock().history.items.clone()
    }

    pub fn record_visit(&self, word: WordRef, at: i64) -> Result<Vec<HistoryItem>> {
        let mut g = self.lock();
        let h = &mut g.history;
        // دیدنِ تازه باید از «پاک کردن» و از برداشتنِ قبلیِ همین واژه جلوتر باشد
        let known = h.cleared_at.max(tombstone_at(&h.removed, &word.word_norm).unwrap_or(0));
        let at = after(at, known);
        h.removed.retain(|t| t.word_norm != word.word_norm);
        h.items.retain(|x| x.word_norm != word.word_norm);
        h.items.insert(0, HistoryItem { word_norm: word.word_norm, word: word.word, at });
        h.items.truncate(MAX_HISTORY);
        self.save_history(&g.history)?;
        self.auto.touch();
        Ok(g.history.items.clone())
    }

    pub fn remove_history(&self, word_norm: &str) -> Result<Vec<HistoryItem>> {
        let mut g = self.lock();
        let h = &mut g.history;
        if let Some(pos) = h.items.iter().position(|x| x.word_norm == word_norm) {
            let live = h.items.remove(pos);
            let at = after(now_ms(), live.at.max(h.cleared_at));
            push_tombstone(&mut h.removed, word_norm.to_string(), at);
            self.save_history(&g.history)?;
            self.auto.touch();
        }
        Ok(g.history.items.clone())
    }

    pub fn clear_history(&self) -> Result<()> {
        let mut g = self.lock();
        let h = &mut g.history;
        let known = h.items.iter().map(|x| x.at).chain(h.removed.iter().map(|t| t.at)).chain(std::iter::once(h.cleared_at)).max().unwrap_or(0);
        h.cleared_at = after(now_ms(), known);
        h.items.clear();
        h.removed.clear();
        self.save_history(&g.history)?;
        self.auto.touch();
        Ok(())
    }

    // ---- علاقه‌مندی‌ها (جدیدترین اول) ----
    pub fn favorites(&self) -> Vec<FavoriteItem> {
        self.lock().favorites.items.clone()
    }

    /// اگر نبود اضافه می‌کند، اگر بود برمی‌دارد. خروجی: (اکنون علاقه‌مندی است؟، فهرست جدید)
    pub fn toggle_favorite(&self, word: WordRef, at: i64) -> Result<(bool, Vec<FavoriteItem>)> {
        let mut g = self.lock();
        let f = &mut g.favorites;
        let now_favorite = match f.items.iter().position(|x| x.word_norm == word.word_norm) {
            Some(pos) => {
                // برداشتن: گوری که اکیداً از خودِ آن علاقه‌مندی جلوتر است
                let live = f.items.remove(pos);
                push_tombstone(&mut f.removed, word.word_norm, after(at, live.at));
                false
            }
            None => {
                // افزودن: اکیداً جلوتر از برداشتنِ قبلیِ همین واژه؛ آن گور دیگر لازم نیست
                let known = tombstone_at(&f.removed, &word.word_norm).unwrap_or(0);
                f.removed.retain(|t| t.word_norm != word.word_norm);
                f.items.insert(0, FavoriteItem { word_norm: word.word_norm, word: word.word, at: after(at, known) });
                true
            }
        };
        self.save_favorites(&g.favorites)?;
        self.auto.touch();
        Ok((now_favorite, g.favorites.items.clone()))
    }

    // ---- تنظیمات ----
    pub fn settings(&self) -> Settings {
        self.lock().settings.clone()
    }

    pub fn set_settings(&self, settings: Settings) -> Result<Settings> {
        let mut g = self.lock();
        g.settings = settings.sanitized();
        let bytes = serde_json::to_vec_pretty(&SettingsFile { version: FORMAT_VERSION, settings: g.settings.clone() })?;
        write_atomic(&self.dir.join("settings.json"), &bytes)?;
        Ok(g.settings.clone())
    }

    // ---- پشتیبانِ همگام‌سازی (ماژول `sync`) ----

    /// عکسی از تاریخچه و علاقه‌مندی‌ها (با گورها) برای نوشتن در فایل همگام‌سازی.
    pub(crate) fn snapshot_lists(&self) -> (HistoryState, FavoritesState) {
        let g = self.lock();
        (g.history.clone(), g.favorites.clone())
    }

    /// فایل‌های دستگاه‌های دیگر را با داده‌های همین دستگاه ادغام می‌کند؛ همه‌چیز زیر یک قفل،
    /// پس تغییری که هم‌زمان در رابط انجام شود گم نمی‌شود. فقط اگر چیزی تغییر کرده باشد می‌نویسد.
    pub(crate) fn merge_remote(&self, remote_history: &[HistoryState], remote_favorites: &[FavoritesState]) -> Result<MergeCounts> {
        let mut g = self.lock();
        let merged_history = {
            let mut parts: Vec<&HistoryState> = vec![&g.history];
            parts.extend(remote_history.iter());
            merge_history(&parts)
        };
        let merged_favorites = {
            let mut parts: Vec<&FavoritesState> = vec![&g.favorites];
            parts.extend(remote_favorites.iter());
            merge_favorites(&parts)
        };
        let (history_added, history_removed) = history_diff(&g.history, &merged_history);
        let (favorites_added, favorites_removed) = favorites_diff(&g.favorites, &merged_favorites);
        let mut changed = false;
        if merged_history != g.history {
            g.history = merged_history;
            self.save_history(&g.history)?;
            changed = true;
        }
        if merged_favorites != g.favorites {
            g.favorites = merged_favorites;
            self.save_favorites(&g.favorites)?;
            changed = true;
        }
        if changed {
            self.auto.touch();
        }
        Ok(MergeCounts { favorites_added, favorites_removed, history_added, history_removed })
    }

    /// شناسهٔ پایدار این دستگاه؛ بار اول ساخته و در `sync.json` ذخیره می‌شود.
    pub(crate) fn device_id(&self) -> Result<String> {
        let mut g = self.lock();
        if let Some(id) = g.sync.device_id.clone().filter(|s| !s.is_empty()) {
            return Ok(id);
        }
        let id = new_device_id();
        g.sync.device_id = Some(id.clone());
        self.save_sync_config(&g.sync)?;
        Ok(id)
    }

    pub(crate) fn sync_dir_setting(&self) -> Option<String> {
        self.lock().sync.dir.clone()
    }

    pub(crate) fn store_sync_dir(&self, dir: Option<String>) -> Result<()> {
        let mut g = self.lock();
        g.sync.dir = dir;
        self.save_sync_config(&g.sync)?;
        self.auto.set_enabled(g.sync.dir.is_some() && g.sync.auto_export.unwrap_or(true));
        self.auto.touch(); // پوشهٔ تازه: نسخهٔ فعلی همین حالا در آن نوشته شود
        Ok(())
    }

    pub(crate) fn store_auto_export(&self, enabled: bool) -> Result<()> {
        let mut g = self.lock();
        g.sync.auto_export = Some(enabled);
        self.save_sync_config(&g.sync)?;
        self.auto.set_enabled(g.sync.dir.is_some() && enabled);
        self.auto.touch(); // روشن شد: وضعیت فعلی همین حالا نوشته شود
        Ok(())
    }

    pub(crate) fn auto_export_setting(&self) -> bool {
        self.lock().sync.auto_export.unwrap_or(true)
    }

    pub(crate) fn auto_shared(&self) -> Arc<AutoShared> {
        Arc::clone(&self.auto)
    }

    pub(crate) fn export_lock(&self) -> MutexGuard<'_, ()> {
        self.export_lock.lock().unwrap_or_else(|e| e.into_inner())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    fn w(n: &str) -> WordRef {
        WordRef { word_norm: n.into(), word: n.to_uppercase() }
    }

    fn store() -> (tempfile::TempDir, UserStore) {
        let d = tempfile::tempdir().unwrap();
        let s = UserStore::open(d.path().join("user")).unwrap();
        (d, s)
    }

    #[test]
    fn fresh_store_is_empty_with_default_settings() {
        let (_d, s) = store();
        assert!(s.history().is_empty() && s.favorites().is_empty());
        assert_eq!(s.settings(), Settings::default());
        assert!(s.warnings().is_empty());
    }

    #[test]
    fn history_is_newest_first_and_deduplicated() {
        let (_d, s) = store();
        s.record_visit(w("a"), 1).unwrap();
        s.record_visit(w("b"), 2).unwrap();
        let h = s.record_visit(w("a"), 3).unwrap();
        assert_eq!(h.iter().map(|x| (x.word_norm.as_str(), x.at)).collect::<Vec<_>>(), vec![("a", 3), ("b", 2)]);
    }

    #[test]
    fn history_is_capped() {
        let (_d, s) = store();
        for i in 0..(MAX_HISTORY + 25) {
            s.record_visit(w(&format!("w{i}")), i as i64).unwrap();
        }
        let h = s.history();
        assert_eq!(h.len(), MAX_HISTORY);
        assert_eq!(h[0].word_norm, format!("w{}", MAX_HISTORY + 24));
    }

    #[test]
    fn history_remove_and_clear() {
        let (_d, s) = store();
        s.record_visit(w("a"), 1).unwrap();
        s.record_visit(w("b"), 2).unwrap();
        assert_eq!(s.remove_history("a").unwrap().len(), 1);
        s.clear_history().unwrap();
        assert!(s.history().is_empty());
    }

    #[test]
    fn favorites_toggle() {
        let (_d, s) = store();
        let (on, list) = s.toggle_favorite(w("a"), 1).unwrap();
        assert!(on && list.len() == 1);
        let (on, _) = s.toggle_favorite(w("b"), 2).unwrap();
        assert!(on);
        assert_eq!(s.favorites()[0].word_norm, "b");
        let (on, list) = s.toggle_favorite(w("a"), 3).unwrap();
        assert!(!on && list.len() == 1 && list[0].word_norm == "b");
    }

    #[test]
    fn everything_persists_across_reopen_in_separate_files() {
        let d = tempfile::tempdir().unwrap();
        let dir = d.path().join("u");
        {
            let s = UserStore::open(&dir).unwrap();
            s.record_visit(w("a"), 5).unwrap();
            s.toggle_favorite(w("b"), 6).unwrap();
            s.set_settings(Settings { theme: Theme::Dark, font_scale: 1.3, fulltext: true }).unwrap();
        }
        for f in ["history.json", "favorites.json", "settings.json"] {
            assert!(dir.join(f).is_file(), "{f}");
        }
        assert!(!dir.join("history.json.tmp").exists());
        let s = UserStore::open(&dir).unwrap();
        assert_eq!(s.history()[0].word_norm, "a");
        assert_eq!(s.favorites()[0].word_norm, "b");
        assert_eq!(s.settings(), Settings { theme: Theme::Dark, font_scale: 1.3, fulltext: true });
        assert!(s.warnings().is_empty());
    }

    #[test]
    fn settings_are_clamped() {
        let (_d, s) = store();
        assert_eq!(s.set_settings(Settings { font_scale: 9.0, ..Default::default() }).unwrap().font_scale, MAX_FONT_SCALE);
        assert_eq!(s.set_settings(Settings { font_scale: 0.0, ..Default::default() }).unwrap().font_scale, MIN_FONT_SCALE);
        assert_eq!(s.set_settings(Settings { font_scale: f32::NAN, ..Default::default() }).unwrap().font_scale, 1.0);
    }

    #[test]
    fn corrupt_file_is_set_aside_and_reported() {
        let d = tempfile::tempdir().unwrap();
        let dir = d.path().join("u");
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("history.json"), "{ not json").unwrap();
        let s = UserStore::open(&dir).unwrap();
        assert!(s.history().is_empty());
        assert_eq!(s.warnings().len(), 1);
        let kept: Vec<_> = fs::read_dir(&dir).unwrap().filter_map(|e| e.ok()).map(|e| e.file_name().to_string_lossy().into_owned()).collect();
        assert!(kept.iter().any(|n| n.starts_with("history.corrupt-")), "{kept:?}");
        // و دوباره قابل نوشتن است
        s.record_visit(w("a"), 1).unwrap();
        assert_eq!(UserStore::open(&dir).unwrap().history().len(), 1);
    }

    #[test]
    fn unknown_future_fields_and_missing_fields_are_tolerated() {
        let d = tempfile::tempdir().unwrap();
        let dir = d.path().join("u");
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("settings.json"), r#"{"version":1,"theme":"dark","somethingNew":42}"#).unwrap();
        let s = UserStore::open(&dir).unwrap();
        assert_eq!(s.settings(), Settings { theme: Theme::Dark, font_scale: 1.0, fulltext: false });
    }

    #[test]
    fn concurrent_writers_do_not_lose_updates() {
        let d = tempfile::tempdir().unwrap();
        let s = Arc::new(UserStore::open(d.path().join("u")).unwrap());
        let threads: Vec<_> = (0..8)
            .map(|t| {
                let s = Arc::clone(&s);
                std::thread::spawn(move || {
                    for i in 0..10 {
                        s.record_visit(w(&format!("t{t}-{i}")), i).unwrap();
                    }
                })
            })
            .collect();
        for t in threads {
            t.join().unwrap();
        }
        assert_eq!(s.history().len(), 80);
        assert_eq!(UserStore::open(s.dir()).unwrap().history().len(), 80);
    }

    // ---- گورها و زمان پاک‌سازی (پایهٔ همگام‌سازی) ----

    #[test]
    fn removing_a_favorite_leaves_a_tombstone_and_adding_it_again_clears_it() {
        let (_d, s) = store();
        s.toggle_favorite(w("a"), 100).unwrap();
        s.toggle_favorite(w("a"), 200).unwrap(); // برداشتن
        let (_, f) = s.snapshot_lists();
        assert!(f.items.is_empty());
        assert_eq!(f.removed, vec![Tombstone { word_norm: "a".into(), at: 200 }]);
        let (on, list) = s.toggle_favorite(w("a"), 300).unwrap(); // دوباره افزودن
        assert!(on && list[0].at == 300);
        assert!(s.snapshot_lists().1.removed.is_empty());
    }

    #[test]
    fn new_events_are_newer_than_what_the_device_knows_even_with_a_slow_clock() {
        let (_d, s) = store();
        s.toggle_favorite(w("a"), 1_000).unwrap();
        // ساعت عقب رفته: «برداشتن» با زمانِ ۵۰ هم باید از افزودنِ ۱٬۰۰۰ جلوتر ثبت شود…
        s.toggle_favorite(w("a"), 50).unwrap();
        assert_eq!(s.snapshot_lists().1.removed[0].at, 1_001);
        // …و افزودنِ دوباره هم از برداشتن
        s.toggle_favorite(w("a"), 10).unwrap();
        assert_eq!(s.favorites()[0].at, 1_002);
    }

    #[test]
    fn clearing_history_records_a_time_and_later_visits_are_newer_than_it() {
        let (_d, s) = store();
        s.record_visit(w("a"), 500).unwrap();
        s.clear_history().unwrap();
        let (h, _) = s.snapshot_lists();
        assert!(h.items.is_empty() && h.cleared_at >= 501);
        let list = s.record_visit(w("b"), 1).unwrap(); // ساعت عقب
        assert!(list[0].at > h.cleared_at);
    }

    #[test]
    fn removing_one_history_entry_leaves_a_tombstone_and_a_later_visit_clears_it() {
        let (_d, s) = store();
        s.record_visit(w("a"), 10).unwrap();
        s.remove_history("a").unwrap();
        let (h, _) = s.snapshot_lists();
        assert!(h.items.is_empty() && h.removed.len() == 1 && h.removed[0].at > 10);
        let list = s.record_visit(w("a"), 5).unwrap();
        assert!(list[0].at > h.removed[0].at);
        assert!(s.snapshot_lists().0.removed.is_empty());
    }

    #[test]
    fn tombstones_and_clear_time_persist_across_reopen() {
        let d = tempfile::tempdir().unwrap();
        let dir = d.path().join("u");
        {
            let s = UserStore::open(&dir).unwrap();
            s.toggle_favorite(w("a"), 10).unwrap();
            s.toggle_favorite(w("a"), 20).unwrap();
            s.record_visit(w("h"), 30).unwrap();
            s.remove_history("h").unwrap();
        }
        let s = UserStore::open(&dir).unwrap();
        let (h, f) = s.snapshot_lists();
        assert_eq!(f.removed.len(), 1);
        assert_eq!(h.removed.len(), 1);
        s.clear_history().unwrap();
        let again = UserStore::open(&dir).unwrap();
        assert!(again.snapshot_lists().0.cleared_at > 0);
        assert!(again.warnings().is_empty());
    }

    #[test]
    fn files_from_older_versions_load_and_stay_in_the_old_shape_when_nothing_new_is_needed() {
        let d = tempfile::tempdir().unwrap();
        let dir = d.path().join("u");
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("history.json"), r#"{"version":1,"items":[{"wordNorm":"a","word":"A","at":5}]}"#).unwrap();
        fs::write(dir.join("favorites.json"), r#"{"version":1,"items":[{"wordNorm":"b","word":"B","at":6}]}"#).unwrap();
        let s = UserStore::open(&dir).unwrap();
        assert_eq!((s.history().len(), s.favorites().len()), (1, 1));
        assert!(s.warnings().is_empty());
        // تا وقتی گور و زمان پاک‌سازی نیست، فیلدهای تازه در فایل ظاهر نمی‌شوند
        s.record_visit(w("c"), 7).unwrap();
        let text = fs::read_to_string(dir.join("history.json")).unwrap();
        assert!(!text.contains("removed") && !text.contains("clearedAt"), "{text}");
    }

    #[test]
    fn tombstone_list_is_capped_and_deduplicated_per_word() {
        let mut list = Vec::new();
        for i in 0..(MAX_TOMBSTONES + 10) {
            push_tombstone(&mut list, format!("w{i}"), i as i64);
        }
        assert_eq!(list.len(), MAX_TOMBSTONES);
        assert_eq!(list[0].word_norm, format!("w{}", MAX_TOMBSTONES + 9));
        push_tombstone(&mut list, format!("w{}", MAX_TOMBSTONES + 5), 99_999);
        assert_eq!(list.len(), MAX_TOMBSTONES);
        assert_eq!((list[0].word_norm.clone(), list[0].at), (format!("w{}", MAX_TOMBSTONES + 5), 99_999));
    }
}
