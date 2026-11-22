//! هستهٔ بدون-وابستگی-به-رابط برنامهٔ «لغت‌نامهٔ دهخدا (آفلاین)».
//!
//! * [`normalize`] — نرمال‌سازی فارسی (آینهٔ `tools/fa_text.py`)
//! * [`db`] / [`search`] / [`fulltext`] — پایگاه SQLite فقط‌خواندنی، جست‌وجوی واژه و متن‌کامل
//! * [`snippet`] — برش و هایلایت خلاصهٔ نتایج متن‌کامل
//! * [`sanitize`] — پاک‌سازی HTML معنی‌ها
//! * [`userdata`] — تاریخچه، علاقه‌مندی‌ها و تنظیمات (فایل‌های جدا از دیتابیس)
//! * [`sync`] — همگام‌سازی دستی علاقه‌مندی‌ها و تاریخچه از راه یک پوشه (خروجی/ورودی)

pub mod db;
pub mod error;
pub mod fulltext;
pub mod normalize;
pub mod sanitize;
pub mod search;
pub mod snippet;
pub mod sync;
pub mod userdata;

pub use db::{Database, DbInfo};
pub use error::{Error, Result};
pub use normalize::{fold, normalize, normalize_fold, NORM_VERSION};
