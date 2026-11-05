# M2, step 3: baseline LibRaw vs LibRaw with OpenMP, same files, same machine state, interleaved.
# Prerequisite (done by tools\m2-openmp.cmd): vcpkg_installed_openmp\ with libraw[openmp].
# The baseline vcpkg_installed\ and target\ are not changed. Undo: delete vcpkg_installed_openmp\
# and target-openmp\ (docs/libraw-build.md).
$ErrorActionPreference = 'Stop'
$repo = (Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path
$out = "D:\PhotoWorkflowData\analysis\raw-openmp-$(Get-Date -Format yyyyMMdd-HHmmss)"
New-Item -ItemType Directory -Force $out | Out-Null
Set-Location $repo

$ompRoot = "$repo\vcpkg_installed_openmp"
$status = Get-Content "$ompRoot\vcpkg\status" -Raw
if ($status -notmatch '(?ms)Package: libraw\s.*?Feature: openmp') { throw 'libraw[openmp] is not installed in vcpkg_installed_openmp' }

# Baseline build (unchanged configuration).
Remove-Item Env:VCPKG_INSTALLED_DIR -ErrorAction SilentlyContinue
Remove-Item Env:CARGO_TARGET_DIR -ErrorAction SilentlyContinue
cargo build -p raw-decode-bench --profile profiling
if ($LASTEXITCODE -ne 0) { throw 'baseline build failed' }
$exeSt = "$repo\target\profiling\raw-decode-bench.exe"

# OpenMP build, separate install root and target dir.
$env:VCPKG_INSTALLED_DIR = $ompRoot
$env:CARGO_TARGET_DIR = "$repo\target-openmp"
cargo build -p raw-decode-bench --profile profiling
$rc = $LASTEXITCODE
Remove-Item Env:VCPKG_INSTALLED_DIR
Remove-Item Env:CARGO_TARGET_DIR
if ($rc -ne 0) { throw 'OpenMP build failed' }
$exeOmp = "$repo\target-openmp\profiling\raw-decode-bench.exe"

# MSVC OpenMP runtime: use Windows' copy if present, else copy it next to the OpenMP exe only.
if (-not (Test-Path "$env:WINDIR\System32\vcomp140.dll")) {
  $vs = @("${env:ProgramFiles(x86)}\Microsoft Visual Studio", "$env:ProgramFiles\Microsoft Visual Studio") | Where-Object { Test-Path $_ }
  $dll = Get-ChildItem $vs -Recurse -Filter vcomp140.dll -ErrorAction SilentlyContinue |
    Where-Object { $_.FullName -match '\\x64\\' -and $_.FullName -match 'OpenMP' -and $_.FullName -notmatch 'debug' } |
    Select-Object -First 1
  if (-not $dll) { throw 'vcomp140.dll not found (Windows or VS Build Tools redist)' }
  Copy-Item $dll.FullName (Split-Path $exeOmp)
  "vcomp140.dll copied from $($dll.FullName)" | Tee-Object "$out\vcomp.txt"
} else {
  "vcomp140.dll from $env:WINDIR\System32" | Tee-Object "$out\vcomp.txt"
}

# Record exactly what was measured.
Copy-Item "$repo\vcpkg_installed\vcpkg\status" "$out\vcpkg-status-baseline.txt"
Copy-Item "$ompRoot\vcpkg\status" "$out\vcpkg-status-openmp.txt"
Get-Content "$repo\vcpkg_installed\x64-windows-static-md\share\libraw\vcpkg_abi_info.txt" | Set-Content "$out\libraw-abi-baseline.txt"
Get-Content "$ompRoot\x64-windows-static-md\share\libraw\vcpkg_abi_info.txt" | Set-Content "$out\libraw-abi-openmp.txt"
"baseline $((Get-FileHash $exeSt -Algorithm SHA256).Hash)`nopenmp   $((Get-FileHash $exeOmp -Algorithm SHA256).Hash)" | Set-Content "$out\exe-sha256.txt"
"OMP_NUM_THREADS=$env:OMP_NUM_THREADS (empty = all logical CPUs)" | Set-Content "$out\omp-env.txt"

# Same steps, alternating builds, so both see the same thermal and cache conditions.
foreach ($step in 'single', 'tiles', 'ptiles', 'half', 'full') {
  & "$PSScriptRoot\suite.ps1" -Exe $exeSt  -Out "$out\baseline" -Steps $step
  & "$PSScriptRoot\suite.ps1" -Exe $exeOmp -Out "$out\openmp"   -Steps $step
}
Write-Host "`nResults in $out" -ForegroundColor Green
