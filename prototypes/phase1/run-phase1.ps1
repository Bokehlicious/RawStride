# ADR 0008 phase 1: builds the prototypes and runs P0 (JPEG decoders) and P4b (demosaic methods).
# Started by run-phase1.cmd. Writes only to D:\PhotoWorkflowData\analysis\phase1 and to
# target\prototypes (build output). Installs nothing; the app's code and settings are not touched.
param(
    [string]$Raw = 'D:\PhotoWorkflowData\RAW',
    [string]$Out = 'D:\PhotoWorkflowData\analysis\phase1',
    [string]$Threads = 'all,1',
    # 'p0' = build and run P0 only (P4b results are left as they are).
    [string]$Only = 'all',
    [string]$P0Csv = 'p0-decode.csv'
)
# 'Continue': in Windows PowerShell 5.1, stderr from native tools (cargo progress) must not abort.
$ErrorActionPreference = 'Continue'
$here = Split-Path -Parent $MyInvocation.MyCommand.Path
$repo = Resolve-Path (Join-Path $here '..\..')
$target = Join-Path $repo 'target\prototypes'
$exe = Join-Path $target 'release\proto-phase1.exe'
New-Item -ItemType Directory -Force -Path $Out | Out-Null
$log = Join-Path $Out 'run-phase1.log'
function Log($m) { $line = "$(Get-Date -Format 'yyyy-MM-dd HH:mm:ss') $m"; Write-Host $line; Add-Content -Path $log -Value $line }

Log "=== phase 1 start ==="
# Same crate versions as the app: seed the prototype lock file from the root one (first run only).
$lock = Join-Path $here 'Cargo.lock'
if (-not (Test-Path $lock)) { Copy-Item (Join-Path $repo 'Cargo.lock') $lock -ErrorAction Stop; Log "seeded Cargo.lock from the root lock" }

Log "[1/4] build (release, target\prototypes)"
Push-Location $here
try {
    cmd /c "cargo build --release --target-dir `"$target`" > `"$Out\build.txt`" 2>&1"
    if ($LASTEXITCODE -ne 0) {
        # Fallback: libjpeg-turbo and LibRaw's libjpeg are both linked statically; if they clash,
        # build without libjpeg-turbo so P4b still runs (P0 then has no turbo columns).
        if ($Only -eq 'p0') { throw "build with libjpeg-turbo failed - see $Out\build.txt" }
        Log "build with libjpeg-turbo FAILED - see build.txt; retrying without it"
        cmd /c "cargo build --release --no-default-features --target-dir `"$target`" > `"$Out\build-noturbo.txt`" 2>&1"
        if ($LASTEXITCODE -ne 0) { throw "build failed - see $Out\build-noturbo.txt" }
    }
} finally { Pop-Location }
& $exe version 2>&1 | ForEach-Object { Log $_ }

# Machine state for the record (no changes made).
$cpu = (Get-CimInstance Win32_Processor | Select-Object -First 1).Name
$ram = [math]::Round((Get-CimInstance Win32_ComputerSystem).TotalPhysicalMemory / 1GB, 1)
$plan = (& powercfg /getactivescheme) -join ' '
Log "machine: $cpu, $ram GB, $plan"

Log "[2/4] P0: embedded-preview JPEG decode over all RAW files in $Raw"
$all = Get-ChildItem -Path $Raw -File | Where-Object { $_.Extension -match '^\.(arw|rw2|dng|nef|cr2|orf|pef)$' } | Sort-Object Name
& $exe p0 (Join-Path $Out $P0Csv) @($all.FullName) 2>&1 | ForEach-Object { Log $_ }
if ($LASTEXITCODE -ne 0) { throw "P0 failed" }

if ($Only -eq 'p0') { Log "P0 only: done. Results: $Out\$P0Csv"; Log "=== phase 1 end ==="; exit 0 }
Log "[3/4] P4b: demosaic methods, threads $Threads (about 20-40 minutes)"
$names = 'TN100423.ARW', 'TN100424.ARW', 'TN100426.ARW', 'TN100433.ARW', 'TN100439.ARW',
         'P1240231.RW2', 'P1240233.RW2', 'P1240235.RW2', 'P1240236.RW2'
$files = $names | ForEach-Object { Join-Path $Raw $_ } | Where-Object { Test-Path $_ }
if ($files.Count -ne $names.Count) { Log "WARNING: only $($files.Count) of $($names.Count) P4b files found" }
$p4b = Join-Path $Out 'p4b'
& $exe p4b $p4b $Threads @($files) 2>&1 | ForEach-Object { Log $_ }
if ($LASTEXITCODE -ne 0) { throw "P4b failed" }

Log "[4/4] done. Results: $Out (p0-decode.csv, p4b\p4b-timing.csv, p4b\p4b-quality.csv, p4b\crops)"
Log "Blind review: open p4b\crops first; the key is in p4b\key-open-after-review - open it only afterwards."
Log "=== phase 1 end ==="
