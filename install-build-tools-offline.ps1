$AppRoot = Split-Path -Parent $MyInvocation.MyCommand.Path
$ProjectRoot = Split-Path -Parent $AppRoot
$Layout = Join-Path $ProjectRoot "downloads\vs_buildtools_layout"
$Setup = Join-Path $Layout "vs_setup.exe"

if (-not (Test-Path -LiteralPath $Setup)) {
    Write-Host "Offline setup entry not found: $Setup" -ForegroundColor Red
    Write-Host "Please confirm downloads\vs_buildtools_layout exists and is complete." -ForegroundColor Yellow
    exit 1
}

Write-Host "Installing Microsoft C++ Build Tools from local offline layout." -ForegroundColor Cyan
Write-Host $Setup -ForegroundColor White
Write-Host "This step requires Administrator/UAC approval." -ForegroundColor Yellow

Start-Process -FilePath $Setup -ArgumentList @(
    "--noweb",
    "--quiet",
    "--wait",
    "--norestart",
    "--add", "Microsoft.VisualStudio.Workload.VCTools",
    "--includeRecommended"
) -Wait

Write-Host "Installer returned. Reopen PowerShell, then check cl.exe/link.exe." -ForegroundColor Green
