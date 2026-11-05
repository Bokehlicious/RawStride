# Runs the app's built-in measurement (ui/web/bench.js) once and samples CPU and memory of the
# app process and its WebView2 processes every 250 ms. ADR 0006 steps 3 and 4.
param(
  [Parameter(Mandatory)][string]$Exe,
  [Parameter(Mandatory)][ValidateSet('on','off')][string]$Raw,
  [Parameter(Mandatory)][string]$OutPrefix,
  [string]$Folder = 'D:\PhotoWorkflowData\RAW',
  # Long-edge limit for half-size images; -1 = the app's default (screen size), 0 = full half size.
  [int]$HalfMax = -1,
  # Reuse of the half-size job's RAW session for tiles (step 4); 'off' only to compare.
  [ValidateSet('on','off')][string]$Reuse = 'on',
  [int]$TimeoutSeconds = 480
)
$ErrorActionPreference = 'Stop'
Get-Process photoworkflow -ErrorAction SilentlyContinue | Stop-Process -Force
Start-Sleep -Seconds 1
$json = "$OutPrefix.json"
$csv = "$OutPrefix-samples.csv"
if (Test-Path $json) { Remove-Item $json }
'epoch_ms,app_cpu_ms,webview_cpu_ms,app_private_mb,app_ws_mb,webview_private_mb,webview_ws_mb,webview_procs' | Set-Content $csv
$appArgs = @("`"$Folder`"", '--bench', "`"$json`"", '--bench-raw', $Raw)
if ($HalfMax -ge 0) { $appArgs += @('--bench-half-max', "$HalfMax") }
if ($Reuse -eq 'off') { $appArgs += @('--bench-reuse', 'off') }
$p = Start-Process $Exe -ArgumentList $appArgs -PassThru
$sw = [Diagnostics.Stopwatch]::StartNew()
$children = @()
$lastScan = -10000
function Descendants([int]$root) {
  $all = Get-CimInstance Win32_Process -Property ProcessId, ParentProcessId
  $set = New-Object System.Collections.Generic.HashSet[int]
  $queue = New-Object System.Collections.Queue
  $queue.Enqueue($root)
  while ($queue.Count) {
    $id = $queue.Dequeue()
    foreach ($c in $all | Where-Object { $_.ParentProcessId -eq $id }) {
      if ($set.Add([int]$c.ProcessId)) { $queue.Enqueue([int]$c.ProcessId) }
    }
  }
  return @($set)
}
while (-not $p.HasExited -and $sw.Elapsed.TotalSeconds -lt $TimeoutSeconds) {
  if ($sw.ElapsedMilliseconds - $lastScan -gt 1500) { $children = Descendants $p.Id; $lastScan = $sw.ElapsedMilliseconds }
  try {
    $p.Refresh()
    $wv = @($children | ForEach-Object { Get-Process -Id $_ -ErrorAction SilentlyContinue })
    # Invariant culture: a decimal comma would break the CSV.
    $line = [string]::Format([Globalization.CultureInfo]::InvariantCulture, '{0},{1:F0},{2:F0},{3:F1},{4:F1},{5:F1},{6:F1},{7}',
      ([DateTimeOffset]::UtcNow.ToUnixTimeMilliseconds()),
      $p.TotalProcessorTime.TotalMilliseconds,
      (($wv | ForEach-Object { $_.TotalProcessorTime.TotalMilliseconds } | Measure-Object -Sum).Sum -as [double]),
      ($p.PrivateMemorySize64 / 1MB), ($p.WorkingSet64 / 1MB),
      ((($wv | Measure-Object -Property PrivateMemorySize64 -Sum).Sum) / 1MB),
      ((($wv | Measure-Object -Property WorkingSet64 -Sum).Sum) / 1MB),
      $wv.Count)
    Add-Content $csv $line
  } catch { }
  Start-Sleep -Milliseconds 250
}
if (-not $p.HasExited) { Stop-Process -Id $p.Id -Force; throw "measurement run timed out ($Raw)" }
if (-not (Test-Path $json)) { throw "no results written ($Raw)" }
Write-Host "run $Raw done in $([int]$sw.Elapsed.TotalSeconds) s: $json"
