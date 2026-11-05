# ADR 0006 step 5: verifies the standalone distribution folder on its own.
#  - copies dist\PhotoWorkflow to a folder outside the repository and works only from that copy;
#  - static checks: every DLL import of every file resolves to the folder itself or to Windows;
#    the exe is searched for development paths;
#  - runs the built-in measurement (ui/web/bench.js) from the copy with a cleaned environment
#    (only Windows' own variables, PATH = Windows folders only: no cargo, vcpkg, Visual Studio,
#    ExifTool), records which DLLs the process really loads (and from where), its child processes,
#    threads, CPU and memory;
#  - runs: RAW rendering on, RAW rendering off, and a folder with broken files.
# Development tools are used only to inspect (dumpbin), never by the app.
param(
  [string]$Dist = (Join-Path $PSScriptRoot '..\dist\PhotoWorkflow'),
  [string]$TestRoot = 'D:\PhotoWorkflowData\dist-test',
  [string]$Images = 'D:\PhotoWorkflowData\RAW',
  [string]$Out = 'D:\PhotoWorkflowData\analysis\step5',
  [int]$TimeoutSeconds = 480,
  # Runs to do (default all); e.g. -Runs broken to repeat only the broken-file run.
  [string[]]$Runs = @('on', 'off', 'broken')
)
$ErrorActionPreference = 'Stop'
$Dist = (Resolve-Path $Dist).Path
New-Item -ItemType Directory -Force $Out | Out-Null
$report = New-Object System.Collections.Generic.List[string]
function Say([string]$s) { $report.Add($s); Write-Host $s }

# ---------- 1. copy outside the repository ----------
$app = Join-Path $TestRoot 'PhotoWorkflow'
Get-Process photoworkflow -ErrorAction SilentlyContinue | Stop-Process -Force
Start-Sleep -Seconds 1
if (Test-Path $app) { Remove-Item $app -Recurse -Force }
New-Item -ItemType Directory -Force $app | Out-Null
Copy-Item "$Dist\*" $app -Recurse
Say "== 1. Folder under test: $app (copied from $Dist)"
foreach ($f in Get-ChildItem $app -File) {
  $h1 = (Get-FileHash $f.FullName -Algorithm SHA256).Hash
  $h2 = (Get-FileHash (Join-Path $Dist $f.Name) -Algorithm SHA256).Hash
  Say ("   {0,-22} {1,12:N0} bytes  {2}" -f $f.Name, $f.Length, $(if ($h1 -eq $h2) { 'identical to dist' } else { 'DIFFERENT' }))
}
$before = Get-ChildItem $app -Recurse | ForEach-Object { $_.FullName } | Sort-Object

# ---------- 2. static checks ----------
$vswhere = "${env:ProgramFiles(x86)}\Microsoft Visual Studio\Installer\vswhere.exe"
$vs = & $vswhere -latest -products * -property installationPath
$msvc = Get-ChildItem "$vs\VC\Tools\MSVC" -Directory | Sort-Object Name -Descending | Select-Object -First 1
$dumpbin = "$($msvc.FullName)\bin\Hostx64\x64\dumpbin.exe"
Say ""
Say "== 2. DLL imports of every file in the folder (dumpbin /dependents)"
$missing = 0
foreach ($f in Get-ChildItem $app -File | Where-Object { $_.Extension -match '\.(exe|dll)$' }) {
  $imports = & $dumpbin /nologo /dependents $f.FullName | ForEach-Object { $_.Trim() } | Where-Object { $_ -match '^[\w\-\.]+\.dll$' }
  Say "   $($f.Name):"
  foreach ($d in $imports) {
    $where = if (Test-Path (Join-Path $app $d)) { 'in the folder' }
      elseif ($d -match '^(api|ext)-ms-') { 'Windows API set' }
      elseif (Test-Path "$env:SystemRoot\System32\$d") { 'Windows (System32)' }
      else { $missing++; 'MISSING' }
    if ($d -match '^(vcruntime|msvcp|vcomp|concrt)' -and $where -ne 'in the folder') { $missing++; $where += '  <- must be shipped (not part of Windows)' }
    Say ("      {0,-45} {1}" -f $d, $where)
  }
}
Say "   unresolved imports: $missing"
Say ""
Say "== 2b. Debug directory of the exe"
& $dumpbin /nologo /headers "$app\photoworkflow.exe" | Select-String -Pattern 'RSDS|\.pdb' | ForEach-Object { Say "   $($_.Line.Trim())" }

Say ""
Say "== 2c. Development paths inside the exe (text found, with context)"
$bytes = [IO.File]::ReadAllBytes("$app\photoworkflow.exe")
$texts = @(
  @{ enc = 'ASCII'; s = [Text.Encoding]::ASCII.GetString($bytes) },
  @{ enc = 'UTF-16'; s = [Text.Encoding]::Unicode.GetString($bytes) }
)
$patterns = @('C:\Development', 'C:\\Development', 'vcpkg', '.cargo', 'rustup', 'Visual Studio', 'target\release',
  'localhost:1420', 'D:\PhotoWorkflowData', 'ui\web', 'ui/web', '.pdb')
foreach ($p in $patterns) {
  foreach ($t in $texts) {
    $n = 0; $idx = 0; $ctx = @()
    while (($idx = $t.s.IndexOf($p, $idx, [StringComparison]::OrdinalIgnoreCase)) -ge 0) {
      $n++
      if ($ctx.Count -lt 4) {
        $a = [Math]::Max(0, $idx - 40); $c = $t.s.Substring($a, [Math]::Min(120, $t.s.Length - $a))
        $ctx += ($c -replace '[^\x20-\x7E]', '.')
      }
      $idx += $p.Length
    }
    if ($n) {
      Say "   '$p' ($($t.enc)): $n"
      foreach ($c in $ctx) { Say "      $c" }
    }
  }
}

# ---------- 3. test folders ----------
$broken = Join-Path $TestRoot 'broken'
if (Test-Path $broken) { Remove-Item $broken -Recurse -Force }
New-Item -ItemType Directory -Force $broken | Out-Null
foreach ($n in 'TN100424.ARW', 'P1240233.RW2', 'TN005960.ARW') { Copy-Item (Join-Path $Images $n) $broken }
function Truncated([string]$src, [string]$dst, [double]$part) {
  $b = [IO.File]::ReadAllBytes($src)
  $len = [int]($b.Length * $part)
  $o = New-Object byte[] $len
  [Array]::Copy($b, $o, $len)
  [IO.File]::WriteAllBytes($dst, $o)
}
Truncated (Join-Path $Images 'TN100423.ARW') (Join-Path $broken 'ZZ_broken_a7r_truncated.ARW') 0.4
# Cut before or inside the RAW data, to find where LibRaw itself gives up.
foreach ($part in 0.02, 0.05, 0.1, 0.99) {
  Truncated (Join-Path $Images 'TN100426.ARW') (Join-Path $broken ('ZZ_broken_a7r_cut{0:D2}.ARW' -f [int]($part * 100))) $part
}
Truncated (Join-Path $Images 'P1240235.RW2') (Join-Path $broken 'ZZ_broken_rw2_cut90.RW2') 0.9
Truncated (Join-Path $Images 'IMG_20261026_062851.dng') (Join-Path $broken 'ZZ_broken_dng_cut50.dng') 0.5
# A valid image LibRaw cannot decode, with a preview smaller than the screen (so RAW rendering
# is attempted and must fail back to the preview): a JPEG named .ARW.
$jpeg = Join-Path $TestRoot 'source\small.jpg'
if (Test-Path $jpeg) { Copy-Item $jpeg (Join-Path $broken 'ZZ_broken_jpeg_as_raw.ARW') }
Truncated (Join-Path $Images 'P1240231.RW2') (Join-Path $broken 'ZZ_broken_rw2_truncated.RW2') 0.4
$rnd = New-Object Random 42
$g = New-Object byte[] (3MB); $rnd.NextBytes($g)
[IO.File]::WriteAllBytes((Join-Path $broken 'ZZ_broken_garbage.ARW'), $g)
Say ""
Say "== 3. Broken-file folder: $broken"
Get-ChildItem $broken | ForEach-Object { Say ("   {0,-32} {1,12:N0} bytes" -f $_.Name, $_.Length) }

# ---------- 4. runs with a cleaned environment ----------
$keep = 'SystemRoot', 'windir', 'SystemDrive', 'USERPROFILE', 'USERNAME', 'USERDOMAIN', 'COMPUTERNAME', 'APPDATA',
  'LOCALAPPDATA', 'TEMP', 'TMP', 'ProgramData', 'ProgramFiles', 'ProgramFiles(x86)', 'ProgramW6432', 'CommonProgramFiles',
  'CommonProgramFiles(x86)', 'CommonProgramW6432', 'PUBLIC', 'HOMEDRIVE', 'HOMEPATH', 'OS', 'PROCESSOR_ARCHITECTURE',
  'NUMBER_OF_PROCESSORS', 'ComSpec', 'PATHEXT', 'ALLUSERSPROFILE'
$cleanPath = "$env:SystemRoot\System32;$env:SystemRoot;$env:SystemRoot\System32\Wbem;$env:SystemRoot\System32\WindowsPowerShell\v1.0\"
Say ""
Say "== 4. Environment of the app in the runs: only $($keep -join ', ')"
Say "   PATH = $cleanPath"
Say "   (removed from this shell's environment: $((Get-ChildItem env: | Where-Object { $keep -notcontains $_.Name -and $_.Name -ne 'Path' } | ForEach-Object Name) -join ', '))"
Say "   exiftool on the normal PATH: $((Get-Command exiftool -ErrorAction SilentlyContinue | ForEach-Object Source) -join ', ')"

Add-Type -Namespace 'Pw' -Name PwWin -MemberDefinition @'
[DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
[DllImport("user32.dll")] public static extern bool ShowWindowAsync(IntPtr h, int cmd);
'@

function Descendants([int]$root) {
  $all = Get-CimInstance Win32_Process -Property ProcessId, ParentProcessId, Name, ExecutablePath
  $set = @{}
  $queue = New-Object System.Collections.Queue
  $queue.Enqueue($root)
  while ($queue.Count) {
    $id = $queue.Dequeue()
    foreach ($c in $all | Where-Object { $_.ParentProcessId -eq $id }) {
      if (-not $set.ContainsKey([int]$c.ProcessId)) { $set[[int]$c.ProcessId] = $c; $queue.Enqueue([int]$c.ProcessId) }
    }
  }
  return $set
}

function Run([string]$label, [string]$folder, [string]$raw) {
  $json = Join-Path $Out "bench-$label.json"
  $csv = Join-Path $Out "bench-$label-samples.csv"
  if (Test-Path $json) { Remove-Item $json }
  'epoch_ms,app_cpu_ms,webview_cpu_ms,app_private_mb,app_ws_mb,webview_private_mb,webview_ws_mb,webview_procs,app_threads' | Set-Content $csv
  $si = New-Object System.Diagnostics.ProcessStartInfo
  $si.FileName = "$app\photoworkflow.exe"
  $si.Arguments = "`"$folder`" --bench `"$json`" --bench-raw $raw"
  $si.WorkingDirectory = $TestRoot
  $si.UseShellExecute = $false
  $saved = @{}
  foreach ($k in $keep) { $v = [Environment]::GetEnvironmentVariable($k); if ($v) { $saved[$k] = $v } }
  $si.EnvironmentVariables.Clear()
  foreach ($k in $saved.Keys) { $si.EnvironmentVariables[$k] = $saved[$k] }
  $si.EnvironmentVariables['PATH'] = $cleanPath
  $p = [Diagnostics.Process]::Start($si)
  $sw = [Diagnostics.Stopwatch]::StartNew()
  # Bring the window to the front: a window covered by others gets no animation frames.
  while ($p.MainWindowHandle -eq [IntPtr]::Zero -and $sw.Elapsed.TotalSeconds -lt 15 -and -not $p.HasExited) {
    Start-Sleep -Milliseconds 100; $p.Refresh()
  }
  if ($p.MainWindowHandle -ne [IntPtr]::Zero) {
    [void][Pw.PwWin]::ShowWindowAsync($p.MainWindowHandle, 3)
    [void][Pw.PwWin]::SetForegroundWindow($p.MainWindowHandle)
  }
  $modules = @{}; $children = @{}; $kids = @{}; $lastScan = -10000; $lastMod = -10000
  while (-not $p.HasExited -and $sw.Elapsed.TotalSeconds -lt $TimeoutSeconds) {
    if ($sw.ElapsedMilliseconds - $lastScan -gt 1500) {
      $kids = Descendants $p.Id
      foreach ($k in $kids.Values) { $children["$($k.Name)|$($k.ExecutablePath)"] = $true }
      $lastScan = $sw.ElapsedMilliseconds
    }
    try {
      $p.Refresh()
      if ($sw.ElapsedMilliseconds - $lastMod -gt 2000) {
        foreach ($m in $p.Modules) { $modules[$m.FileName.ToLower()] = $m.FileName }
        $lastMod = $sw.ElapsedMilliseconds
      }
      $wv = @($kids.Keys | ForEach-Object { Get-Process -Id $_ -ErrorAction SilentlyContinue })
      $line = [string]::Format([Globalization.CultureInfo]::InvariantCulture, '{0},{1:F0},{2:F0},{3:F1},{4:F1},{5:F1},{6:F1},{7},{8}',
        ([DateTimeOffset]::UtcNow.ToUnixTimeMilliseconds()),
        $p.TotalProcessorTime.TotalMilliseconds,
        (($wv | ForEach-Object { $_.TotalProcessorTime.TotalMilliseconds } | Measure-Object -Sum).Sum -as [double]),
        ($p.PrivateMemorySize64 / 1MB), ($p.WorkingSet64 / 1MB),
        ((($wv | Measure-Object -Property PrivateMemorySize64 -Sum).Sum) / 1MB),
        ((($wv | Measure-Object -Property WorkingSet64 -Sum).Sum) / 1MB),
        $wv.Count, $p.Threads.Count)
      Add-Content $csv $line
    } catch { }
    Start-Sleep -Milliseconds 250
  }
  $timedOut = -not $p.HasExited
  if ($timedOut) { Stop-Process -Id $p.Id -Force }
  Say ""
  Say "== Run '$label' (RAW $raw, $folder): $(if ($timedOut) { 'TIMED OUT' } elseif (Test-Path $json) { "finished in $([int]$sw.Elapsed.TotalSeconds) s, exit code $($p.ExitCode)" } else { "NO RESULTS, exit code $($p.ExitCode)" })"
  Say "   DLLs loaded by the app process, by location:"
  $groups = $modules.Values | Group-Object {
    $f = $_.ToLower()
    if ($f.StartsWith($app.ToLower())) { '1 the folder under test' }
    elseif ($f.StartsWith("$env:SystemRoot\".ToLower())) { '2 Windows' }
    elseif ($f -match 'edgewebview|webview2') { '3 WebView2 runtime' }
    else { '4 OTHER' }
  } | Sort-Object Name
  foreach ($gr in $groups) {
    Say "   $($gr.Name.Substring(2)): $($gr.Count)"
    if ($gr.Name -notlike '2*') { foreach ($m in $gr.Group | Sort-Object) { Say "      $m" } }
  }
  foreach ($d in 'vcomp140.dll', 'vcruntime140.dll', 'vcruntime140_1.dll', 'msvcp140.dll') {
    $hit = $modules.Values | Where-Object { (Split-Path $_ -Leaf) -ieq $d }
    Say ("   {0,-20} {1}" -f $d, $(if ($hit) { $hit -join ', ' } else { 'not loaded' }))
  }
  Say "   child processes: $(($children.Keys | Sort-Object) -join '; ')"
  if (Test-Path $json) {
    $r = Get-Content $json -Raw | ConvertFrom-Json
    Say "   frames that fell back to a timer (window covered): $($r.frameFallbacks); page visibility at the end: $($r.visibility)"
    if ($r.fatal) { Say "   MEASUREMENT STOPPED: $($r.fatal)" }
    foreach ($e in @($r.gridErrors)) { if ($e) { Say "   image not readable (reported by the app): $($e.name): $($e.error)" } }
    foreach ($e in @($r.errors)) { if ($e) { Say "   page error: $e" } }
  }
}

if ($Runs -contains 'on') { Run 'on' $Images 'on' }
if ($Runs -contains 'off') { Run 'off' $Images 'off' }
if ($Runs -contains 'broken') { Run 'broken' $broken 'on' }

# ---------- 5. nothing written into the program folder ----------
$after = Get-ChildItem $app -Recurse | ForEach-Object { $_.FullName } | Sort-Object
Say ""
Say "== 5. Program folder after the runs: $(if (@(Compare-Object $before $after).Count -eq 0) { 'unchanged' } else { 'CHANGED: ' + ((Compare-Object $before $after | ForEach-Object InputObject) -join ', ') })"
$data = Join-Path $env:LOCALAPPDATA 'com.photoworkflow.app'
if (Test-Path $data) {
  $sz = (Get-ChildItem $data -Recurse -File -ErrorAction SilentlyContinue | Measure-Object Length -Sum).Sum
  Say ("   per-user data (WebView2 profile): {0} ({1:N1} MB)" -f $data, ($sz / 1MB))
}
$report | Set-Content (Join-Path $Out "verify-dist-$($Runs -join '-').txt")
