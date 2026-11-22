# ساخت نصب‌کنندهٔ ویندوز با یک دستور (PowerShell). از ریشهٔ پروژه اجرا کنید:
#     powershell -ExecutionPolicy Bypass -File scripts\build-windows.ps1
# اگر چیزی نصب نبود، می‌گوید کدام و از کجا؛ هیچ‌چیز را خودش نصب نمی‌کند.
# توجه: این اسکریپت را روی ویندوز واقعی نمی‌توانستم اجرا کنم؛ دستورهای دستی README مرجع‌اند.

$ErrorActionPreference = "Stop"
Set-Location (Join-Path $PSScriptRoot "..")

function Need($cmd, $where) {
    if (-not (Get-Command $cmd -ErrorAction SilentlyContinue)) {
        Write-Host "ابزار '$cmd' پیدا نشد. از اینجا نصبش کنید: $where" -ForegroundColor Red
        exit 1
    }
}
function Step($title, [scriptblock]$body) {
    Write-Host "`n=== $title ===" -ForegroundColor Cyan
    & $body
    if ($LASTEXITCODE -ne 0) { Write-Host "این مرحله ناموفق بود: $title" -ForegroundColor Red; exit 1 }
}

Need "node"  "https://nodejs.org (نسخهٔ LTS)"
Need "npm"   "https://nodejs.org (همراه Node.js نصب می‌شود)"
Need "cargo" "https://rustup.rs (و ابزارهای C++ ویژوال استودیو؛ README را ببینید)"
$py = if (Get-Command python -ErrorAction SilentlyContinue) { "python" } elseif (Get-Command py -ErrorAction SilentlyContinue) { "py" } else { $null }
if (-not $py) { Write-Host "پایتون پیدا نشد: https://www.python.org/downloads (گزینهٔ Add python.exe to PATH را بزنید)" -ForegroundColor Red; exit 1 }

Step "۱) ساخت پایگاه داده (دانلود و تبدیل دامپ‌ها؛ چند دقیقه)" { & $py tools\build_db.py }
Step "۲) نصب وابستگی‌های رابط" { npm install }
Step "۳) ساخت نصب‌کننده (بار اول طولانی است؛ Rust همه‌چیز را کامپایل می‌کند)" { npm run tauri:build }

$out = Join-Path (Get-Location) "target\release\bundle\nsis"
Write-Host "`nتمام شد. نصب‌کننده اینجاست:" -ForegroundColor Green
Get-ChildItem $out -Filter *.exe | ForEach-Object { Write-Host "  $($_.FullName)" }
if (Test-Path $out) { Start-Process explorer.exe $out }
