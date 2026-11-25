$ErrorActionPreference = 'Stop'
$AppRoot = Split-Path -Parent $MyInvocation.MyCommand.Path
Set-Location $AppRoot
. .\dev-env.ps1

npm run build -- --no-bundle
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

$BuiltExe = Join-Path $AppRoot 'src-tauri\target\release\tunnel-helper.exe'
if (-not (Test-Path -LiteralPath $BuiltExe)) { throw "Built executable not found: $BuiltExe" }
$OutputDir = Join-Path $AppRoot 'dist\portable\build-latest\windows-desktop'
New-Item -ItemType Directory -Path $OutputDir -Force | Out-Null
Copy-Item -LiteralPath $BuiltExe -Destination (Join-Path $OutputDir 'tunnel-helper.exe') -Force
Write-Host "Desktop build: $OutputDir\tunnel-helper.exe"
