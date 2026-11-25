$ErrorActionPreference = 'Stop'
$projectRoot = Split-Path -Parent $PSScriptRoot
$manifest = Join-Path $projectRoot 'linux-server\Cargo.toml'
$source = Join-Path $projectRoot 'linux-server\target\release\tunnel-helper.exe'
$outputDir = Join-Path $projectRoot 'dist\portable\build-latest\windows-web'
$output = Join-Path $outputDir 'tunnel-helper-web.exe'

cargo build --locked --release --manifest-path $manifest
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
New-Item -ItemType Directory -Path $outputDir -Force | Out-Null
Copy-Item -LiteralPath $source -Destination $output -Force
Write-Output "Windows web binary: $output"
