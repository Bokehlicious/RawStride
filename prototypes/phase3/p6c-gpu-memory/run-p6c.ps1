# ADR 0008 P6c: GPU PPG memory budget and texture-cache validation (presenter B2-res, WebGL2).
# Part A: M0 overlap sweep (U1-U3), upload/pool/half variants (NAT + MIX), stress M1-M5, mixed sizes (M6).
# Part B: the P5 scenarios with GPU PPG at texture budgets 2048-768 MB (A7R), K2 CPU tier, A1, RW2.
# Part C: on-screen latency with latency.ps1 (Z, RIGHT at 1500/400 ms, pan D at 100 %).
# Every configuration runs in a fresh process, so memory starts from the same baseline.
# Writes only to D:\PhotoWorkflowData\analysis\phase3\p6c and target\prototypes. Installs nothing.
param(
    [ValidateSet('A', 'B', 'C', 'all')][string]$Part = 'all',
    [string]$Raw = 'D:\PhotoWorkflowData\RAW',
    [string]$Out = 'D:\PhotoWorkflowData\analysis\phase3\p6c',
    [string]$Latency = 'D:\PhotoWorkflowData\pm-study\tools\latency.ps1'
)
$ErrorActionPreference = 'Continue'
$here = Split-Path -Parent $MyInvocation.MyCommand.Path
$repo = Resolve-Path (Join-Path $here '..\..\..')
$target = Join-Path $repo 'target\prototypes'
$exe = Join-Path $target 'release\p6c-gpu-memory.exe'
$title = 'P6c GPU memory'
New-Item -ItemType Directory -Force -Path $Out | Out-Null
$log = Join-Path $Out 'run-p6c.log'
function Log($m) { $line = "$(Get-Date -Format 'yyyy-MM-dd HH:mm:ss') $m"; Write-Host $line; Add-Content -Path $log -Value $line }

Log "=== P6c start (part $Part) ==="
$lock = Join-Path $here 'Cargo.lock'
if (-not (Test-Path $lock)) { Copy-Item (Join-Path $here '..\p6b-gpu-ppg\Cargo.lock') $lock -ErrorAction Stop; Log "seeded Cargo.lock from P6b" }
Log "build (release, target\prototypes)"
Push-Location $here
try {
    cmd /c "cargo build --release --target-dir `"$target`" > `"$Out\build.txt`" 2>&1"
    if ($LASTEXITCODE -ne 0) { throw "build failed - see $Out\build.txt" }
} finally { Pop-Location }

function Files($names) {
    $f = @()
    foreach ($n in $names) { $p = Join-Path $Raw $n; if (Test-Path $p) { $f += "`"$p`"" } else { Log "missing: $n" } }
    return $f
}
# the P5 groups (phase1 run-p5.ps1)
$a7r = Files @('TN100423.ARW', 'TN100424.ARW', 'TN100426.ARW', 'TN100433.ARW', 'TN100439.ARW')
$a1 = Files @('TN005956.ARW', 'TN005960.ARW', 'TN005978.ARW', 'TN005979.ARW', 'TN005983.ARW', 'TN005993.ARW',
              'TN006061.ARW', 'TN006062.ARW', 'TN006063.ARW', 'TN006064.ARW', 'TN006065.ARW', 'TN006066.ARW', 'TN006067.ARW')
$rw2 = Files @('P1240231.RW2', 'P1240233.RW2', 'P1240235.RW2', 'P1240236.RW2')
# M6: mixed sizes, interleaved
$mixed = Files @('TN100423.ARW', 'TN005956.ARW', 'P1240231.RW2', 'TN100424.ARW', 'P1240233.RW2', 'TN006061.ARW',
                 'TN100426.ARW', 'P1240235.RW2', 'TN100433.ARW', 'P1240236.RW2', 'TN100439.ARW')

function RunCfg($tag, $suite, $files, [string[]]$opts, [int]$minutes) {
    $json = Join-Path $Out "$tag.json"
    if (Test-Path $json) { Rename-Item $json "$tag-$(Get-Date -Format 'yyyyMMdd-HHmmss').json"; Log "kept earlier $tag.json" }
    Log "run $tag ($suite $($opts -join ' '))"
    $args2 = @('--suite', $suite, '--out', "`"$Out`"", '--tag', $tag) + $opts + $files
    $p = Start-Process -FilePath $exe -ArgumentList $args2 -PassThru
    if (-not $p.WaitForExit($minutes * 60000)) { Stop-Process -Id $p.Id -Force; Log "  TIMEOUT after $minutes min"; return }
    if (-not (Test-Path $json)) { Log "  no result"; return }
    $r = Get-Content $json -Raw | ConvertFrom-Json
    if ($r.error) { Log "  ERROR: $($r.error)" } else { Log "  ok" }
    Start-Sleep -Seconds 5
}

if ($Part -in 'A', 'all') {
    Log "[A] memory: M0 overlap sweep, variants, stress, mixed sizes"
    foreach ($u in 'U1', 'U2', 'U3') { RunCfg "m0-$u" 'm0' $a7r @('--upload', $u) 6 }
    RunCfg 'natmix-a7r-U2-1400' 'natmix' $a7r @('--budget', '1400', '--upload', 'U2') 12
    RunCfg 'natmix-a7r-U3-1400' 'natmix' $a7r @('--budget', '1400', '--upload', 'U3') 12
    RunCfg 'natmix-a7r-U4-1400' 'natmix' $a7r @('--budget', '1400', '--upload', 'U4') 12
    RunCfg 'natmix-a7r-H0-1400' 'natmix' $a7r @('--budget', '1400', '--half', '0') 12
    RunCfg 'stress-a7r-U1-1400' 'stress' $a7r @('--budget', '1400') 20
    RunCfg 'natmix-mixed-U1-1400' 'natmix' $mixed @('--budget', '1400') 12
    RunCfg 'natmix-mixed-U4-1400' 'natmix' $mixed @('--budget', '1400', '--upload', 'U4') 12
}
if ($Part -in 'B', 'all') {
    Log "[B] P5 scenarios with GPU PPG"
    foreach ($b in 2048, 1400, 1300, 1024, 768) { RunCfg "p5-a7r-U1-$b" 'p5' $a7r @('--budget', "$b") 15 }
    foreach ($b in 1400, 768) { RunCfg "p5-a7r-K2-$b" 'p5' $a7r @('--budget', "$b", '--tier', '1536') 15 }
    foreach ($b in 1400, 768) { RunCfg "p5-a1-U1-$b" 'p5' $a1 @('--budget', "$b") 15 }
    RunCfg 'p5-rw2-U1-1400' 'p5' $rw2 @('--budget', '1400') 15
}
if ($Part -in 'C', 'all') {
    Log "[C] on-screen latency (latency.ps1), A7R, texture budget 1400 MB"
    $csv = Join-Path $Out 'p6c-latency.csv'
    if (Test-Path $csv) { Rename-Item $csv "p6c-latency-$(Get-Date -Format 'yyyyMMdd-HHmmss').csv" }
    function Lat($key, $count, $gap, $quiet, $label) {
        & powershell -NoProfile -ExecutionPolicy Bypass -File $Latency -Title $title -Key $key -Count $count -Gap $gap -Quiet $quiet `
            -Region '0.3,0.3,0.4,0.4' -Label $label -Out $csv 2>&1 | ForEach-Object { Log $_ }
    }
    $app = Start-Process -FilePath $exe -ArgumentList (@('--suite', 'interactive', '--out', "`"$Out`"", '--tag', 'interactive', '--budget', '1400') + $a7r) -PassThru
    Start-Sleep -Seconds 10
    Lat 'Z' 20 700 300 'p6c-a7r-z-1400'
    Lat 'RIGHT' 12 1500 600 'p6c-a7r-next-1500ms-1400'
    Lat 'RIGHT' 12 400 100 'p6c-a7r-next-400ms-1400'
    Stop-Process -Id $app.Id -Force -ErrorAction SilentlyContinue
    Start-Sleep -Seconds 3
    $app = Start-Process -FilePath $exe -ArgumentList (@('--suite', 'interactive', '--out', "`"$Out`"", '--tag', 'interactive-zoomed', '--budget', '1400', '--zoomed') + $a7r) -PassThru
    Start-Sleep -Seconds 10
    Lat 'D' 20 500 200 'p6c-a7r-pan-1400'
    Stop-Process -Id $app.Id -Force -ErrorAction SilentlyContinue
}
Log "=== P6c end ==="
