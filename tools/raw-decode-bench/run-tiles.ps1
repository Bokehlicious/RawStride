# M2, step 2: several regions after one shared unpack, with the BASELINE LibRaw (no OpenMP).
# Uses the existing vcpkg_installed\ (unchanged). Results: D:\PhotoWorkflowData\analysis\raw-tiles-<ts>\
$ErrorActionPreference = 'Stop'
$repo = (Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path
$out = "D:\PhotoWorkflowData\analysis\raw-tiles-$(Get-Date -Format yyyyMMdd-HHmmss)"
New-Item -ItemType Directory -Force $out | Out-Null
Set-Location $repo
Remove-Item Env:VCPKG_INSTALLED_DIR -ErrorAction SilentlyContinue
Remove-Item Env:CARGO_TARGET_DIR -ErrorAction SilentlyContinue
cargo build -p raw-decode-bench --profile profiling
if ($LASTEXITCODE -ne 0) { throw 'build failed' }
$exe = "$repo\target\profiling\raw-decode-bench.exe"
# Record exactly what was measured.
Copy-Item "$repo\vcpkg_installed\vcpkg\status" "$out\vcpkg-status-baseline.txt"
Get-Content "$repo\vcpkg_installed\x64-windows-static-md\share\libraw\vcpkg_abi_info.txt" | Set-Content "$out\libraw-abi-baseline.txt"
(Get-FileHash $exe -Algorithm SHA256).Hash | Set-Content "$out\exe-sha256-baseline.txt"
& "$PSScriptRoot\suite.ps1" -Exe $exe -Out $out -Steps tiles, ptiles, half
Write-Host "`nResults in $out" -ForegroundColor Green
