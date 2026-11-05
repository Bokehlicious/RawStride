# Start-up and idle memory of an app build (ADR 0006: compare every step with M1).
# Starts the exe N times with a folder, measures the time until its main window exists, waits,
# then reads the app process's memory. WebView2 child processes are not included (LibRaw and the
# pipeline live in the app process). Writes one CSV line per run.
param(
  [Parameter(Mandatory)][string]$Exe,
  [Parameter(Mandatory)][string]$Label,
  [Parameter(Mandatory)][string]$Csv,
  [string]$Folder = 'D:\PhotoWorkflowData\RAW',
  [int]$Runs = 5,
  [int]$IdleSeconds = 6
)
$ErrorActionPreference = 'Stop'
if (-not (Test-Path $Csv)) { 'label,run,window_ms,working_set_mb,private_mb,exe_mb' | Set-Content $Csv }
$exeMb = [math]::Round((Get-Item $Exe).Length / 1MB, 1)
for ($i = 0; $i -lt $Runs; $i++) {
  Get-Process photoworkflow -ErrorAction SilentlyContinue | Stop-Process -Force
  Start-Sleep -Milliseconds 800
  $sw = [Diagnostics.Stopwatch]::StartNew()
  $p = Start-Process $Exe -ArgumentList "`"$Folder`"" -PassThru
  while ($p.MainWindowHandle -eq 0 -and $sw.ElapsedMilliseconds -lt 30000) {
    Start-Sleep -Milliseconds 5
    $p.Refresh()
  }
  $win = $sw.ElapsedMilliseconds
  Start-Sleep -Seconds $IdleSeconds
  $p.Refresh()
  $ws = [math]::Round($p.WorkingSet64 / 1MB, 1)
  $pm = [math]::Round($p.PrivateMemorySize64 / 1MB, 1)
  "$Label,$i,$win,$ws,$pm,$exeMb" | Add-Content $Csv
  Write-Host ("{0,-10} run {1}: window {2} ms, working set {3} MB, private {4} MB" -f $Label, $i, $win, $ws, $pm)
  Stop-Process -Id $p.Id -Force
}
