use thiserror::Error;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Error)]
pub enum Error {
    #[error("database error: {0}")]
    Sqlite(rusqlite::Error),
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("incompatible database: {0}")]
    IncompatibleDb(String),
    #[error("full-text search is not available in this database")]
    NoFullText,
    #[error("search cancelled")]
    Cancelled,
    /// خطای همگام‌سازی با پوشه؛ متنش فارسی و آمادهٔ نمایش به کاربر است.
    #[error("{0}")]
    Sync(String),
}

impl From<rusqlite::Error> for Error {
    fn from(e: rusqlite::Error) -> Self {
        // وقتی جست‌وجوی تازه‌تری شروع شود، progress handler پرس‌وجوی قبلی را قطع می‌کند.
        if let rusqlite::Error::SqliteFailure(f, _) = &e {
            if f.code == rusqlite::ErrorCode::OperationInterrupted {
                return Error::Cancelled;
            }
        }
        Error::Sqlite(e)
    }
}
