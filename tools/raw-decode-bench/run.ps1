# Runs the LibRaw decode measurements for milestone 2 (A7R IVA primary, A1 for comparison).
# Prerequisite (once, in the repo root): vcpkg install --triplet x64-windows-static-md
param([string]$Raw = 'D:\PhotoWorkflowData\RAW')
$ErrorActionPreference = 'Stop'
$out = "D:\PhotoWorkflowData\analysis\raw-decode-$(Get-Date -Format yyyyMMdd-HHmmss)"
New-Item -ItemType Directory -Force $out | Out-Null
Set-Location (Join-Path $PSScriptRoot '..\..')
cargo build -p raw-decode-bench --profile profiling
if ($LASTEXITCODE -ne 0) { throw 'build failed' }
$exe = '.\target\profiling\raw-decode-bench.exe'
$a7r = @(Get-ChildItem $Raw -Filter 'TN1004*.ARW' | Sort-Object Name | ForEach-Object FullName)
$a1  = @(Get-ChildItem $Raw -Filter 'TN0059*.ARW' | Sort-Object Name | ForEach-Object FullName)
& $exe single $a7r[0] $a1[0] --reps 3 --out "$out\single.csv" | Tee-Object "$out\single.txt"
& $exe parallel @a7r --threads 1,2,4,8 --out "$out\parallel-a7riva.csv" | Tee-Object "$out\parallel-a7riva.txt"
Write-Host "`nResults in $out" -ForegroundColor Green
