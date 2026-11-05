# Builds a standalone distribution folder of PhotoWorkflow (ADR 0006, step 1/5):
#   dist\PhotoWorkflow\photoworkflow.exe + the MSVC runtime DLLs it imports (incl. vcomp140.dll,
#   the OpenMP runtime LibRaw needs), copied app-locally from the VS Build Tools redist folder.
# Lists every imported DLL and where it comes from in dist\PhotoWorkflow\dist-manifest.txt.
param([switch]$NoBuild)
$ErrorActionPreference = 'Stop'
$repo = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
Set-Location $repo
if (-not $NoBuild) {
  cargo build --release -p photoworkflow
  if ($LASTEXITCODE -ne 0) { throw 'release build failed' }
}
$exe = "$repo\target\release\photoworkflow.exe"
$dist = "$repo\dist\PhotoWorkflow"
if (Test-Path $dist) { Remove-Item $dist -Recurse -Force }
New-Item -ItemType Directory -Force $dist | Out-Null
Copy-Item $exe $dist

# Visual Studio (Build Tools) installation, dumpbin and the redistributable folders.
$vswhere = "${env:ProgramFiles(x86)}\Microsoft Visual Studio\Installer\vswhere.exe"
if (-not (Test-Path $vswhere)) { throw 'vswhere.exe not found (Visual Studio Installer)' }
$vs = & $vswhere -latest -products * -property installationPath
$msvc = Get-ChildItem "$vs\VC\Tools\MSVC" -Directory | Sort-Object Name -Descending | Select-Object -First 1
$dumpbin = "$($msvc.FullName)\bin\Hostx64\x64\dumpbin.exe"
if (-not (Test-Path $dumpbin)) { throw "dumpbin.exe not found: $dumpbin" }
$redist = Get-ChildItem "$vs\VC\Redist\MSVC" -Directory | Where-Object { Test-Path "$($_.FullName)\x64" } |
  Sort-Object Name -Descending | Select-Object -First 1
if (-not $redist) { throw 'VC\Redist\MSVC\<version>\x64 not found' }
$redistDirs = Get-ChildItem "$($redist.FullName)\x64" -Directory | Where-Object { $_.Name -match 'CRT|OpenMP' }

function Get-Imports([string]$file) {
  & $dumpbin /nologo /dependents $file | ForEach-Object { $_.Trim() } |
    Where-Object { $_ -match '^[\w\-\.]+\.dll$' }
}
# DLLs that are part of Windows itself (never shipped).
function Is-System([string]$dll) {
  $d = $dll.ToLower()
  if ($d.StartsWith('api-ms-win-') -or $d.StartsWith('ext-ms-')) { return $true }
  return (Test-Path "$env:WINDIR\System32\$dll") -and -not ($d -match '^(vcruntime|msvcp|vcomp|concrt|vccorlib)')
}

$manifest = @("PhotoWorkflow distribution  $(Get-Date -Format s)", "exe: $exe", "")
$queue = New-Object System.Collections.Queue
$queue.Enqueue($exe)
$seen = @{}
while ($queue.Count -gt 0) {
  $file = $queue.Dequeue()
  foreach ($dll in Get-Imports $file) {
    $key = $dll.ToLower()
    if ($seen.ContainsKey($key)) { continue }
    $seen[$key] = $true
    if (Is-System $dll) { $manifest += "system   $dll"; continue }
    $src = $redistDirs | ForEach-Object { Join-Path $_.FullName $dll } | Where-Object { Test-Path $_ } | Select-Object -First 1
    if (-not $src) { $manifest += "MISSING  $dll"; continue }
    Copy-Item $src $dist
    $manifest += "shipped  $dll  <- $src"
    $queue.Enqueue((Join-Path $dist $dll))
  }
}
$manifest += ""
foreach ($f in Get-ChildItem $dist -File) {
  $manifest += "{0,-28} {1,12:N0} bytes  sha256 {2}" -f $f.Name, $f.Length, (Get-FileHash $f.FullName -Algorithm SHA256).Hash
}
$manifest | Set-Content "$dist\dist-manifest.txt"
$manifest | Write-Output
if ($manifest -match '^MISSING') { throw 'a required DLL was not found - see dist-manifest.txt' }
@"
PhotoWorkflow - standalone folder
Requirements: Windows 10/11 x64 with the Microsoft Edge WebView2 Runtime (included in Windows 11).
Ratings, labels and IPTC need ExifTool (exiftool.exe on PATH); it is not included.
The MSVC runtime DLLs in this folder are Microsoft Visual C++ Redistributable files, deployed
app-locally as Microsoft permits. vcomp140.dll is the OpenMP runtime used by LibRaw.
"@ | Set-Content "$dist\README.txt"
Write-Host "`nDistribution: $dist" -ForegroundColor Green
