//! باز کردن پایگاه (فقط‌خواندنی) و استخر کوچک اتصال‌ها.

use std::collections::HashMap;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicU64;
use std::sync::{Arc, Mutex};

use rusqlite::{Connection, OpenFlags};
use serde::Serialize;

use crate::normalize::NORM_VERSION;
use crate::{Error, Result};

/// نسخهٔ طرح‌واره‌ای که این کد می‌شناسد (`meta.schema_version`).
pub const SUPPORTED_SCHEMA_VERSION: u32 = 2;
const MAX_IDLE_CONNECTIONS: usize = 4;

/// اطلاعات ساخت پایگاه (جدول `meta`) برای صفحهٔ «دربارهٔ برنامه».
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DbInfo {
    pub schema_version: u32,
    pub norm_version: u32,
    pub has_fts: bool,
    pub row_count: u64,
    pub distinct_words: u64,
    pub source_url: String,
    pub source_commit: String,
    pub built_at: String,
}

pub struct Database {
    path: PathBuf,
    idle: Mutex<Vec<Connection>>,
    info: DbInfo,
    /// شمارندهٔ نسل جست‌وجوی متن‌کامل؛ با شروع جست‌وجوی جدید، جست‌وجوهای در حال اجرا قطع می‌شوند.
    pub(crate) fts_generation: Arc<AtomicU64>,
}

fn open_connection(path: &Path) -> Result<Connection> {
    // فقط‌خواندنی؛ بدون ساخت هیچ فایل جانبی (-journal/-wal) کنار فایل نصب‌شده
    let flags = OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX;
    let conn = Connection::open_with_flags(path, flags)?;
    conn.execute_batch("PRAGMA query_only=1; PRAGMA cache_size=-32768; PRAGMA mmap_size=268435456;")?;
    Ok(conn)
}

fn read_info(conn: &Connection) -> Result<DbInfo> {
    let has_meta: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name IN ('meta','words') GROUP BY type HAVING COUNT(*)=2)",
        [],
        |r| r.get(0),
    )?;
    if !has_meta {
        return Err(Error::IncompatibleDb("جدول words/meta پیدا نشد؛ فایل پایگاه دادهٔ لغت‌نامه نیست".into()));
    }
    let mut meta: HashMap<String, String> = HashMap::new();
    let mut st = conn.prepare("SELECT key, value FROM meta")?;
    for row in st.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))? {
        let (k, v) = row?;
        meta.insert(k, v);
    }
    let num = |k: &str| -> Result<u64> {
        meta.get(k)
            .and_then(|v| v.parse::<u64>().ok())
            .ok_or_else(|| Error::IncompatibleDb(format!("کلید meta.{k} وجود ندارد یا عدد نیست")))
    };
    let text = |k: &str| meta.get(k).cloned().unwrap_or_default();
    let info = DbInfo {
        schema_version: num("schema_version")? as u32,
        norm_version: num("norm_version")? as u32,
        has_fts: meta.get("has_fts").map(|v| v == "1").unwrap_or(false),
        row_count: num("row_count")?,
        distinct_words: num("distinct_words")?,
        source_url: text("source_url"),
        source_commit: text("source_commit"),
        built_at: text("built_at"),
    };
    if info.schema_version != SUPPORTED_SCHEMA_VERSION {
        return Err(Error::IncompatibleDb(format!(
            "نسخهٔ طرح‌واره {} است؛ این برنامه فقط {} را می‌شناسد",
            info.schema_version, SUPPORTED_SCHEMA_VERSION
        )));
    }
    if info.norm_version != NORM_VERSION {
        return Err(Error::IncompatibleDb(format!(
            "نسخهٔ نرمال‌سازی پایگاه ({}) با برنامه ({}) نمی‌خواند؛ پایگاه را دوباره بسازید",
            info.norm_version, NORM_VERSION
        )));
    }
    Ok(info)
}

impl Database {
    /// پایگاه را فقط‌خواندنی باز و سازگاری نسخه‌ها را بررسی می‌کند.
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref().to_path_buf();
        if !path.is_file() {
            return Err(Error::Io(io::Error::new(
                io::ErrorKind::NotFound,
                format!("فایل پایگاه داده پیدا نشد: {}", path.display()),
            )));
        }
        let conn = open_connection(&path)?;
        let info = read_info(&conn)?;
        Ok(Database { path, idle: Mutex::new(vec![conn]), info, fts_generation: Arc::new(AtomicU64::new(0)) })
    }

    pub fn info(&self) -> &DbInfo {
        &self.info
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// یک اتصال از استخر می‌گیرد (یا باز می‌کند)، کار را انجام می‌دهد و آن را پس می‌دهد.
    pub(crate) fn with_conn<T>(&self, f: impl FnOnce(&mut Connection) -> Result<T>) -> Result<T> {
        let pooled = self.idle.lock().unwrap_or_else(|e| e.into_inner()).pop();
        let mut conn = match pooled {
            Some(c) => c,
            None => open_connection(&self.path)?,
        };
        let out = f(&mut conn);
        let mut idle = self.idle.lock().unwrap_or_else(|e| e.into_inner());
        if idle.len() < MAX_IDLE_CONNECTIONS {
            idle.push(conn);
        }
        out
    }
}
