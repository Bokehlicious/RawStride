# ADR 0008 phase 2, P3: presenter A (wgpu surface; webview on top or as a strip).
# 1. build; 2. timed swaps in Rust (--bench); 3. on-screen latency with latency.ps1 (key -> pixels).
# Writes only to D:\PhotoWorkflowData\analysis\phase2\p3 and target\prototypes. Installs nothing.
param(
    [string]$Raw = 'D:\PhotoWorkflowData\RAW',
    [string]$Out = 'D:\PhotoWorkflowData\analysis\phase2\p3',
    [string]$Latency = 'D:\PhotoWorkflowData\pm-study\tools\latency.ps1'
)
$ErrorActionPreference = 'Continue'
$here = Split-Path -Parent $MyInvocation.MyCommand.Path
$repo = Resolve-Path (Join-Path $here '..\..\..')
$target = Join-Path $repo 'target\prototypes'
$exe = Join-Path $target 'release\p3-wgpu.exe'
New-Item -ItemType Directory -Force -Path $Out | Out-Null
$log = Join-Path $Out 'run-p3.log'
function Log($m) { $line = "$(Get-Date -Format 'yyyy-MM-dd HH:mm:ss') $m"; Write-Host $line; Add-Content -Path $log -Value $line }

Log "=== P3 start ==="
$stamp = Get-Date -Format 'yyyyMMdd-HHmmss'
foreach ($f in 'p3-bench.json', 'p3-latency.csv') {
    $p = Join-Path $Out $f
    if (Test-Path $p) { Rename-Item $p "$($f -replace '\.(json|csv)$','')-$stamp.$($f.Split('.')[-1])"; Log "kept earlier $f" }
}
$lock = Join-Path $here 'Cargo.lock'
if (-not (Test-Path $lock)) { Copy-Item (Join-Path $repo 'Cargo.lock') $lock -ErrorAction Stop; Log "seeded Cargo.lock from the root lock" }

Log "[1/3] build (release, target\prototypes)"
Push-Location $here
try {
    cmd /c "cargo build --release --target-dir `"$target`" > `"$Out\build.txt`" 2>&1"
    if ($LASTEXITCODE -ne 0) { throw "build failed - see $Out\build.txt" }
} finally { Pop-Location }

$files = @((Join-Path $Raw 'TN005956.ARW'), (Join-Path $Raw 'TN006061.ARW'))
foreach ($f in $files) { if (-not (Test-Path $f)) { throw "missing $f" } }
$quoted = $files | ForEach-Object { "`"$_`"" }

Log "[2/3] timed swaps (about 3 minutes; the window opens and closes by itself)"
$json = Join-Path $Out 'p3-bench.json'
$p = Start-Process -FilePath $exe -ArgumentList (@('--bench', "`"$json`"", '--layout', 'overlay') + $quoted) -PassThru
if (-not $p.WaitForExit(900000)) { Stop-Process -Id $p.Id -Force; throw "bench run timed out" }
if (-not (Test-Path $json)) { throw "no bench result" }
Log "bench written: $json"

Log "[3/3] on-screen latency: arrow key -> changed pixels (latency.ps1), 8 runs"
$csv = Join-Path $Out 'p3-latency.csv'
foreach ($layout in 'overlay', 'strip') {
    foreach ($kind in 'v1440', 'full') {
        foreach ($swap in 'upload', 'resident') {
            $appArgs = @('--interactive', $swap, $kind, '--layout', $layout) + $quoted
            $app = Start-Process -FilePath $exe -ArgumentList $appArgs -PassThru
            Start-Sleep -Seconds 8
            & powershell -NoProfile -ExecutionPolicy Bypass -File $Latency -Title 'P3 wgpu' -Key RIGHT -Count 20 -Gap 700 -Quiet 300 `
                -Region '0.3,0.3,0.4,0.4' -Label "p3-$layout-$swap-$kind" -Out $csv 2>&1 | ForEach-Object { Log $_ }
            Stop-Process -Id $app.Id -Force -ErrorAction SilentlyContinue
            Start-Sleep -Seconds 2
        }
    }
}
Log "done. Results: $Out (p3-bench.json, p3-latency.csv)"
Log "=== P3 end ==="
