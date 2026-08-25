$Root = Split-Path -Parent $MyInvocation.MyCommand.Path
$ProjectRoot = Split-Path -Parent $Root
$NodeDir = Join-Path $ProjectRoot ".tools\node-v22.18.0-win-x64"
$CargoHome = Join-Path $ProjectRoot ".tools\cargo"
$RustupHome = Join-Path $ProjectRoot ".tools\rustup"
$NpmCache = Join-Path $ProjectRoot ".tools\npm-cache"

$env:CARGO_HOME = $CargoHome
$env:RUSTUP_HOME = $RustupHome
$env:npm_config_cache = $NpmCache
$env:RUSTUP_DIST_SERVER = "https://rsproxy.cn"
$env:RUSTUP_UPDATE_ROOT = "https://rsproxy.cn/rustup"
$env:PATH = "$NodeDir;$CargoHome\bin;$env:PATH"

Write-Host "Node: $NodeDir"
Write-Host "Cargo home: $CargoHome"
Write-Host "Rustup home: $RustupHome"
Write-Host "npm cache: $NpmCache"
