$AppRoot = Split-Path -Parent $MyInvocation.MyCommand.Path
Set-Location $AppRoot

. .\dev-env.ps1

$BuiltExe = Join-Path $AppRoot "src-tauri\target\release\tunnel-helper.exe"
if (Test-Path -LiteralPath $BuiltExe) {
    Remove-Item -LiteralPath $BuiltExe -Force
}

npm run build -- --no-bundle
if ($LASTEXITCODE -ne 0) {
    Write-Host "Build failed." -ForegroundColor Red
    exit $LASTEXITCODE
}

$PortableExe = Join-Path $AppRoot "tunnel-helper.exe"
$BackupDir = Join-Path $AppRoot "backups"

if (-not (Test-Path -LiteralPath $BuiltExe)) {
    Write-Host "Built executable not found: $BuiltExe" -ForegroundColor Red
    exit 1
}

if (Test-Path -LiteralPath $PortableExe) {
    if (-not (Test-Path -LiteralPath $BackupDir)) {
        New-Item -ItemType Directory -Path $BackupDir | Out-Null
    }

    $Stamp = Get-Date -Format "yyyyMMdd-HHmmss"
    $BackupExe = Join-Path $BackupDir "tunnel-helper-$Stamp.exe"
    Copy-Item -LiteralPath $PortableExe -Destination $BackupExe -Force
    Write-Host "Previous executable backed up:" -ForegroundColor Yellow
    Write-Host $BackupExe -ForegroundColor White
}

Copy-Item -LiteralPath $BuiltExe -Destination $PortableExe -Force
Write-Host "Portable executable generated:" -ForegroundColor Green
Write-Host $PortableExe -ForegroundColor White
