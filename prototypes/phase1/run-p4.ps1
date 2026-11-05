# ADR 0008 phase 1, P4: scheduling without UI (visible region first, zoom-target pre-render,
# neighbour prefetch), with PPG (ADR 0008 section 11). Started by run-p4.cmd.
# Writes only to D:\PhotoWorkflowData\analysis\phase1\p4 and target\prototypes. Installs nothing.
param(
    [string]$Raw = 'D:\PhotoWorkflowData\RAW',
    [string]$Out = 'D:\PhotoWorkflowData\analysis\phase1\p4'
)
# 'Continue': in Windows PowerShell 5.1, stderr from native tools (cargo progress) must not abort.
$ErrorActionPreference = 'Continue'
$here = Split-Path -Parent $MyInvocation.MyCommand.Path
$repo = Resolve-Path (Join-Path $here '..\..')
$target = Join-Path $repo 'target\prototypes'
$exe = Join-Path $target 'release\proto-phase1.exe'
New-Item -ItemType Directory -Force -Path $Out | Out-Null
$log = Join-Path $Out 'run-p4.log'
function Log($m) { $line = "$(Get-Date -Format 'yyyy-MM-dd HH:mm:ss') $m"; Write-Host $line; Add-Content -Path $log -Value $line }

Log "=== P4 start ==="
# Earlier results are kept under a time stamp, never appended to.
$stamp = Get-Date -Format 'yyyyMMdd-HHmmss'
foreach ($f in 'p4-events.csv', 'p4-runs.csv', 'p4-summary.csv') {
    $p = Join-Path $Out $f
    if (Test-Path $p) { Rename-Item $p "$($f -replace '\.csv$','')-$stamp.csv"; Log "kept earlier $f as -$stamp" }
}

Log "[1/3] build (release, target\prototypes)"
Push-Location $here
try {
    cmd /c "cargo build --release --target-dir `"$target`" > `"$Out\build.txt`" 2>&1"
    if ($LASTEXITCODE -ne 0) { throw "build failed - see $Out\build.txt" }
} finally { Pop-Location }
& $exe version 2>&1 | ForEach-Object { Log $_ }
$cpu = (Get-CimInstance Win32_Processor | Select-Object -First 1).Name
$plan = (& powercfg /getactivescheme) -join ' '
Log "machine: $cpu, $plan"

Log "[2/3] P4 runs, about 5 minutes each, 12 runs (about an hour)"
$groups = @(
    @{ Name = 'A7R'; Strategies = 'none,cur,full,full-tiles,full-dwell,full-ahd';
       Files = 'TN100423.ARW', 'TN100424.ARW', 'TN100426.ARW', 'TN100433.ARW', 'TN100439.ARW' },
    @{ Name = 'A1'; Strategies = 'none,cur,full';
       Files = 'TN005956.ARW', 'TN005960.ARW', 'TN005978.ARW', 'TN005979.ARW', 'TN005983.ARW', 'TN005993.ARW',
               'TN006061.ARW', 'TN006062.ARW', 'TN006063.ARW', 'TN006064.ARW', 'TN006065.ARW', 'TN006066.ARW', 'TN006067.ARW' },
    @{ Name = 'RW2'; Strategies = 'none,cur,full';
       Files = 'P1240231.RW2', 'P1240233.RW2', 'P1240235.RW2', 'P1240236.RW2' }
)
foreach ($g in $groups) {
    $files = $g.Files | ForEach-Object { Join-Path $Raw $_ } | Where-Object { Test-Path $_ }
    if ($files.Count -ne $g.Files.Count) { Log "WARNING: $($g.Name): only $($files.Count) of $($g.Files.Count) files found" }
    & $exe p4 $Out $g.Name $g.Strategies @($files) 2>&1 | ForEach-Object { Log $_ }
    if ($LASTEXITCODE -ne 0) { throw "P4 $($g.Name) failed" }
}

Log "[3/3] summary"
& $exe p4sum $Out 2>&1 | ForEach-Object { Log $_ }
if ($LASTEXITCODE -ne 0) { throw "P4 summary failed" }
Log "done. Results: $Out (p4-summary.csv, p4-runs.csv, p4-events.csv)"
Log "=== P4 end ==="
