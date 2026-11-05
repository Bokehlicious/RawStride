# ADR 0008 P6: GPU demosaicing in presenter B2 (WebGL2). 1. build; 2. in-page timings and quality
# (--bench); 3. on-screen latency with latency.ps1 (Z with the raw data on the GPU; RIGHT = new file).
# Writes only to D:\PhotoWorkflowData\analysis\phase3\p6 and target\prototypes. Installs nothing.
param(
    [string]$Raw = 'D:\PhotoWorkflowData\RAW',
    [string]$Out = 'D:\PhotoWorkflowData\analysis\phase3\p6',
    [string]$Latency = 'D:\PhotoWorkflowData\pm-study\tools\latency.ps1'
)
$ErrorActionPreference = 'Continue'
$here = Split-Path -Parent $MyInvocation.MyCommand.Path
$repo = Resolve-Path (Join-Path $here '..\..\..')
$target = Join-Path $repo 'target\prototypes'
$exe = Join-Path $target 'release\p6-gpu-demosaic.exe'
New-Item -ItemType Directory -Force -Path $Out | Out-Null
$log = Join-Path $Out 'run-p6.log'
function Log($m) { $line = "$(Get-Date -Format 'yyyy-MM-dd HH:mm:ss') $m"; Write-Host $line; Add-Content -Path $log -Value $line }

Log "=== P6 start ==="
$stamp = Get-Date -Format 'yyyyMMdd-HHmmss'
foreach ($f in 'p6-bench.json', 'p6-latency.csv') {
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

$names = 'TN100423.ARW', 'TN100424.ARW', 'TN100426.ARW', 'TN100433.ARW', 'TN100439.ARW',
         'P1240231.RW2', 'P1240233.RW2', 'P1240235.RW2', 'P1240236.RW2', 'TN005956.ARW', 'TN006061.ARW'
$files = $names | ForEach-Object { Join-Path $Raw $_ } | Where-Object { Test-Path $_ }
$quoted = $files | ForEach-Object { "`"$_`"" }

Log "[2/3] in-page timings and quality (about 5 minutes; the window opens and closes by itself)"
$json = Join-Path $Out 'p6-bench.json'
$p = Start-Process -FilePath $exe -ArgumentList (@('--bench', "`"$json`"") + $quoted) -PassThru
if (-not $p.WaitForExit(1200000)) { Stop-Process -Id $p.Id -Force; throw "bench run timed out" }
if (-not (Test-Path $json)) { throw "no bench result" }
Log "bench written: $json"

Log "[3/3] on-screen latency (latency.ps1): Z on A7R files, then RIGHT"
$csv = Join-Path $Out 'p6-latency.csv'
$a7r = $quoted[0..4]
$app = Start-Process -FilePath $exe -ArgumentList (@('--interactive') + $a7r) -PassThru
Start-Sleep -Seconds 10
& powershell -NoProfile -ExecutionPolicy Bypass -File $Latency -Title 'P6 GPU demosaic' -Key Z -Count 20 -Gap 700 -Quiet 300 `
    -Region '0.3,0.3,0.4,0.4' -Label 'p6-a7r-z-gpu' -Out $csv 2>&1 | ForEach-Object { Log $_ }
& powershell -NoProfile -ExecutionPolicy Bypass -File $Latency -Title 'P6 GPU demosaic' -Key RIGHT -Count 10 -Gap 1500 -Quiet 600 `
    -Region '0.3,0.3,0.4,0.4' -Label 'p6-a7r-next-cold' -Out $csv 2>&1 | ForEach-Object { Log $_ }
Stop-Process -Id $app.Id -Force -ErrorAction SilentlyContinue
Log "done. Results: $Out (p6-bench.json, p6-latency.csv, crops\)"
Log "=== P6 end ==="
