//! پوستهٔ Tauri: پایگاه دادهٔ همراه برنامه و پوشهٔ دادهٔ کاربر را باز می‌کند و دستورهای رابط را به
//! هستهٔ `dehkhoda-core` وصل می‌کند. منطق جست‌وجو اینجا نیست؛ همه‌اش در `crates/dehkhoda-core` است و
//! آنجا تست می‌شود. این برنامه هیچ کد شبکه‌ای ندارد. تنها افزونهٔ Tauri که به کار رفته پنجرهٔ «انتخاب پوشه»
//! برای همگام‌سازی است؛ فقط از سمت Rust صدا زده می‌شود و هیچ مجوزی از آن به رابط داده نشده است.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use dehkhoda_core::fulltext::FtsPage;
use dehkhoda_core::search::{EntryGroup, WordHit, DEFAULT_LIMIT};
use dehkhoda_core::sync::{ExportReport, ImportReport, SyncInfo};
use dehkhoda_core::userdata::{now_ms, FavoriteItem, HistoryItem, Settings, UserStore, WordRef};
use dehkhoda_core::{Database, DbInfo, Error};
use serde::Serialize;
use tauri::{AppHandle, Manager, State};
use tauri_plugin_dialog::{DialogExt, FilePath};

/// نام فایل پایگاه داده کنار برنامه (نگاشت در tauri.conf.json → bundle.resources)
pub const DB_FILE: &str = "dehkhoda.sqlite";
/// تعداد نتایج هر صفحهٔ جست‌وجوی متن‌کامل
const FTS_PAGE: usize = 50;

struct AppState {
    db: Result<Arc<Database>, String>,
    user: Result<Arc<UserStore>, String>,
}

// ───────────── پیدا کردن پایگاه داده ─────────────

/// مسیرهایی که به ترتیب بررسی می‌شوند: متغیر محیطی، کنار برنامهٔ نصب‌شده، و (فقط در حالت توسعه) پوشهٔ پروژه.
fn db_search_paths(env_override: Option<String>, resource_dir: Option<PathBuf>, dev_dir: Option<PathBuf>) -> Vec<PathBuf> {
    let mut paths = Vec::new();
    if let Some(p) = env_override.filter(|p| !p.trim().is_empty()) {
        paths.push(PathBuf::from(p));
    }
    if let Some(dir) = resource_dir {
        paths.push(dir.join(DB_FILE));
    }
    if let Some(dir) = dev_dir {
        paths.push(dir.join("resources").join(DB_FILE));
    }
    paths
}

/// ویندوز گاهی مسیر را با پیشوند «\\?\» می‌دهد؛ برای SQLite و پیام‌های خطا مسیر ساده بهتر است.
fn simplify(path: PathBuf) -> PathBuf {
    let text = path.to_string_lossy().into_owned();
    match text.strip_prefix(r"\\?\") {
        Some(rest) if !rest.starts_with("UNC\\") => PathBuf::from(rest),
        _ => path,
    }
}

fn pick_db(paths: &[PathBuf]) -> Result<PathBuf, String> {
    paths.iter().find(|p| p.is_file()).cloned().map(simplify).ok_or_else(|| {
        let tried: Vec<String> = paths.iter().map(|p| format!("  • {}", p.display())).collect();
        format!("فایل {DB_FILE} پیدا نشد. این مسیرها بررسی شد:\n{}", tried.join("\n"))
    })
}

fn locate_db(app: &AppHandle) -> Result<PathBuf, String> {
    let dev_dir = if cfg!(debug_assertions) { Some(PathBuf::from(env!("CARGO_MANIFEST_DIR"))) } else { None };
    let paths = db_search_paths(std::env::var("DEHKHODA_DB").ok(), app.path().resource_dir().ok(), dev_dir);
    pick_db(&paths)
}

fn open_user_store(app: &AppHandle) -> Result<Arc<UserStore>, String> {
    let dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    UserStore::open(dir.join("user")).map(Arc::new).map_err(|e| e.to_string())
}

// ───────────── دستورها ─────────────

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Bootstrap {
    info: Option<DbInfo>,
    db_error: Option<String>,
    settings: Settings,
    history: Vec<HistoryItem>,
    favorites: Vec<FavoriteItem>,
    warnings: Vec<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct FavoriteToggle {
    is_favorite: bool,
    favorites: Vec<FavoriteItem>,
}

fn db(state: &State<'_, AppState>) -> Result<Arc<Database>, String> {
    state.db.clone()
}

fn user(state: &State<'_, AppState>) -> Result<Arc<UserStore>, String> {
    state.user.clone().map_err(|e| format!("ذخیره‌سازی تاریخچه و تنظیمات در دسترس نیست: {e}"))
}

/// پیام «cancelled» را رابط بی‌صدا نادیده می‌گیرد (جست‌وجوی تازه‌تر جایگزین شده است).
fn err(e: Error) -> String {
    match e {
        Error::Cancelled => "cancelled".to_string(),
        other => other.to_string(),
    }
}

/// همهٔ وضعیت اولیه یک‌جا؛ اگر پایگاه باز نشده باشد رابط صفحهٔ راهنما نشان می‌دهد.
#[tauri::command(async)]
fn bootstrap(state: State<'_, AppState>) -> Bootstrap {
    let (info, db_error) = match &state.db {
        Ok(db) => (Some(db.info().clone()), None),
        Err(e) => (None, Some(e.clone())),
    };
    let mut warnings = Vec::new();
    let (settings, history, favorites) = match &state.user {
        Ok(u) => {
            warnings.extend(u.warnings().iter().cloned());
            (u.settings(), u.history(), u.favorites())
        }
        Err(e) => {
            warnings.push(format!("ذخیره‌سازی تاریخچه و تنظیمات در دسترس نیست: {e}"));
            (Settings::default(), Vec::new(), Vec::new())
        }
    };
    Bootstrap { info, db_error, settings, history, favorites, warnings }
}

#[tauri::command(async)]
fn search_words(state: State<'_, AppState>, query: String) -> Result<Vec<WordHit>, String> {
    db(&state)?.search_words(&query, DEFAULT_LIMIT).map_err(err)
}

#[tauri::command(async)]
fn search_fulltext(state: State<'_, AppState>, query: String, offset: usize) -> Result<FtsPage, String> {
    db(&state)?.search_fulltext(&query, offset, FTS_PAGE).map_err(err)
}

#[tauri::command(async)]
fn get_entry_group(state: State<'_, AppState>, key: String, focus_id: Option<i64>, offset: usize) -> Result<Option<EntryGroup>, String> {
    db(&state)?.get_entry_group(&key, focus_id, offset).map_err(err)
}

#[tauri::command(async)]
fn record_visit(state: State<'_, AppState>, word_norm: String, word: String) -> Result<Vec<HistoryItem>, String> {
    user(&state)?.record_visit(WordRef { word_norm, word }, now_ms()).map_err(err)
}

#[tauri::command(async)]
fn clear_history(state: State<'_, AppState>) -> Result<Vec<HistoryItem>, String> {
    let u = user(&state)?;
    u.clear_history().map_err(err)?;
    Ok(u.history())
}

#[tauri::command(async)]
fn toggle_favorite(state: State<'_, AppState>, word_norm: String, word: String) -> Result<FavoriteToggle, String> {
    let (is_favorite, favorites) = user(&state)?.toggle_favorite(WordRef { word_norm, word }, now_ms()).map_err(err)?;
    Ok(FavoriteToggle { is_favorite, favorites })
}

#[tauri::command(async)]
fn save_settings(state: State<'_, AppState>, settings: Settings) -> Result<Settings, String> {
    user(&state)?.set_settings(settings).map_err(err)
}

// ───────────── همگام‌سازی دستی از راه یک پوشه ─────────────

/// مسیر پوشهٔ انتخاب‌شده در پنجرهٔ بومی (بدون پیشوند «\\?\» ویندوز)؛ آدرس غیرفایلی = `None`.
fn folder_text(picked: FilePath) -> Option<String> {
    picked.into_path().ok().map(|p| simplify(p).to_string_lossy().into_owned())
}

/// پنجرهٔ بومی انتخاب پوشه؛ `None` یعنی کاربر منصرف شد. فقط مسیر برمی‌گرداند و چیزی نمی‌خواند یا نمی‌نویسد.
#[tauri::command(async)]
fn pick_sync_folder(app: AppHandle, state: State<'_, AppState>) -> Result<Option<String>, String> {
    let current = user(&state).ok().and_then(|u| u.sync_dir());
    let mut dialog = app.dialog().file().set_title("پوشهٔ همگام‌سازی را انتخاب کنید");
    if let Some(dir) = current.filter(|d| Path::new(d).is_dir()) {
        dialog = dialog.set_directory(dir);
    }
    if let Some(window) = app.get_webview_window("main") {
        dialog = dialog.set_parent(&window);
    }
    Ok(dialog.blocking_pick_folder().and_then(folder_text))
}

#[tauri::command(async)]
fn get_sync_info(state: State<'_, AppState>) -> Result<SyncInfo, String> {
    user(&state)?.sync_info().map_err(err)
}

#[tauri::command(async)]
fn set_sync_dir(state: State<'_, AppState>, dir: Option<String>) -> Result<SyncInfo, String> {
    user(&state)?.set_sync_dir(dir).map_err(err)
}

#[tauri::command(async)]
fn sync_export(state: State<'_, AppState>) -> Result<ExportReport, String> {
    user(&state)?.export_sync().map_err(err)
}

#[tauri::command(async)]
fn sync_import(state: State<'_, AppState>) -> Result<ImportReport, String> {
    user(&state)?.import_sync().map_err(err)
}

/// ذخیرهٔ خودکار (پس از هر تغییرِ علاقه‌مندی‌ها یا تاریخچه) را روشن/خاموش می‌کند.
#[tauri::command(async)]
fn set_auto_export(state: State<'_, AppState>, enabled: bool) -> Result<SyncInfo, String> {
    user(&state)?.set_auto_export(enabled).map_err(err)
}

pub fn run() {
    let app = tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let handle = app.handle();
            let db = locate_db(handle).and_then(|path| Database::open(&path).map(Arc::new).map_err(|e| e.to_string()));
            let user = open_user_store(handle);
            if let Ok(store) = &user {
                // ذخیرهٔ خودکار علاقه‌مندی‌ها و تاریخچه در پوشهٔ همگام‌سازی (اگر انتخاب شده باشد)
                let _ = store.spawn_auto_export();
            }
            app.manage(AppState { db, user });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            bootstrap,
            search_words,
            search_fulltext,
            get_entry_group,
            record_visit,
            clear_history,
            toggle_favorite,
            save_settings,
            pick_sync_folder,
            get_sync_info,
            set_sync_dir,
            sync_export,
            sync_import,
            set_auto_export
        ])
        .build(tauri::generate_context!())
        .expect("error while building the Dehkhoda application");
    app.run(|handle, event| {
        // هنگام بسته شدن برنامه، تغییرهای هنوز ننوشتهٔ «ذخیرهٔ خودکار» را همین حالا می‌نویسیم
        if let tauri::RunEvent::Exit = event {
            if let Some(state) = handle.try_state::<AppState>() {
                if let Ok(store) = &state.user {
                    store.flush_auto_export();
                }
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn touch(dir: &Path, name: &str) -> PathBuf {
        let p = dir.join(name);
        std::fs::write(&p, b"x").unwrap();
        p
    }

    #[test]
    fn search_order_is_env_then_resources_then_dev_folder() {
        let paths = db_search_paths(Some("/custom/d.sqlite".into()), Some("/app/res".into()), Some("/proj".into()));
        assert_eq!(
            paths,
            vec![PathBuf::from("/custom/d.sqlite"), PathBuf::from("/app/res").join(DB_FILE), PathBuf::from("/proj/resources").join(DB_FILE)]
        );
        // متغیر محیطی خالی نادیده گرفته می‌شود؛ حالت release مسیر توسعه ندارد
        assert_eq!(db_search_paths(Some("  ".into()), Some("/app".into()), None), vec![PathBuf::from("/app").join(DB_FILE)]);
    }

    #[test]
    fn first_existing_file_wins() {
        let a = tempfile::tempdir().unwrap();
        let b = tempfile::tempdir().unwrap();
        let in_b = touch(b.path(), DB_FILE);
        assert_eq!(pick_db(&[a.path().join(DB_FILE), in_b.clone()]).unwrap(), in_b);
        let first = touch(a.path(), DB_FILE);
        assert_eq!(pick_db(&[first.clone(), in_b]).unwrap(), first);
    }

    #[test]
    fn missing_database_message_lists_every_place_that_was_tried() {
        let msg = pick_db(&[PathBuf::from("/x/dehkhoda.sqlite"), PathBuf::from("/y/dehkhoda.sqlite")]).unwrap_err();
        assert!(msg.contains("/x/dehkhoda.sqlite") && msg.contains("/y/dehkhoda.sqlite"), "{msg}");
        assert!(pick_db(&[PathBuf::from("/")]).is_err()); // پوشه فایل نیست
    }

    #[test]
    fn windows_verbatim_prefix_is_removed_but_unc_paths_are_kept() {
        assert_eq!(simplify(PathBuf::from(r"\\?\C:\Users\علی\Dehkhoda\dehkhoda.sqlite")), PathBuf::from(r"C:\Users\علی\Dehkhoda\dehkhoda.sqlite"));
        assert_eq!(simplify(PathBuf::from(r"\\?\UNC\server\share\d.sqlite")), PathBuf::from(r"\\?\UNC\server\share\d.sqlite"));
        assert_eq!(simplify(PathBuf::from("/usr/lib/d.sqlite")), PathBuf::from("/usr/lib/d.sqlite"));
        assert_eq!(simplify(PathBuf::from(r"C:\plain\d.sqlite")), PathBuf::from(r"C:\plain\d.sqlite"));
    }

    #[test]
    fn picked_folder_becomes_a_plain_path_text() {
        assert_eq!(folder_text(FilePath::Path(PathBuf::from(r"\\?\C:\Users\علی\Drive\Dehkhoda"))).as_deref(), Some(r"C:\Users\علی\Drive\Dehkhoda"));
        assert_eq!(folder_text(FilePath::Path(PathBuf::from("/home/ali/Drive"))).as_deref(), Some("/home/ali/Drive"));
    }

    #[test]
    fn cancelled_is_a_stable_machine_readable_message() {
        assert_eq!(err(Error::Cancelled), "cancelled");
        assert_ne!(err(Error::NoFullText), "cancelled");
    }
}
