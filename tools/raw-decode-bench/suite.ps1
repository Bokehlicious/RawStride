# Shared measurement steps for one bench executable. Called by run-tiles.ps1 and run-openmp.ps1.
param(
  [Parameter(Mandatory)][string]$Exe,
  [Parameter(Mandatory)][string]$Out,
  [string[]]$Steps = @('single', 'tiles', 'ptiles', 'half', 'full'),
  [string]$Raw = 'D:\PhotoWorkflowData\RAW'
)
$ErrorActionPreference = 'Stop'
New-Item -ItemType Directory -Force $Out | Out-Null
$a7r = @(Get-ChildItem $Raw -Filter 'TN1004*.ARW' | Sort-Object Name | ForEach-Object FullName)
$a1  = @(Get-ChildItem $Raw -Filter 'TN0059*.ARW' | Sort-Object Name | ForEach-Object FullName)
function Run([string]$name, [string[]]$a) {
  & $Exe @a --out "$Out\$name.csv" | Tee-Object "$Out\$name.txt"
  if ($LASTEXITCODE -ne 0) { throw "$name failed ($Exe)" }
}
foreach ($s in $Steps) {
  switch ($s) {
    'single' { Run 'single'          (@('single', $a7r[0], $a1[0], '--reps', '3')) }
    'tiles'  { Run 'tiles'           (@('tiles', $a7r[0], $a1[0], '--reps', '3')) }
    'ptiles' { Run 'ptiles-a7riva'   (@('ptiles', $a7r[0], '--threads', '1,2,4')) }
    'half'   { Run 'parallel-half'   (@('parallel') + $a7r + @('--threads', '1,2,3,4', '--config', 'half_size')) }
    'full'   { Run 'parallel-a7riva' (@('parallel') + $a7r + @('--threads', '1,2,4,8')) }
  }
}
