# ADR 0008 / GUI_REQUIREMENTS P7: image presentation in eframe + egui, glow and wgpu.
# A: M0 texture memory per renderer. B: the P5 scenarios at texture budgets 2048/1024/512 MB (A7R),
# A1 and RW2. C: stress M1-M5. D: on-screen latency with latency.ps1 (Z, RIGHT at 1500/400 ms, pan D).
# Every configuration runs in a fresh process. Writes only to D:\PhotoWorkflowData\analysis\gui\p7 and
# target\prototypes. Installs nothing.
param(
    [ValidateSet('A', 'B', 'C', 'D', 'all')][string]$Part = 'all',
    [string]$Raw = 'D:\PhotoWorkflowData\RAW',
    [string]$Out = 'D:\PhotoWorkflowData\analysis\gui\p7',
    [string]$Latency = 'D:\PhotoWorkflowData\pm-study\tools\latency.ps1'
)
$ErrorActionPreference = 'Continue'
$here = Split-Path -Parent $MyInvocation.MyCommand.Path
$repo = Resolve-Path (Join-Path $here '..\..\..')
$target = Join-Path $repo 'target\prototypes'
$exe = Join-Path $target 'release\p7-egui-presenter.exe'
$title = 'P7 egui presenter'
New-Item -ItemType Directory -Force -Path $Out | Out-Null
$log = Join-Path $Out 'run-p7.log'
function Log($m) { $line = "$(Get-Date -Format 'yyyy-MM-dd HH:mm:ss') $m"; Write-Host $line; Add-Content -Path $log -Value $line }

Log "=== P7 start (part $Part) ==="
Log "build (release, target\prototypes)"
Push-Location $here
try {
    cmd /c "cargo build --release --target-dir `"$target`" > `"$Out\build-run.txt`" 2>&1"
    if ($LASTEXITCODE -ne 0) { throw "build failed - see $Out\build-run.txt" }
} finally { Pop-Location }

function Files($names) {
    $f = @()
    foreach ($n in $names) { $p = Join-Path $Raw $n; if (Test-Path $p) { $f += "`"$p`"" } else { Log "missing: $n" } }
    return $f
}
# the P5 groups (phase 1 run-p5.ps1)
$a7r = Files @('TN100423.ARW', 'TN100424.ARW', 'TN100426.ARW', 'TN100433.ARW', 'TN100439.ARW')
$a1 = Files @('TN005956.ARW', 'TN005960.ARW', 'TN005978.ARW', 'TN005979.ARW', 'TN005983.ARW', 'TN005993.ARW',
              'TN006061.ARW', 'TN006062.ARW', 'TN006063.ARW', 'TN006064.ARW', 'TN006065.ARW', 'TN006066.ARW', 'TN006067.ARW')
$rw2 = Files @('P1240231.RW2', 'P1240233.RW2', 'P1240235.RW2', 'P1240236.RW2')

function RunCfg($tag, $suite, $renderer, $files, [string[]]$opts, [int]$minutes) {
    $txt = Join-Path $Out "$tag.txt"
    if (Test-Path $txt) { Rename-Item $txt "$tag-$(Get-Date -Format 'yyyyMMdd-HHmmss').txt"; Log "kept earlier $tag.txt" }
    Log "run $tag ($suite $renderer $($opts -join ' '))"
    $args2 = @('--suite', $suite, '--renderer', $renderer, '--out', "`"$Out`"", '--tag', $tag) + $opts + $files
    $p = Start-Process -FilePath $exe -ArgumentList $args2 -PassThru -RedirectStandardError (Join-Path $Out "$tag-stderr.txt")
    if (-not $p.WaitForExit($minutes * 60000)) { Stop-Process -Id $p.Id -Force; Log "  TIMEOUT after $minutes min"; return }
    if (-not (Test-Path $txt)) { Log "  no result (exit $($p.ExitCode))"; return }
    Log "  ok (exit $($p.ExitCode))"
    Start-Sleep -Seconds 5
}

if ($Part -in 'A', 'all') {
    Log "[A] M0 texture memory"
    foreach ($r in 'wgpu', 'glow') { RunCfg "m0-$r" 'm0' $r $a7r @() 6 }
}
if ($Part -in 'B', 'all') {
    Log "[B] P5 scenarios"
    foreach ($r in 'wgpu', 'glow') {
        foreach ($b in 2048, 1024, 512) { RunCfg "p5-a7r-$r-$b" 'p5' $r $a7r @('--tex-mb', "$b") 15 }
        RunCfg "p5-a1-$r-1024" 'p5' $r $a1 @('--tex-mb', '1024') 15
    }
    RunCfg 'p5-rw2-wgpu-1024' 'p5' 'wgpu' $rw2 @('--tex-mb', '1024') 15
}
if ($Part -in 'C', 'all') {
    Log "[C] stress M1-M5"
    foreach ($r in 'wgpu', 'glow') { RunCfg "stress-a7r-$r-1024" 'stress' $r $a7r @('--tex-mb', '1024') 20 }
}
if ($Part -in 'D', 'all') {
    Log "[D] on-screen latency (latency.ps1)"
    $csv = Join-Path $Out 'p7-latency.csv'
    if (Test-Path $csv) { Rename-Item $csv "p7-latency-$(Get-Date -Format 'yyyyMMdd-HHmmss').csv" }
    function Lat($key, $count, $gap, $quiet, $label) {
        & powershell -NoProfile -ExecutionPolicy Bypass -File $Latency -Title $title -Key $key -Count $count -Gap $gap -Quiet $quiet `
            -Region '0.3,0.3,0.4,0.4' -Label $label -Out $csv 2>&1 | ForEach-Object { Log $_ }
    }
    function App($renderer, $files, [string[]]$opts) {
        $a = @('--suite', 'interactive', '--renderer', $renderer, '--out', "`"$Out`"", '--tag', "interactive-$renderer", '--tex-mb', '1024') + $opts + $files
        $p = Start-Process -FilePath $exe -ArgumentList $a -PassThru
        Start-Sleep -Seconds 10
        return $p
    }
    foreach ($r in 'wgpu', 'glow') {
        $p = App $r $a7r @()
        Lat 'Z' 20 1500 300 "p7-a7r-$r-z"
        Lat 'RIGHT' 12 1500 600 "p7-a7r-$r-next-1500ms"
        Lat 'RIGHT' 12 400 100 "p7-a7r-$r-next-400ms"
        Stop-Process -Id $p.Id -Force -ErrorAction SilentlyContinue
        Start-Sleep -Seconds 3
        $p = App $r $a7r @('--zoomed')
        Lat 'D' 20 500 200 "p7-a7r-$r-pan"
        Stop-Process -Id $p.Id -Force -ErrorAction SilentlyContinue
        Start-Sleep -Seconds 3
        $p = App $r $a1 @()
        Lat 'RIGHT' 12 1500 600 "p7-a1-$r-next-1500ms"
        Stop-Process -Id $p.Id -Force -ErrorAction SilentlyContinue
        Start-Sleep -Seconds 3
    }
}
Log "=== P7 end ==="
