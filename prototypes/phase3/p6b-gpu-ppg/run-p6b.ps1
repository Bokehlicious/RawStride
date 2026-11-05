# ADR 0008 P6b: PPG demosaicing on the GPU in presenter B2-res (WebGL2).
# 1. build; 2. in-page timings, quality against LibRaw PPG/AHD, stepping, GPU memory and context loss
# (--bench); 3. on-screen latency with latency.ps1 (Z, pan D at 100 %, RIGHT at a normal and a fast pace).
# Writes only to D:\PhotoWorkflowData\analysis\phase3\p6b and target\prototypes. Installs nothing.
param(
    [string]$Raw = 'D:\PhotoWorkflowData\RAW',
    [string]$Out = 'D:\PhotoWorkflowData\analysis\phase3\p6b',
    [string]$Latency = 'D:\PhotoWorkflowData\pm-study\tools\latency.ps1'
)
$ErrorActionPreference = 'Continue'
$here = Split-Path -Parent $MyInvocation.MyCommand.Path
$repo = Resolve-Path (Join-Path $here '..\..\..')
$target = Join-Path $repo 'target\prototypes'
$exe = Join-Path $target 'release\p6b-gpu-ppg.exe'
$title = 'P6b GPU PPG'
New-Item -ItemType Directory -Force -Path $Out | Out-Null
$log = Join-Path $Out 'run-p6b.log'
function Log($m) { $line = "$(Get-Date -Format 'yyyy-MM-dd HH:mm:ss') $m"; Write-Host $line; Add-Content -Path $log -Value $line }

Log "=== P6b start ==="
$stamp = Get-Date -Format 'yyyyMMdd-HHmmss'
foreach ($f in 'p6b-bench.json', 'p6b-latency.csv') {
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

# A7R x5 first: the stepping scenarios use the first five files. Then RW2 x4, A1 x2 and
# Ricoh GR III DNG (TN002512 horizontal, TN002513 vertical) for the orientation check.
$names = 'TN100423.ARW', 'TN100424.ARW', 'TN100426.ARW', 'TN100433.ARW', 'TN100439.ARW',
         'P1240231.RW2', 'P1240233.RW2', 'P1240235.RW2', 'P1240236.RW2', 'TN005956.ARW', 'TN006061.ARW',
         'TN002512.DNG', 'TN002513.DNG'
$files = @()
foreach ($n in $names) {
    $hit = Get-ChildItem -Path $Raw -Recurse -File -Filter $n -ErrorAction SilentlyContinue | Select-Object -First 1
    if ($hit) { $files += $hit.FullName } else { Log "missing: $n" }
}
$quoted = $files | ForEach-Object { "`"$_`"" }
Log "files: $($files.Count)"

Log "[2/3] in-page measurements (about 8 minutes; the window opens and closes by itself)"
$json = Join-Path $Out 'p6b-bench.json'
$p = Start-Process -FilePath $exe -ArgumentList (@('--bench', "`"$json`"") + $quoted) -PassThru
if (-not $p.WaitForExit(1800000)) { Stop-Process -Id $p.Id -Force; throw "bench run timed out" }
if (-not (Test-Path $json)) { throw "no bench result" }
Log "bench written: $json"

Log "[3/3] on-screen latency (latency.ps1) on the A7R files"
$csv = Join-Path $Out 'p6b-latency.csv'
$a7r = $quoted[0..4]
function Lat($key, $count, $gap, $quiet, $label) {
    & powershell -NoProfile -ExecutionPolicy Bypass -File $Latency -Title $title -Key $key -Count $count -Gap $gap -Quiet $quiet `
        -Region '0.3,0.3,0.4,0.4' -Label $label -Out $csv 2>&1 | ForEach-Object { Log $_ }
}
# fit view: Z toggles fit / 100 %, then RIGHT at a normal and a fast pace (prefetch +-1, +2)
$app = Start-Process -FilePath $exe -ArgumentList (@('--interactive') + $a7r) -PassThru
Start-Sleep -Seconds 10
Lat 'Z' 20 700 300 'p6b-a7r-z-ppg'
Lat 'RIGHT' 12 1500 600 'p6b-a7r-next-1500ms'
Lat 'RIGHT' 12 400 100 'p6b-a7r-next-400ms'
Stop-Process -Id $app.Id -Force -ErrorAction SilentlyContinue
Start-Sleep -Seconds 3
# 100 % view: pan with D, then RIGHT while zoomed
$app = Start-Process -FilePath $exe -ArgumentList (@('--interactive', '--zoomed') + $a7r) -PassThru
Start-Sleep -Seconds 10
Lat 'D' 20 500 200 'p6b-a7r-pan-ppg'
Lat 'RIGHT' 12 1500 600 'p6b-a7r-next-zoomed-1500ms'
Stop-Process -Id $app.Id -Force -ErrorAction SilentlyContinue
Log "done. Results: $Out (p6b-bench.json, p6b-latency.csv, crops\)"
Log "=== P6b end ==="
