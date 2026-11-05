# Which crop origins/margins reproduce the full decode exactly (A7R IVA showed a one-row shift
# at some origins)? Uses the OpenMP build (pixels verified identical to the baseline).
# Results: D:\PhotoWorkflowData\analysis\raw-crops-<ts>\
$ErrorActionPreference = 'Stop'
$repo = (Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path
$raw = 'D:\PhotoWorkflowData\RAW'
$out = "D:\PhotoWorkflowData\analysis\raw-crops-$(Get-Date -Format yyyyMMdd-HHmmss)"
New-Item -ItemType Directory -Force $out | Out-Null
Set-Location $repo
$env:VCPKG_INSTALLED_DIR = "$repo\vcpkg_installed_openmp"
$env:CARGO_TARGET_DIR = "$repo\target-openmp"
cargo build -p raw-decode-bench --profile profiling
$rc = $LASTEXITCODE
Remove-Item Env:VCPKG_INSTALLED_DIR
Remove-Item Env:CARGO_TARGET_DIR
if ($rc -ne 0) { throw 'build failed' }
$files = @(
  (Get-ChildItem $raw -Filter 'TN1004*.ARW' | Sort-Object Name | Select-Object -First 1).FullName,
  (Get-ChildItem $raw -Filter 'TN0059*.ARW' | Sort-Object Name | Select-Object -First 1).FullName,
  (Get-ChildItem $raw -Filter '*.RW2' | Sort-Object Name | Select-Object -First 1).FullName
) | Where-Object { $_ }
& "$repo\target-openmp\profiling\raw-decode-bench.exe" crops @files --out "$out\crops.csv" | Tee-Object "$out\crops.txt"
if ($LASTEXITCODE -ne 0) { throw 'crops failed' }
Write-Host "`nResults in $out" -ForegroundColor Green
