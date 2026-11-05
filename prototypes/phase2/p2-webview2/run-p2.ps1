# ADR 0008 phase 2, P2: presenter B2 (WebView2 shared buffer -> WebGL2/WebGPU) against B1.
# 1. build; 2. in-page measurement (--bench); 3. on-screen latency with latency.ps1 (key -> pixels).
# Writes only to D:\PhotoWorkflowData\analysis\phase2\p2 and target\prototypes. Installs nothing.
param(
    [string]$Raw = 'D:\PhotoWorkflowData\RAW',
    [string]$Out = 'D:\PhotoWorkflowData\analysis\phase2\p2',
    [string]$Latency = 'D:\PhotoWorkflowData\pm-study\tools\latency.ps1'
)
$ErrorActionPreference = 'Continue'
$here = Split-Path -Parent $MyInvocation.MyCommand.Path
$repo = Resolve-Path (Join-Path $here '..\..\..')
$target = Join-Path $repo 'target\prototypes'
$exe = Join-Path $target 'release\p2-webview2.exe'
New-Item -ItemType Directory -Force -Path $Out | Out-Null
$log = Join-Path $Out 'run-p2.log'
function Log($m) { $line = "$(Get-Date -Format 'yyyy-MM-dd HH:mm:ss') $m"; Write-Host $line; Add-Content -Path $log -Value $line }

Log "=== P2 start ==="
$stamp = Get-Date -Format 'yyyyMMdd-HHmmss'
foreach ($f in 'p2-bench.json', 'p2-latency.csv') {
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
$gpuName = (Get-CimInstance Win32_VideoController | Select-Object -ExpandProperty Name) -join '; '
Log "GPU: $gpuName"

Log "[2/3] in-page measurement (about 5 minutes; the window opens and closes by itself)"
$json = Join-Path $Out 'p2-bench.json'
$p = Start-Process -FilePath $exe -ArgumentList (@('--bench', "`"$json`"") + ($files | ForEach-Object { "`"$_`"" })) -PassThru
if (-not $p.WaitForExit(900000)) { Stop-Process -Id $p.Id -Force; throw "bench run timed out" }
if (-not (Test-Path $json)) { throw "no bench result" }
Log "bench written: $json"

Log "[3/3] on-screen latency: arrow key -> changed pixels (latency.ps1), 8 runs"
$csv = Join-Path $Out 'p2-latency.csv'
foreach ($kind in 'v1440', 'full') {
    foreach ($path in 'b1', 'b2-new', 'b2-pool', 'b2-res') {
        $appArgs = @('--interactive', $path, $kind) + ($files | ForEach-Object { "`"$_`"" })
        $app = Start-Process -FilePath $exe -ArgumentList $appArgs -PassThru
        Start-Sleep -Seconds 10
        & powershell -NoProfile -ExecutionPolicy Bypass -File $Latency -Title 'P2 WebView2' -Key RIGHT -Count 20 -Gap 700 -Quiet 300 `
            -Region '0.3,0.3,0.4,0.4' -Label "p2-$path-$kind" -Out $csv 2>&1 | ForEach-Object { Log $_ }
        Stop-Process -Id $app.Id -Force -ErrorAction SilentlyContinue
        Start-Sleep -Seconds 2
    }
}
Log "done. Results: $Out (p2-bench.json, p2-latency.csv)"
Log "=== P2 end ==="
