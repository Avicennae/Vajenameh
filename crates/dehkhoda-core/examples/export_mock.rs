//! نمونه‌داده برای حالت توسعهٔ مرورگری رابط (VITE_MOCK=1): مدخل‌های واقعی با خروجی واقعی sanitizer.
//!
//!     cargo run -p dehkhoda-core --example export_mock -- src-tauri/resources/dehkhoda.sqlite src/api/mockData.json آب کتاب ...

use dehkhoda_core::Database;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let db = Database::open(args.next().ok_or("usage: export_mock <db> <out.json> <word>...")?)?;
    let out = args.next().ok_or("missing output path")?;
    let mut groups = Vec::new();
    for word in args {
        match db.get_entry_group_paged(&word, None, 0, 1000)? {
            Some(g) if g.total <= 12 => groups.push(g),
            Some(g) => eprintln!("skipped {word}: {} entries (mock keeps groups of at most 12)", g.total),
            None => eprintln!("not found: {word}"),
        }
    }
    std::fs::write(&out, serde_json::to_string(&groups)?)?;
    eprintln!("wrote {} groups to {out}", groups.len());
    Ok(())
}
