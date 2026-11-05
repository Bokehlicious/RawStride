# Pixel check before the RAW pipeline: (1) do the baseline and the OpenMP LibRaw produce identical
# pixels, and (2) which tile overlap makes a tile identical to the same pixels of a full decode?
# Needs vcpkg_installed_openmp\ (tools\m2-openmp.cmd). Results: D:\PhotoWorkflowData\analysis\raw-verify-<ts>\
$ErrorActionPreference = 'Stop'
$repo = (Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path
$raw = 'D:\PhotoWorkflowData\RAW'
$out = "D:\PhotoWorkflowData\analysis\raw-verify-$(Get-Date -Format yyyyMMdd-HHmmss)"
New-Item -ItemType Directory -Force $out | Out-Null
Set-Location $repo

Remove-Item Env:VCPKG_INSTALLED_DIR -ErrorAction SilentlyContinue
Remove-Item Env:CARGO_TARGET_DIR -ErrorAction SilentlyContinue
cargo build -p raw-decode-bench --profile profiling
if ($LASTEXITCODE -ne 0) { throw 'baseline build failed' }
$env:VCPKG_INSTALLED_DIR = "$repo\vcpkg_installed_openmp"
$env:CARGO_TARGET_DIR = "$repo\target-openmp"
cargo build -p raw-decode-bench --profile profiling
$rc = $LASTEXITCODE
Remove-Item Env:VCPKG_INSTALLED_DIR
Remove-Item Env:CARGO_TARGET_DIR
if ($rc -ne 0) { throw 'OpenMP build failed' }

$files = @(
  (Get-ChildItem $raw -Filter 'TN1004*.ARW' | Sort-Object Name | Select-Object -First 1).FullName,
  (Get-ChildItem $raw -Filter 'TN0059*.ARW' | Sort-Object Name | Select-Object -First 1).FullName,
  (Get-ChildItem $raw -Filter '*.RW2' | Sort-Object Name | Select-Object -First 1).FullName,
  (Get-ChildItem $raw -Filter '*.dng' | Sort-Object Name | Select-Object -First 1).FullName
) | Where-Object { $_ }

foreach ($b in @(@('baseline', "$repo\target\profiling\raw-decode-bench.exe"),
                 @('openmp', "$repo\target-openmp\profiling\raw-decode-bench.exe"))) {
  & $b[1] verify @files --out "$out\verify-$($b[0]).csv" | Tee-Object "$out\verify-$($b[0]).txt"
  if ($LASTEXITCODE -ne 0) { throw "verify failed ($($b[0]))" }
}

# Compare every hash (whole images and tiles) between the two builds.
$a = Import-Csv "$out\verify-baseline.csv"
$o = Import-Csv "$out\verify-openmp.csv"
$lines = @()
$same = 0; $diff = 0
for ($i = 0; $i -lt $a.Count; $i++) {
  $key = "$($a[$i].kind) $($a[$i].file) $($a[$i].what) auto=$($a[$i].auto_bright) margin=$($a[$i].margin)"
  if ($a[$i].hash -eq $o[$i].hash -and $a[$i].w -eq $o[$i].w -and $a[$i].h -eq $o[$i].h) { $same++; $lines += "SAME $key" }
  else { $diff++; $lines += "DIFF $key  baseline $($a[$i].hash) openmp $($o[$i].hash)" }
}
$lines += "identical: $same, different: $diff"
$lines | Tee-Object "$out\compare.txt"
Write-Host "`nResults in $out" -ForegroundColor Green
