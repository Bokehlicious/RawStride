# ADR 0008 phase 1, P5: memory budget. Strategy p5 (owner, 2026-11-05) = full + 150 ms dwell +
# other display mode after the neighbours + exact visible region, with ONE budget for unpacked
# sessions and views. 4 GB is the chosen budget; 2 and 3 GB are run for comparison (ADR 8.2).
# Started by run-p5.cmd. Writes only to D:\PhotoWorkflowData\analysis\phase1\p5 and
# target\prototypes. Installs nothing.
param(
    [string]$Raw = 'D:\PhotoWorkflowData\RAW',
    [string]$Out = 'D:\PhotoWorkflowData\analysis\phase1\p5'
)
# 'Continue': in Windows PowerShell 5.1, stderr from native tools (cargo progress) must not abort.
$ErrorActionPreference = 'Continue'
$here = Split-Path -Parent $MyInvocation.MyCommand.Path
$repo = Resolve-Path (Join-Path $here '..\..')
$target = Join-Path $repo 'target\prototypes'
$exe = Join-Path $target 'release\proto-phase1.exe'
New-Item -ItemType Directory -Force -Path $Out | Out-Null
$log = Join-Path $Out 'run-p5.log'
function Log($m) { $line = "$(Get-Date -Format 'yyyy-MM-dd HH:mm:ss') $m"; Write-Host $line; Add-Content -Path $log -Value $line }

Log "=== P5 start ==="
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

Log "[2/3] P5 runs, about 8 minutes each, 7 runs (about an hour)"
$groups = @(
    @{ Name = 'A7R'; Strategies = 'p5@4096,p5@3072,p5@2048';
       Files = 'TN100423.ARW', 'TN100424.ARW', 'TN100426.ARW', 'TN100433.ARW', 'TN100439.ARW' },
    @{ Name = 'A1'; Strategies = 'p5@4096,p5@3072,p5@2048';
       Files = 'TN005956.ARW', 'TN005960.ARW', 'TN005978.ARW', 'TN005979.ARW', 'TN005983.ARW', 'TN005993.ARW',
               'TN006061.ARW', 'TN006062.ARW', 'TN006063.ARW', 'TN006064.ARW', 'TN006065.ARW', 'TN006066.ARW', 'TN006067.ARW' },
    @{ Name = 'RW2'; Strategies = 'p5@4096';
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
Log "=== P5 end ==="
