//! همگام‌سازی دستی علاقه‌مندی‌ها و تاریخچه از راه یک پوشه (مثلاً پوشهٔ Google Drive، OneDrive یا Dropbox).
//!
//! * «خروجی گرفتن»: هر دستگاه فقط فایل خودش را می‌نویسد: `dehkhoda-sync-<نام-دستگاه>-<شناسه>.json`.
//!   چون دو دستگاه هیچ‌وقت یک فایل را نمی‌نویسند، سرویس‌های همگام‌سازی ابری «نسخهٔ تعارضی» نمی‌سازند.
//! * «ورودی گرفتن»: فایل‌های دستگاه‌های دیگر خوانده و با داده‌های همین دستگاه ادغام می‌شوند.
//! * ادغام برای هر واژه «آخرین تغییر برنده است» (افزودن/دیدن در برابر برداشتن/پاک‌کردن). نتیجه به
//!   ترتیب فایل‌ها بستگی ندارد و تکرارش همان نتیجه را می‌دهد؛ پس هر دستگاه هر وقت خواست ورودی
//!   می‌گیرد و همه به یک وضعیت می‌رسند. اگر زمان دو تغییر دقیقاً برابر باشد «ماندن» برنده است، تا
//!   هیچ‌چیز به‌خاطر یک تساوی گم نشود.
//! * «ذخیرهٔ خودکار» (پیش‌فرض روشن): وقتی پوشه انتخاب شده باشد، پس از هر تغییرِ علاقه‌مندی‌ها یا تاریخچه
//!   فایل همین دستگاه خودکار نوشته می‌شود؛ تغییرهای پشت‌سرهم یک نوشتن می‌شوند و هنگام بستن برنامه هم
//!   آنچه مانده نوشته می‌شود. فقط «خروجی» خودکار است؛ «ورودی گرفتن» همچنان دستی است.
//! * فایل‌های ناخوانا یا ناشناخته نادیده گرفته می‌شوند و دلیلشان در گزارش می‌آید؛ هرگز کل کار را
//!   نمی‌شکنند. فایل‌ها را هیچ‌وقت پاک یا دستکاری نمی‌کنیم.

use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Condvar, Mutex, MutexGuard, Weak};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

use crate::userdata::{now_ms, write_atomic, FavoriteItem, FavoritesState, HistoryItem, HistoryState, Tombstone, UserStore, MAX_HISTORY, MAX_TOMBSTONES};
use crate::{Error, Result};

/// مقدار فیلد `format` در فایل همگام‌سازی
pub const SYNC_FORMAT: &str = "dehkhoda-sync";
/// بالاترین نسخهٔ قالبی که این برنامه می‌فهمد
pub const SYNC_VERSION: u32 = 1;
const FILE_PREFIX: &str = "dehkhoda-sync-";
const FILE_SUFFIX: &str = ".json";
/// فایل‌های بزرگ‌تر از این خوانده نمی‌شوند (فایل واقعی چند ده کیلوبایت است)
const MAX_FILE_BYTES: u64 = 16 * 1024 * 1024;
const MAX_FAVORITES: usize = 100_000;
/// واژه‌ای که از این بلندتر باشد معتبر نیست (طولانی‌ترین سرواژه‌ها چند ده نویسه‌اند)
const MAX_WORD_CHARS: usize = 200;

// ───────────── ادغام ─────────────

trait Keyed: Clone {
    fn key(&self) -> &str;
    fn at(&self) -> i64;
    /// نوشتار نمایشی؛ فقط برای شکستن تساوی زمانی، تا نتیجه به ترتیب ورودی‌ها بستگی نداشته باشد
    fn label(&self) -> &str;
}

impl Keyed for HistoryItem {
    fn key(&self) -> &str {
        &self.word_norm
    }
    fn at(&self) -> i64 {
        self.at
    }
    fn label(&self) -> &str {
        &self.word
    }
}

impl Keyed for FavoriteItem {
    fn key(&self) -> &str {
        &self.word_norm
    }
    fn at(&self) -> i64 {
        self.at
    }
    fn label(&self) -> &str {
        &self.word
    }
}

/// «آخرین تغییر برنده است» برای هر واژه؛ خروجی (زنده‌ها، گورها) هر دو جدیدترین‌اول و مرتب‌اند.
fn merge_keyed<T: Keyed>(lists: &[&[T]], tombs: &[&[Tombstone]], cleared_at: i64) -> (Vec<T>, Vec<Tombstone>) {
    // جدیدترین نمونهٔ زندهٔ هر واژه (آنچه تا زمان «پاک کردن» یا قدیمی‌تر باشد اصلاً حساب نمی‌شود)
    let mut best: HashMap<&str, &T> = HashMap::new();
    for item in lists.iter().flat_map(|l| l.iter()) {
        if item.at() <= cleared_at || item.key().is_empty() {
            continue;
        }
        let replace = match best.get(item.key()) {
            Some(cur) => (item.at(), item.label()) > (cur.at(), cur.label()),
            None => true,
        };
        if replace {
            best.insert(item.key(), item);
        }
    }
    // جدیدترین برداشتنِ هر واژه
    let mut dead: HashMap<&str, i64> = HashMap::new();
    for t in tombs.iter().flat_map(|l| l.iter()) {
        if t.at <= cleared_at || t.word_norm.is_empty() {
            continue;
        }
        let slot = dead.entry(t.word_norm.as_str()).or_insert(t.at);
        *slot = (*slot).max(t.at);
    }
    let mut live: Vec<T> = Vec::new();
    for (k, item) in &best {
        let beaten = matches!(dead.get(k), Some(&d) if d > item.at());
        if !beaten {
            live.push((*item).clone());
        }
    }
    live.sort_by(|a, b| b.at().cmp(&a.at()).then_with(|| a.key().cmp(b.key())));
    let mut removed: Vec<Tombstone> = Vec::new();
    for (k, &d) in &dead {
        let word_stays = matches!(best.get(k), Some(item) if d <= item.at());
        if !word_stays {
            removed.push(Tombstone { word_norm: (*k).to_string(), at: d });
        }
    }
    removed.sort_by(|a, b| b.at.cmp(&a.at).then_with(|| a.word_norm.cmp(&b.word_norm)));
    removed.truncate(MAX_TOMBSTONES);
    (live, removed)
}

/// علاقه‌مندی‌های چند منبع (اولی معمولاً همین دستگاه) را یکی می‌کند.
pub fn merge_favorites(parts: &[&FavoritesState]) -> FavoritesState {
    let lists: Vec<&[FavoriteItem]> = parts.iter().map(|p| p.items.as_slice()).collect();
    let tombs: Vec<&[Tombstone]> = parts.iter().map(|p| p.removed.as_slice()).collect();
    let (items, removed) = merge_keyed(&lists, &tombs, i64::MIN);
    FavoritesState { items, removed }
}

/// تاریخچهٔ چند منبع را یکی می‌کند؛ «پاک کردنِ» هر منبع روی ورودی‌های قدیمی‌تر از خودش اثر دارد
/// و فقط ۵۰۰ مورد جدیدتر می‌ماند.
pub fn merge_history(parts: &[&HistoryState]) -> HistoryState {
    let cleared_at = parts.iter().map(|p| p.cleared_at).max().unwrap_or(0).max(0);
    let lists: Vec<&[HistoryItem]> = parts.iter().map(|p| p.items.as_slice()).collect();
    let tombs: Vec<&[Tombstone]> = parts.iter().map(|p| p.removed.as_slice()).collect();
    let (mut items, removed) = merge_keyed(&lists, &tombs, cleared_at);
    items.truncate(MAX_HISTORY);
    HistoryState { items, removed, cleared_at }
}

fn diff<T: Keyed>(before: &[T], after: &[T]) -> (usize, usize) {
    let old: HashSet<&str> = before.iter().map(|x| x.key()).collect();
    let new: HashSet<&str> = after.iter().map(|x| x.key()).collect();
    (new.difference(&old).count(), old.difference(&new).count())
}

/// (واژه‌های تازه، واژه‌های ازدست‌رفته) پس از ادغام
pub(crate) fn favorites_diff(before: &FavoritesState, after: &FavoritesState) -> (usize, usize) {
    diff(&before.items, &after.items)
}

pub(crate) fn history_diff(before: &HistoryState, after: &HistoryState) -> (usize, usize) {
    diff(&before.items, &after.items)
}

/// شمارش تغییرهای یک ادغام (برای گزارش به کاربر)
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct MergeCounts {
    pub favorites_added: usize,
    pub favorites_removed: usize,
    pub history_added: usize,
    pub history_removed: usize,
}

// ───────────── فایل همگام‌سازی ─────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncFile {
    pub format: String,
    pub version: u32,
    pub device_id: String,
    /// فقط برای آدم‌ها (چه دستگاهی این فایل را نوشته)
    #[serde(default)]
    pub device_name: String,
    #[serde(default)]
    pub exported_at: i64,
    #[serde(default)]
    pub favorites: FavoritesState,
    #[serde(default)]
    pub history: HistoryState,
}

fn valid_text(s: &str) -> bool {
    !s.trim().is_empty() && s.chars().count() <= MAX_WORD_CHARS
}

/// فایل از بیرون می‌آید و قابل‌اعتماد نیست: موردهای بی‌معنی حذف و اندازه‌ها محدود می‌شوند.
fn clean(file: &mut SyncFile) {
    let f = &mut file.favorites;
    f.items.retain(|x| valid_text(&x.word_norm) && valid_text(&x.word) && x.at > 0);
    f.removed.retain(|t| valid_text(&t.word_norm) && t.at > 0);
    f.items.sort_by(|a, b| b.at.cmp(&a.at));
    f.items.truncate(MAX_FAVORITES);
    f.removed.sort_by(|a, b| b.at.cmp(&a.at));
    f.removed.truncate(MAX_TOMBSTONES);

    let h = &mut file.history;
    h.items.retain(|x| valid_text(&x.word_norm) && valid_text(&x.word) && x.at > 0);
    h.removed.retain(|t| valid_text(&t.word_norm) && t.at > 0);
    h.items.sort_by(|a, b| b.at.cmp(&a.at));
    h.items.truncate(MAX_HISTORY);
    h.removed.sort_by(|a, b| b.at.cmp(&a.at));
    h.removed.truncate(MAX_TOMBSTONES);
    h.cleared_at = h.cleared_at.max(0);
}

/// یک فایل همگام‌سازی را می‌خواند؛ خطا = دلیل فارسی برای گزارش (نه شکستن کل ورودی).
fn read_sync_file(path: &Path) -> std::result::Result<SyncFile, String> {
    let meta = fs::metadata(path).map_err(|e| format!("خواندن ممکن نشد ({e})"))?;
    if meta.len() > MAX_FILE_BYTES {
        return Err("بیش‌ازحد بزرگ است و نادیده گرفته شد".to_string());
    }
    let bytes = fs::read(path).map_err(|e| format!("خواندن ممکن نشد ({e})"))?;
    let bytes = bytes.strip_prefix(&[0xEF, 0xBB, 0xBF][..]).unwrap_or(&bytes);
    let mut file: SyncFile = serde_json::from_slice(bytes).map_err(|_| "فایل خراب یا ناشناخته است".to_string())?;
    if file.format != SYNC_FORMAT {
        return Err("فایل همگام‌سازی دهخدا نیست".to_string());
    }
    if file.version > SYNC_VERSION {
        return Err("با نسخهٔ جدیدتر برنامه ساخته شده؛ برنامه را به‌روز کنید".to_string());
    }
    if file.device_id.trim().is_empty() || file.device_id.chars().count() > 64 {
        return Err("شناسهٔ دستگاه نامعتبر است".to_string());
    }
    clean(&mut file);
    Ok(file)
}

/// فایل‌های همگام‌سازیِ یک پوشه، به ترتیب نام (فایل‌های «نسخهٔ تعارضی» سرویس‌های ابری هم می‌آیند).
fn list_sync_files(dir: &Path) -> std::io::Result<Vec<PathBuf>> {
    let mut found = Vec::new();
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let name = entry.file_name().to_string_lossy().to_lowercase();
        let is_file = entry.file_type().map(|t| t.is_file()).unwrap_or(false);
        if is_file && name.starts_with(FILE_PREFIX) && name.ends_with(FILE_SUFFIX) {
            found.push(entry.path());
        }
    }
    found.sort();
    Ok(found)
}

/// نام دستگاه برای نام فایل: فقط حرف و عدد انگلیسی، `-` و `_`؛ وگرنه «device».
fn clean_device_name(raw: &str) -> String {
    let mapped: String = raw.chars().map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' { c } else { '-' }).collect();
    let short: String = mapped.trim_matches('-').chars().take(24).collect();
    let short = short.trim_end_matches('-');
    if short.is_empty() {
        "device".to_string()
    } else {
        short.to_string()
    }
}

pub(crate) fn device_name() -> String {
    let raw = std::env::var("COMPUTERNAME").or_else(|_| std::env::var("HOSTNAME")).unwrap_or_default();
    clean_device_name(&raw)
}

/// شناسهٔ تصادفی ۸ رقمی (هگز) برای یک دستگاه
pub(crate) fn new_device_id() -> String {
    use std::collections::hash_map::RandomState;
    use std::hash::{BuildHasher, Hasher};
    let mut h = RandomState::new().build_hasher();
    h.write_i64(now_ms());
    h.write_u32(std::process::id());
    format!("{:08x}", h.finish() as u32)
}

pub fn sync_file_name(device_name: &str, device_id: &str) -> String {
    format!("{FILE_PREFIX}{device_name}-{device_id}{FILE_SUFFIX}")
}

// ───────────── ذخیرهٔ خودکار ─────────────

/// تغییرهای پشت‌سرهم تا این‌قدر که ساکت بمانند منتظر می‌مانیم و بعد یک بار می‌نویسیم…
pub const AUTO_DEBOUNCE: Duration = Duration::from_millis(1_500);
/// …ولی نه بیش از این‌قدر از اولین تغییرِ آن دسته.
pub const AUTO_MAX_DELAY: Duration = Duration::from_secs(10);

/// نتیجهٔ آخرین ذخیرهٔ خودکار (فقط در حافظه است و با بسته شدن برنامه از بین می‌رود)
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AutoStatus {
    /// میلی‌ثانیهٔ یونیکس
    pub at: i64,
    /// `None` = موفق
    pub error: Option<String>,
}

#[derive(Default)]
struct AutoState {
    dirty: bool,
    first: Option<Instant>,
    last: Option<Instant>,
    stop: bool,
}

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

/// هماهنگی بین تغییرها (که `touch` می‌زنند) و نخِ پس‌زمینهٔ ذخیرهٔ خودکار.
pub(crate) struct AutoShared {
    /// پوشه انتخاب شده و ذخیرهٔ خودکار روشن است؟
    enabled: AtomicBool,
    exports: AtomicUsize,
    state: Mutex<AutoState>,
    cv: Condvar,
    last_result: Mutex<Option<AutoStatus>>,
}

impl AutoShared {
    pub(crate) fn new() -> Self {
        AutoShared { enabled: AtomicBool::new(false), exports: AtomicUsize::new(0), state: Mutex::new(AutoState::default()), cv: Condvar::new(), last_result: Mutex::new(None) }
    }

    pub(crate) fn set_enabled(&self, on: bool) {
        self.enabled.store(on, Ordering::SeqCst);
    }

    pub(crate) fn is_enabled(&self) -> bool {
        self.enabled.load(Ordering::SeqCst)
    }

    /// «یک تغییر ثبت شد»؛ فقط وقتی کار می‌کند که ذخیرهٔ خودکار فعال باشد.
    pub(crate) fn touch(&self) {
        if !self.is_enabled() {
            return;
        }
        let mut st = lock(&self.state);
        let now = Instant::now();
        if !st.dirty {
            st.dirty = true;
            st.first = Some(now);
        }
        st.last = Some(now);
        self.cv.notify_all();
    }

    /// آیا تغییرِ ننوشته‌ای هست؟ (و علامتش را برمی‌دارد)
    pub(crate) fn take_dirty(&self) -> bool {
        let mut st = lock(&self.state);
        let was = st.dirty;
        st.dirty = false;
        st.first = None;
        st.last = None;
        was
    }

    pub(crate) fn stop(&self) {
        lock(&self.state).stop = true;
        self.cv.notify_all();
    }

    pub(crate) fn last(&self) -> Option<AutoStatus> {
        lock(&self.last_result).clone()
    }

    fn set_last(&self, status: AutoStatus) {
        *lock(&self.last_result) = Some(status);
    }

    /// تعداد نوشتن‌هایی که تا حالا انجام شده (برای تست‌ها)
    #[cfg(test)]
    pub(crate) fn exports(&self) -> usize {
        self.exports.load(Ordering::SeqCst)
    }
}

/// نخِ پس‌زمینه: منتظر تغییر می‌ماند، تا آرام‌شدنِ تغییرها صبر می‌کند و یک بار می‌نویسد. فقط `Weak` به
/// ذخیره‌گاه دارد؛ با رها شدن ذخیره‌گاه (`Drop`) خودش تمام می‌شود.
fn run_auto_worker(shared: Arc<AutoShared>, store: Weak<UserStore>, debounce: Duration, max_delay: Duration) {
    loop {
        let mut st = lock(&shared.state);
        while !st.dirty && !st.stop {
            st = shared.cv.wait(st).unwrap_or_else(|e| e.into_inner());
        }
        if st.stop {
            return;
        }
        // تا وقتی تغییرها ادامه دارند صبر می‌کنیم (سقف: max_delay از اولین تغییر)
        while st.dirty && !st.stop {
            let (Some(first), Some(last)) = (st.first, st.last) else { break };
            let deadline = (last + debounce).min(first + max_delay);
            let now = Instant::now();
            if now >= deadline {
                break;
            }
            st = shared.cv.wait_timeout(st, deadline - now).unwrap_or_else(|e| e.into_inner()).0;
        }
        if st.stop {
            return;
        }
        if !st.dirty {
            continue; // کسی (مثلاً `flush_auto_export`) در این فاصله خودش نوشته است
        }
        st.dirty = false;
        st.first = None;
        st.last = None;
        drop(st);
        let Some(store) = store.upgrade() else { return };
        store.auto_export_now();
    }
}

// ───────────── گزارش‌ها و دستورها ─────────────

/// وضعیت همگام‌سازی برای نمایش در رابط
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SyncInfo {
    /// پوشهٔ انتخاب‌شده (اگر هنوز انتخاب نشده `None`)
    pub dir: Option<String>,
    pub device_name: String,
    /// نام فایلی که این دستگاه در آن پوشه می‌نویسد
    pub file_name: String,
    /// ذخیرهٔ خودکار پس از هر تغییر روشن است؟ (پیش‌فرض: بله)
    pub auto_export: bool,
    /// نتیجهٔ آخرین ذخیرهٔ خودکار در همین اجرای برنامه (`None` = هنوز انجام نشده)
    pub last_auto: Option<AutoStatus>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ExportReport {
    pub dir: String,
    pub file: String,
    /// تعداد علاقه‌مندی‌ها و واژه‌های تاریخچه که در فایل نوشته شد
    pub favorites: usize,
    pub history: usize,
}

/// فایلی که هنگام ورودی خوانده نشد، با دلیلش (برای نمایش به کاربر)
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SkippedFile {
    pub file: String,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ImportReport {
    pub dir: String,
    /// چند فایل از دستگاه‌های دیگر خوانده شد
    pub files_read: usize,
    /// فایل‌هایی که خوانده نشد، هر کدام با دلیلش
    pub skipped: Vec<SkippedFile>,
    pub favorites_added: usize,
    pub favorites_removed: usize,
    pub history_added: usize,
    pub history_removed: usize,
    /// فهرست‌های تازهٔ همین دستگاه پس از ادغام (رابط با آن‌ها خودش را به‌روز می‌کند)
    pub favorites: Vec<FavoriteItem>,
    pub history: Vec<HistoryItem>,
}

impl UserStore {
    /// پوشهٔ همگام‌سازیِ انتخاب‌شده (بدون ساختن فایل یا شناسه؛ برای پیش‌فرض پنجرهٔ انتخاب پوشه).
    pub fn sync_dir(&self) -> Option<String> {
        self.sync_dir_setting()
    }

    /// پوشهٔ انتخاب‌شده و نام فایل این دستگاه.
    pub fn sync_info(&self) -> Result<SyncInfo> {
        let id = self.device_id()?;
        let name = device_name();
        Ok(SyncInfo {
            dir: self.sync_dir_setting(),
            file_name: sync_file_name(&name, &id),
            device_name: name,
            auto_export: self.auto_export_setting(),
            last_auto: self.auto_shared().last(),
        })
    }

    /// ذخیرهٔ خودکار را روشن یا خاموش می‌کند (انتخاب در `sync.json` می‌ماند). روشن شدن، وضعیت فعلی را
    /// بی‌درنگ (با همان تأخیر کوتاه) در پوشه می‌نویسد.
    pub fn set_auto_export(&self, enabled: bool) -> Result<SyncInfo> {
        self.store_auto_export(enabled)?;
        self.sync_info()
    }

    /// نخ پس‌زمینهٔ ذخیرهٔ خودکار را راه می‌اندازد؛ یک بار هنگام شروع برنامه.
    pub fn spawn_auto_export(self: &Arc<Self>) -> std::io::Result<()> {
        self.spawn_auto_export_with(AUTO_DEBOUNCE, AUTO_MAX_DELAY)
    }

    pub fn spawn_auto_export_with(self: &Arc<Self>, debounce: Duration, max_delay: Duration) -> std::io::Result<()> {
        let shared = self.auto_shared();
        let weak = Arc::downgrade(self);
        std::thread::Builder::new().name("dehkhoda-autosync".into()).spawn(move || run_auto_worker(shared, weak, debounce, max_delay))?;
        Ok(())
    }

    /// تغییرهای هنوز ننوشته را همین حالا می‌نویسد (هنگام بسته شدن برنامه صدا زده می‌شود).
    pub fn flush_auto_export(&self) {
        if self.auto_shared().take_dirty() {
            self.auto_export_now();
        }
    }

    /// یک ذخیرهٔ خودکار؛ `None` یعنی پوشه‌ای انتخاب نشده یا ذخیرهٔ خودکار خاموش است.
    pub(crate) fn auto_export_now(&self) -> Option<AutoStatus> {
        let auto = self.auto_shared();
        let dir = self.sync_dir_setting()?;
        if !auto.is_enabled() {
            return None;
        }
        let outcome = self.export_sync_to(Path::new(&dir));
        let status = AutoStatus { at: now_ms(), error: outcome.err().map(|e| e.to_string()) };
        auto.set_last(status.clone());
        auto.exports.fetch_add(1, Ordering::SeqCst);
        Some(status)
    }

    /// پوشه را انتخاب (یا با `None` فراموش) می‌کند. پوشه باید وجود داشته باشد.
    pub fn set_sync_dir(&self, dir: Option<String>) -> Result<SyncInfo> {
        let dir = dir.map(|d| d.trim().to_string()).filter(|d| !d.is_empty());
        if let Some(d) = &dir {
            if !Path::new(d).is_dir() {
                return Err(Error::Sync(format!("پوشه‌ای با این مسیر پیدا نشد: {d}")));
            }
        }
        self.store_sync_dir(dir)?;
        self.sync_info()
    }

    fn configured_sync_dir(&self) -> Result<PathBuf> {
        self.sync_dir_setting().map(PathBuf::from).ok_or_else(|| Error::Sync("هنوز پوشه‌ای برای همگام‌سازی انتخاب نشده است".into()))
    }

    /// خروجی گرفتن در پوشهٔ انتخاب‌شده.
    pub fn export_sync(&self) -> Result<ExportReport> {
        let dir = self.configured_sync_dir()?;
        self.export_sync_to(&dir)
    }

    /// ورودی گرفتن از پوشهٔ انتخاب‌شده.
    pub fn import_sync(&self) -> Result<ImportReport> {
        let dir = self.configured_sync_dir()?;
        self.import_sync_from(&dir)
    }

    /// فایل این دستگاه را در `dir` می‌نویسد (اتمیک؛ فایل‌های دیگر دست‌نخورده می‌مانند).
    pub fn export_sync_to(&self, dir: &Path) -> Result<ExportReport> {
        if !dir.is_dir() {
            return Err(Error::Sync(format!("پوشه پیدا نشد: {}", dir.display())));
        }
        let _one_writer_at_a_time = self.export_lock();
        let device_id = self.device_id()?;
        let name = device_name();
        let (history, favorites) = self.snapshot_lists();
        let file = SyncFile {
            format: SYNC_FORMAT.to_string(),
            version: SYNC_VERSION,
            device_id: device_id.clone(),
            device_name: name.clone(),
            exported_at: now_ms(),
            favorites,
            history,
        };
        let bytes = serde_json::to_vec_pretty(&file)?;
        let file_name = sync_file_name(&name, &device_id);
        write_atomic(&dir.join(&file_name), &bytes).map_err(|e| Error::Sync(format!("نوشتن فایل در پوشهٔ {} ممکن نشد: {e}", dir.display())))?;
        Ok(ExportReport { dir: dir.display().to_string(), file: file_name, favorites: file.favorites.items.len(), history: file.history.items.len() })
    }

    /// فایل‌های همگام‌سازیِ دستگاه‌های دیگر را از `dir` می‌خواند و با داده‌های همین دستگاه ادغام می‌کند.
    pub fn import_sync_from(&self, dir: &Path) -> Result<ImportReport> {
        if !dir.is_dir() {
            return Err(Error::Sync(format!("پوشه پیدا نشد: {}", dir.display())));
        }
        let own_id = self.device_id()?;
        let own_file = sync_file_name(&device_name(), &own_id);
        let paths = list_sync_files(dir).map_err(|e| Error::Sync(format!("خواندن پوشهٔ {} ممکن نشد: {e}", dir.display())))?;

        let mut files_read = 0usize;
        let mut skipped = Vec::new();
        let mut remote_history = Vec::new();
        let mut remote_favorites = Vec::new();
        for path in paths {
            let name = path.file_name().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
            if name.eq_ignore_ascii_case(&own_file) {
                continue;
            }
            match read_sync_file(&path) {
                // نسخهٔ قدیمیِ فایل خودِ همین دستگاه (مثلاً با نام عوض‌شده): ارزشی ندارد و نباید چیزی را برگرداند
                Ok(f) if f.device_id == own_id => {}
                Ok(f) => {
                    files_read += 1;
                    remote_history.push(f.history);
                    remote_favorites.push(f.favorites);
                }
                Err(reason) => skipped.push(SkippedFile { file: name, reason }),
            }
        }
        let counts = if files_read > 0 { self.merge_remote(&remote_history, &remote_favorites)? } else { MergeCounts::default() };
        Ok(ImportReport {
            dir: dir.display().to_string(),
            files_read,
            skipped,
            favorites_added: counts.favorites_added,
            favorites_removed: counts.favorites_removed,
            history_added: counts.history_added,
            history_removed: counts.history_removed,
            favorites: self.favorites(),
            history: self.history(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::userdata::WordRef;

    fn fav(n: &str, at: i64) -> FavoriteItem {
        FavoriteItem { word_norm: n.into(), word: n.to_uppercase(), at }
    }
    fn hist(n: &str, at: i64) -> HistoryItem {
        HistoryItem { word_norm: n.into(), word: n.to_uppercase(), at }
    }
    fn tomb(n: &str, at: i64) -> Tombstone {
        Tombstone { word_norm: n.into(), at }
    }
    fn favs(items: Vec<FavoriteItem>, removed: Vec<Tombstone>) -> FavoritesState {
        FavoritesState { items, removed }
    }
    fn hists(items: Vec<HistoryItem>, removed: Vec<Tombstone>, cleared_at: i64) -> HistoryState {
        HistoryState { items, removed, cleared_at }
    }
    fn w(n: &str) -> WordRef {
        WordRef { word_norm: n.into(), word: n.to_uppercase() }
    }
    fn keys(items: &[FavoriteItem]) -> Vec<&str> {
        items.iter().map(|i| i.word_norm.as_str()).collect()
    }

    // ---- ادغام ----

    #[test]
    fn union_keeps_every_word_and_the_newest_version_of_each() {
        let a = favs(vec![fav("x", 10), fav("y", 5)], vec![]);
        let b = favs(vec![fav("x", 20), fav("z", 7)], vec![]);
        let m = merge_favorites(&[&a, &b]);
        assert_eq!(m.items.iter().map(|i| (i.word_norm.as_str(), i.at)).collect::<Vec<_>>(), vec![("x", 20), ("z", 7), ("y", 5)]);
        assert!(m.removed.is_empty());
    }

    #[test]
    fn a_later_removal_beats_an_earlier_add_and_a_later_add_beats_an_earlier_removal() {
        let added = favs(vec![fav("x", 10)], vec![]);
        let removed_later = favs(vec![], vec![tomb("x", 20)]);
        let m = merge_favorites(&[&added, &removed_later]);
        assert!(m.items.is_empty());
        assert_eq!(m.removed, vec![tomb("x", 20)]);
        let added_later = favs(vec![fav("x", 30)], vec![]);
        let m = merge_favorites(&[&m, &added_later]);
        assert_eq!(m.items, vec![fav("x", 30)]);
        assert!(m.removed.is_empty());
    }

    #[test]
    fn on_a_tie_the_word_stays() {
        let m = merge_favorites(&[&favs(vec![fav("x", 10)], vec![]), &favs(vec![], vec![tomb("x", 10)])]);
        assert_eq!(m.items, vec![fav("x", 10)]);
        assert!(m.removed.is_empty());
    }

    #[test]
    fn equal_times_pick_the_same_label_whatever_the_order() {
        let a = favs(vec![FavoriteItem { word_norm: "k".into(), word: "alpha".into(), at: 5 }], vec![]);
        let b = favs(vec![FavoriteItem { word_norm: "k".into(), word: "beta".into(), at: 5 }], vec![]);
        assert_eq!(merge_favorites(&[&a, &b]), merge_favorites(&[&b, &a]));
    }

    #[test]
    fn clear_time_drops_older_and_equal_entries_but_keeps_newer_ones() {
        let a = hists(vec![hist("old", 10), hist("same", 50), hist("new", 60)], vec![tomb("t", 40)], 0);
        let b = hists(vec![], vec![], 50);
        let m = merge_history(&[&a, &b]);
        assert_eq!(m.cleared_at, 50);
        assert_eq!(m.items, vec![hist("new", 60)]);
        assert!(m.removed.is_empty(), "{:?}", m.removed);
    }

    #[test]
    fn merged_history_keeps_only_the_newest_entries() {
        let many = hists((0..(MAX_HISTORY + 20)).map(|i| hist(&format!("w{i}"), 1 + i as i64)).collect(), vec![], 0);
        let m = merge_history(&[&many, &hists(vec![hist("extra", 3)], vec![], 0)]);
        assert_eq!(m.items.len(), MAX_HISTORY);
        assert_eq!(m.items[0].word_norm, format!("w{}", MAX_HISTORY + 19));
        assert!(m.items.iter().all(|i| i.word_norm != "extra"));
    }

    struct Lcg(u64);
    impl Lcg {
        fn next(&mut self) -> u64 {
            self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            self.0 >> 33
        }
        fn below(&mut self, n: u64) -> u64 {
            self.next() % n
        }
    }

    fn random_favs(r: &mut Lcg) -> FavoritesState {
        let mut s = FavoritesState::default();
        for _ in 0..r.below(8) {
            s.items.push(fav(&format!("k{}", r.below(6)), 1 + r.below(30) as i64));
        }
        for _ in 0..r.below(6) {
            s.removed.push(tomb(&format!("k{}", r.below(6)), 1 + r.below(30) as i64));
        }
        s
    }

    fn random_hist(r: &mut Lcg) -> HistoryState {
        let mut s = HistoryState::default();
        for _ in 0..r.below(8) {
            s.items.push(hist(&format!("k{}", r.below(6)), 1 + r.below(30) as i64));
        }
        for _ in 0..r.below(6) {
            s.removed.push(tomb(&format!("k{}", r.below(6)), 1 + r.below(30) as i64));
        }
        if r.below(4) == 0 {
            s.cleared_at = r.below(25) as i64;
        }
        s
    }

    #[test]
    fn merging_is_commutative_idempotent_and_associative_on_random_states() {
        for seed in 0..400u64 {
            let mut r = Lcg(seed * 7919 + 13);
            let (a, b, c) = (random_favs(&mut r), random_favs(&mut r), random_favs(&mut r));
            let ab = merge_favorites(&[&a, &b]);
            assert_eq!(ab, merge_favorites(&[&b, &a]), "favorites commutative, seed {seed}");
            assert_eq!(merge_favorites(&[&ab, &ab]), ab, "favorites idempotent, seed {seed}");
            let left = merge_favorites(&[&ab, &c]);
            assert_eq!(left, merge_favorites(&[&a, &merge_favorites(&[&b, &c])]), "favorites associative, seed {seed}");
            assert_eq!(left, merge_favorites(&[&a, &b, &c]), "favorites n-ary, seed {seed}");

            let (a, b, c) = (random_hist(&mut r), random_hist(&mut r), random_hist(&mut r));
            let ab = merge_history(&[&a, &b]);
            assert_eq!(ab, merge_history(&[&b, &a]), "history commutative, seed {seed}");
            assert_eq!(merge_history(&[&ab, &ab]), ab, "history idempotent, seed {seed}");
            let left = merge_history(&[&ab, &c]);
            assert_eq!(left, merge_history(&[&a, &merge_history(&[&b, &c])]), "history associative, seed {seed}");
            assert_eq!(left, merge_history(&[&a, &b, &c]), "history n-ary, seed {seed}");
        }
    }

    // ---- فایل، پوشه و دستگاه‌ها ----

    fn two_devices() -> (tempfile::TempDir, PathBuf, UserStore, UserStore) {
        let root = tempfile::tempdir().unwrap();
        let shared = root.path().join("drive");
        fs::create_dir_all(&shared).unwrap();
        let a = UserStore::open(root.path().join("a")).unwrap();
        let b = UserStore::open(root.path().join("b")).unwrap();
        (root, shared, a, b)
    }

    #[test]
    fn each_device_writes_its_own_file_and_import_brings_in_the_other_devices_data() {
        let (_root, shared, a, b) = two_devices();
        a.toggle_favorite(w("x"), 100).unwrap();
        a.record_visit(w("h1"), 101).unwrap();
        b.toggle_favorite(w("y"), 102).unwrap();
        let ra = a.export_sync_to(&shared).unwrap();
        let rb = b.export_sync_to(&shared).unwrap();
        assert_ne!(ra.file, rb.file);
        assert!(ra.file.starts_with("dehkhoda-sync-") && ra.file.ends_with(".json"), "{}", ra.file);
        assert_eq!((ra.favorites, ra.history), (1, 1));
        assert!(!shared.join(format!("{}.tmp", ra.file)).exists());

        let r = a.import_sync_from(&shared).unwrap();
        assert_eq!(r.files_read, 1, "فایل خودش را نمی‌خواند");
        assert_eq!((r.favorites_added, r.favorites_removed, r.history_added, r.history_removed), (1, 0, 0, 0));
        assert_eq!(keys(&r.favorites), vec!["y", "x"]);
        assert_eq!(r.history.len(), 1);
        // ذخیره شده است
        assert_eq!(keys(&UserStore::open(a.dir()).unwrap().favorites()), vec!["y", "x"]);

        let r = b.import_sync_from(&shared).unwrap();
        assert_eq!((r.favorites_added, r.history_added), (1, 1));
        assert_eq!(keys(&b.favorites()), vec!["y", "x"]);
        assert_eq!(b.history()[0].word_norm, "h1");
    }

    #[test]
    fn removals_and_clearing_travel_between_devices_and_nothing_comes_back() {
        let (_root, shared, a, b) = two_devices();
        a.toggle_favorite(w("x"), 100).unwrap();
        a.toggle_favorite(w("keep"), 101).unwrap();
        a.record_visit(w("h"), 102).unwrap();
        a.export_sync_to(&shared).unwrap();
        b.import_sync_from(&shared).unwrap();
        assert_eq!(b.favorites().len(), 2);
        assert_eq!(b.history().len(), 1);

        // B یکی را برمی‌دارد و تاریخچه را پاک می‌کند
        b.toggle_favorite(w("x"), 200).unwrap();
        b.clear_history().unwrap();
        b.export_sync_to(&shared).unwrap();
        let r = a.import_sync_from(&shared).unwrap();
        assert_eq!((r.favorites_removed, r.history_removed), (1, 1));
        assert_eq!(keys(&a.favorites()), vec!["keep"]);
        assert!(a.history().is_empty());

        // A دوباره خروجی می‌دهد و B ورودی می‌گیرد: هیچ‌چیز برنمی‌گردد
        a.export_sync_to(&shared).unwrap();
        let r = b.import_sync_from(&shared).unwrap();
        assert_eq!((r.favorites_added, r.favorites_removed, r.history_added, r.history_removed), (0, 0, 0, 0));
        assert_eq!(keys(&b.favorites()), vec!["keep"]);
        assert!(b.history().is_empty());
    }

    #[test]
    fn a_removal_wins_even_when_the_removing_device_has_a_slow_clock() {
        let (_root, shared, a, b) = two_devices();
        a.toggle_favorite(w("x"), 1_000_000).unwrap();
        a.export_sync_to(&shared).unwrap();
        b.import_sync_from(&shared).unwrap();
        b.toggle_favorite(w("x"), 5).unwrap(); // ساعت B خیلی عقب است
        b.export_sync_to(&shared).unwrap();
        a.import_sync_from(&shared).unwrap();
        assert!(a.favorites().is_empty());
    }

    #[test]
    fn importing_twice_changes_nothing_the_second_time() {
        let (_root, shared, a, b) = two_devices();
        b.toggle_favorite(w("y"), 10).unwrap();
        b.record_visit(w("h"), 11).unwrap();
        b.export_sync_to(&shared).unwrap();
        let r1 = a.import_sync_from(&shared).unwrap();
        assert_eq!((r1.favorites_added, r1.history_added), (1, 1));
        let before = (fs::read(a.dir().join("favorites.json")).unwrap(), fs::read(a.dir().join("history.json")).unwrap());
        let r2 = a.import_sync_from(&shared).unwrap();
        assert_eq!((r2.favorites_added, r2.favorites_removed, r2.history_added, r2.history_removed), (0, 0, 0, 0));
        assert_eq!(before, (fs::read(a.dir().join("favorites.json")).unwrap(), fs::read(a.dir().join("history.json")).unwrap()));
    }

    #[test]
    fn a_device_ignores_an_old_copy_of_its_own_file_even_if_it_was_renamed() {
        let (_root, shared, a, _b) = two_devices();
        a.toggle_favorite(w("x"), 10).unwrap();
        let rep = a.export_sync_to(&shared).unwrap();
        fs::rename(shared.join(&rep.file), shared.join("dehkhoda-sync-renamed (1).json")).unwrap();
        a.toggle_favorite(w("x"), 20).unwrap(); // حالا برداشته شده
        let r = a.import_sync_from(&shared).unwrap();
        assert_eq!(r.files_read, 0);
        assert!(a.favorites().is_empty(), "نسخهٔ قدیمیِ خودش نباید برش گرداند");
    }

    #[test]
    fn unreadable_or_foreign_files_are_skipped_with_a_reason_and_never_break_the_import() {
        let (_root, shared, a, b) = two_devices();
        b.toggle_favorite(w("y"), 10).unwrap();
        b.export_sync_to(&shared).unwrap();
        fs::write(shared.join("dehkhoda-sync-broken.json"), "{ not json").unwrap();
        fs::write(shared.join("dehkhoda-sync-other.json"), r#"{"format":"something-else","version":1,"deviceId":"z"}"#).unwrap();
        fs::write(shared.join("dehkhoda-sync-future.json"), r#"{"format":"dehkhoda-sync","version":99,"deviceId":"f"}"#).unwrap();
        fs::write(shared.join("notes.txt"), "irrelevant").unwrap();
        let r = a.import_sync_from(&shared).unwrap();
        assert_eq!(r.files_read, 1);
        assert_eq!(r.skipped.len(), 3, "{:?}", r.skipped);
        assert!(r.skipped.iter().any(|s| s.file == "dehkhoda-sync-broken.json" && !s.reason.is_empty()), "{:?}", r.skipped);
        assert_eq!(keys(&a.favorites()), vec!["y"]);
    }

    #[test]
    fn hostile_entries_inside_a_valid_file_are_dropped() {
        let (_root, shared, a, _b) = two_devices();
        let long = "x".repeat(MAX_WORD_CHARS + 1);
        let body = format!(
            r#"{{"format":"dehkhoda-sync","version":1,"deviceId":"evil","favorites":{{"items":[
                {{"wordNorm":"","word":"A","at":5}},
                {{"wordNorm":"{long}","word":"B","at":5}},
                {{"wordNorm":"ok","word":"OK","at":5}},
                {{"wordNorm":"neg","word":"N","at":-3}}]}}}}"#
        );
        fs::write(shared.join("dehkhoda-sync-evil.json"), body).unwrap();
        let r = a.import_sync_from(&shared).unwrap();
        assert_eq!(keys(&r.favorites), vec!["ok"]);
    }

    #[test]
    fn the_sync_folder_is_remembered_validated_and_required() {
        let root = tempfile::tempdir().unwrap();
        let user = root.path().join("u");
        let drive = root.path().join("drive");
        fs::create_dir_all(&drive).unwrap();
        let drive_text = drive.to_string_lossy().into_owned();

        let s = UserStore::open(&user).unwrap();
        assert!(s.sync_info().unwrap().dir.is_none());
        assert!(matches!(s.export_sync(), Err(Error::Sync(_))));
        assert!(matches!(s.import_sync(), Err(Error::Sync(_))));
        assert!(matches!(s.set_sync_dir(Some(root.path().join("missing").to_string_lossy().into_owned())), Err(Error::Sync(_))));
        let file_name = s.set_sync_dir(Some(format!("  {drive_text}  "))).unwrap().file_name;
        drop(s);

        // بعد از باز شدن دوباره: پوشه و شناسهٔ دستگاه یکی است
        let s = UserStore::open(&user).unwrap();
        let info = s.sync_info().unwrap();
        assert_eq!(info.dir.as_deref(), Some(drive_text.as_str()));
        assert_eq!(info.file_name, file_name);
        let rep = s.export_sync().unwrap();
        assert_eq!(rep.file, file_name);
        assert!(drive.join(&file_name).is_file());
        assert!(s.set_sync_dir(None).unwrap().dir.is_none());
        assert!(s.set_sync_dir(Some("   ".into())).unwrap().dir.is_none());
    }

    #[test]
    fn exporting_to_a_missing_folder_is_a_clear_error() {
        let (root, _shared, a, _b) = two_devices();
        let missing = root.path().join("nope");
        assert!(matches!(a.export_sync_to(&missing), Err(Error::Sync(_))));
        assert!(matches!(a.import_sync_from(&missing), Err(Error::Sync(_))));
    }

    #[test]
    fn names_are_safe_for_file_systems() {
        assert_eq!(clean_device_name("DESKTOP-AB12"), "DESKTOP-AB12");
        assert_eq!(clean_device_name("لپ‌تاپ من"), "device");
        assert_eq!(clean_device_name(""), "device");
        assert_eq!(clean_device_name("my laptop (2)/x"), "my-laptop--2--x");
        assert_eq!(clean_device_name(&"a".repeat(40)).len(), 24);
        assert_eq!(sync_file_name("PC", "ab12cd34"), "dehkhoda-sync-PC-ab12cd34.json");
        let id = new_device_id();
        assert!(id.len() == 8 && id.chars().all(|c| c.is_ascii_hexdigit()), "{id}");
        assert_ne!(new_device_id(), new_device_id());
    }

    #[test]
    fn export_file_has_the_documented_shape() {
        let (_root, shared, a, _b) = two_devices();
        a.toggle_favorite(w("x"), 10).unwrap();
        a.toggle_favorite(w("x"), 20).unwrap();
        a.record_visit(w("h"), 30).unwrap();
        let rep = a.export_sync_to(&shared).unwrap();
        let v: serde_json::Value = serde_json::from_slice(&fs::read(shared.join(rep.file)).unwrap()).unwrap();
        assert_eq!(v["format"], "dehkhoda-sync");
        assert_eq!(v["version"], 1);
        assert!(v["deviceId"].as_str().unwrap().len() == 8);
        assert!(v["exportedAt"].as_i64().unwrap() > 0);
        assert_eq!(v["favorites"]["removed"][0]["wordNorm"], "x");
        assert_eq!(v["history"]["items"][0]["wordNorm"], "h");
    }

    // ---- ذخیرهٔ خودکار ----

    /// منتظر می‌ماند تا شرط برقرار شود (حداکثر ۲۰ ثانیه؛ تست‌ها به زمان‌بندی دقیق وابسته نیستند)
    fn wait_until(what: &str, mut cond: impl FnMut() -> bool) {
        let start = Instant::now();
        while !cond() {
            assert!(start.elapsed() < Duration::from_secs(20), "timed out waiting for: {what}");
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    struct AutoRig {
        _root: tempfile::TempDir,
        user_dir: PathBuf,
        drive: PathBuf,
        store: Arc<UserStore>,
    }

    fn auto_rig(debounce_ms: u64) -> AutoRig {
        let root = tempfile::tempdir().unwrap();
        let user_dir = root.path().join("u");
        let drive = root.path().join("drive");
        fs::create_dir_all(&drive).unwrap();
        let store = Arc::new(UserStore::open(&user_dir).unwrap());
        store.spawn_auto_export_with(Duration::from_millis(debounce_ms), Duration::from_secs(5)).unwrap();
        AutoRig { _root: root, user_dir, drive, store }
    }

    impl AutoRig {
        fn choose_folder(&self) {
            self.store.set_sync_dir(Some(self.drive.to_string_lossy().into_owned())).unwrap();
        }
        /// محتوای فایل این دستگاه در پوشه (`None` = هنوز نوشته نشده)
        fn file(&self) -> Option<SyncFile> {
            let name = self.store.sync_info().ok()?.file_name;
            read_sync_file(&self.drive.join(name)).ok()
        }
    }

    #[test]
    fn choosing_a_folder_and_then_changing_things_saves_by_itself() {
        let rig = auto_rig(30);
        rig.choose_folder();
        wait_until("the first file", || rig.file().is_some());
        rig.store.toggle_favorite(w("x"), 100).unwrap();
        rig.store.record_visit(w("h"), 101).unwrap();
        wait_until("the changes in the file", || rig.file().map(|f| f.favorites.items.len() == 1 && f.history.items.len() == 1).unwrap_or(false));
        let info = rig.store.sync_info().unwrap();
        assert!(info.auto_export);
        assert_eq!(info.last_auto.map(|s| s.error), Some(None));
    }

    #[test]
    fn removals_and_clearing_are_saved_too() {
        let rig = auto_rig(30);
        rig.choose_folder();
        rig.store.toggle_favorite(w("x"), 100).unwrap();
        rig.store.record_visit(w("h"), 101).unwrap();
        wait_until("added", || rig.file().map(|f| f.favorites.items.len() == 1 && f.history.items.len() == 1).unwrap_or(false));
        rig.store.toggle_favorite(w("x"), 200).unwrap();
        rig.store.clear_history().unwrap();
        wait_until("removed", || {
            rig.file().map(|f| f.favorites.items.is_empty() && f.favorites.removed.len() == 1 && f.history.items.is_empty() && f.history.cleared_at > 0).unwrap_or(false)
        });
    }

    #[test]
    fn a_burst_of_changes_becomes_a_few_writes() {
        let rig = auto_rig(250);
        rig.choose_folder();
        wait_until("the first file", || rig.file().is_some());
        let shared = rig.store.auto_shared();
        let before = shared.exports();
        for i in 0..30 {
            rig.store.record_visit(w(&format!("w{i}")), 1_000 + i).unwrap();
        }
        wait_until("all 30 in the file", || rig.file().map(|f| f.history.items.len() == 30).unwrap_or(false));
        let writes = shared.exports() - before;
        assert!((1..=4).contains(&writes), "{writes} writes for 30 quick changes");
    }

    #[test]
    fn turning_it_off_stops_the_writes_and_turning_it_on_writes_the_current_state() {
        let rig = auto_rig(20);
        rig.choose_folder();
        wait_until("the first file", || rig.file().is_some());
        assert!(!rig.store.set_auto_export(false).unwrap().auto_export);
        std::thread::sleep(Duration::from_millis(200)); // اگر نوشتنی هنوز در جریان بود تمام شود
        let shared = rig.store.auto_shared();
        let n = shared.exports();
        rig.store.toggle_favorite(w("x"), 5).unwrap();
        std::thread::sleep(Duration::from_millis(300));
        assert_eq!(shared.exports(), n, "خاموش است؛ نباید چیزی نوشته شود");
        assert!(rig.file().unwrap().favorites.items.is_empty());
        assert!(rig.store.set_auto_export(true).unwrap().auto_export);
        wait_until("the saved favorite", || rig.file().map(|f| f.favorites.items.len() == 1).unwrap_or(false));
    }

    #[test]
    fn the_off_choice_survives_a_restart() {
        let rig = auto_rig(20);
        rig.choose_folder();
        wait_until("the first file", || rig.file().is_some());
        rig.store.set_auto_export(false).unwrap();
        let AutoRig { _root, user_dir, drive, store } = rig;
        drop(store);
        let again = Arc::new(UserStore::open(&user_dir).unwrap());
        again.spawn_auto_export_with(Duration::from_millis(20), Duration::from_secs(5)).unwrap();
        let info = again.sync_info().unwrap();
        assert!(!info.auto_export && info.dir.is_some());
        let before = again.auto_shared().exports();
        again.toggle_favorite(w("z"), 9).unwrap();
        std::thread::sleep(Duration::from_millis(250));
        assert_eq!(again.auto_shared().exports(), before);
        again.export_sync().unwrap(); // دستی همچنان کار می‌کند
        assert_eq!(list_sync_files(&drive).unwrap().len(), 1);
        drop(_root);
    }

    #[test]
    fn nothing_happens_without_a_folder() {
        let rig = auto_rig(20);
        rig.store.toggle_favorite(w("x"), 1).unwrap();
        rig.store.record_visit(w("h"), 2).unwrap();
        std::thread::sleep(Duration::from_millis(200));
        assert_eq!(rig.store.auto_shared().exports(), 0);
        let info = rig.store.sync_info().unwrap();
        assert!(info.last_auto.is_none() && info.dir.is_none() && info.auto_export);
        assert!(list_sync_files(&rig.drive).unwrap().is_empty());
    }

    #[test]
    fn a_missing_folder_is_reported_and_saving_resumes_when_it_is_back() {
        let rig = auto_rig(20);
        rig.choose_folder();
        wait_until("the first file", || rig.file().is_some());
        fs::remove_dir_all(&rig.drive).unwrap();
        rig.store.toggle_favorite(w("x"), 10).unwrap();
        wait_until("an error status", || rig.store.sync_info().unwrap().last_auto.map(|s| s.error.is_some()).unwrap_or(false));
        let err = rig.store.sync_info().unwrap().last_auto.unwrap().error.unwrap();
        assert!(err.contains("پوشه"), "{err}");
        fs::create_dir_all(&rig.drive).unwrap();
        rig.store.toggle_favorite(w("y"), 11).unwrap();
        wait_until("saving works again", || rig.file().map(|f| f.favorites.items.len() == 2).unwrap_or(false));
        assert_eq!(rig.store.sync_info().unwrap().last_auto.unwrap().error, None);
    }

    #[test]
    fn flush_writes_pending_changes_immediately_and_only_once() {
        let rig = auto_rig(60_000); // نخِ پس‌زمینه تا چند ثانیه نمی‌نویسد
        rig.choose_folder();
        rig.store.toggle_favorite(w("x"), 7).unwrap();
        assert!(rig.file().is_none());
        rig.store.flush_auto_export();
        assert_eq!(rig.file().unwrap().favorites.items.len(), 1);
        assert_eq!(rig.store.auto_shared().exports(), 1);
        rig.store.flush_auto_export(); // دیگر چیزی در انتظار نیست
        assert_eq!(rig.store.auto_shared().exports(), 1);
    }

    #[test]
    fn an_import_that_changes_things_is_saved_too() {
        let rig = auto_rig(30);
        rig.choose_folder();
        let b = UserStore::open(rig._root.path().join("b")).unwrap();
        b.toggle_favorite(w("y"), 50).unwrap();
        b.export_sync_to(&rig.drive).unwrap();
        wait_until("this device's first file", || rig.file().is_some());
        let r = rig.store.import_sync().unwrap();
        assert_eq!(r.favorites_added, 1);
        wait_until("this device's file now holds the imported favorite", || rig.file().map(|f| f.favorites.items.len() == 1).unwrap_or(false));
    }

    #[test]
    fn the_background_thread_does_not_keep_the_store_alive() {
        let rig = auto_rig(20);
        let weak = Arc::downgrade(&rig.store);
        let AutoRig { _root, store, .. } = rig;
        drop(store);
        assert!(weak.upgrade().is_none());
        drop(_root);
    }
}
