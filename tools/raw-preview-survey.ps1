<#
.SYNOPSIS
  Survey the embedded previews in RAW files (Sony ARW, Panasonic RW2, DNG). Read-only on the RAW files.

.DESCRIPTION
  1. Reads metadata for every RAW file under -RawDir with ExifTool (batch, one process).
  2. Picks up to -PerGroup files per (make, model, RAW type/compression).
  3. Extracts every embedded preview (ExifTool group "Preview") from those files
     and measures each extracted image (dimensions, bytes).
  Results are CSV files in -OutDir. The RAW files themselves are never modified.

.EXAMPLE
  .\tools\raw-preview-survey.ps1
  .\tools\raw-preview-survey.ps1 -Extensions .rw2
  .\tools\raw-preview-survey.ps1 -Extensions .dng

  Note: only previews ExifTool can extract as files (normally JPEG) end up in previews.csv.
  Uncompressed RGB previews, which some DNG writers use, appear only in metadata.csv.
#>
param(
    [string]  $RawDir     = 'D:\PhotoWorkflowData\RAW',
    [string]  $OutDir     = "D:\PhotoWorkflowData\analysis\raw-previews-$(Get-Date -Format yyyyMMdd-HHmmss)",
    [string[]]$Extensions = @('.arw', '.rw2', '.dng'),
    [int]     $PerGroup   = 3
)
$ErrorActionPreference = 'Stop'
if (-not (Get-Command exiftool -ErrorAction SilentlyContinue)) { throw 'exiftool not found on PATH' }
$exts  = $Extensions | ForEach-Object { $_.ToLowerInvariant() }
$files = @(Get-ChildItem $RawDir -Recurse -File | Where-Object { $exts -contains $_.Extension.ToLowerInvariant() })
if ($files.Count -eq 0) { throw "No files with extensions $($Extensions -join ', ') under $RawDir" }
New-Item -ItemType Directory -Force $OutDir, "$OutDir\extracted" | Out-Null
$files | Group-Object { $_.Extension.ToLowerInvariant() } | ForEach-Object { Write-Host ("{0}: {1} files" -f $_.Name, $_.Count) }

# 1. Metadata for all files (# = numeric values)
$tags = @('-FileName','-Directory','-FileType','-FileSize#','-Make','-Model','-Software',
          '-SonyRawFileType','-SonyRawImageSize','-RawFormat','-Compression','-BitsPerSample',
          '-DNGVersion','-UniqueCameraModel','-OriginalRawFileName','-PhotometricInterpretation','-NewSubfileType',
          '-ImageWidth','-ImageHeight','-SensorWidth','-SensorHeight','-Orientation#',
          '-ThumbnailOffset#','-ThumbnailLength#','-PreviewImageStart#','-PreviewImageLength#',
          '-JpgFromRawStart#','-JpgFromRawLength#')
$list = "$OutDir\files.txt"
[IO.File]::WriteAllLines($list, [string[]]($files.FullName))
cmd /c "exiftool -q -q -csv -a -G1 $($tags -join ' ') -@ `"$list`" > `"$OutDir\metadata.csv`""
$meta = @(Import-Csv "$OutDir\metadata.csv")
Write-Host "Metadata rows: $($meta.Count)"

# 2. Sample per (make, model, raw type). Column names carry the ExifTool group (e.g. "IFD0:Model").
function Get-Col($row, [string]$tag) {
    $p = $row.PSObject.Properties | Where-Object { $_.Name -like "*:$tag" -and $_.Value } | Select-Object -First 1
    if ($p) { $p.Value } else { '' }
}
$sample = $meta | Group-Object {
    $type = (Get-Col $_ 'SonyRawFileType'); if (-not $type) { $type = (Get-Col $_ 'RawFormat') }
    if (-not $type -and (Get-Col $_ 'DNGVersion')) { $type = "DNG $(Get-Col $_ 'DNGVersion')" }
    if (-not $type) { $type = (Get-Col $_ 'Compression') }
    "$(Get-Col $_ 'Make') $(Get-Col $_ 'Model') | $type"
} | ForEach-Object {
    Write-Host ("  {0,-55} {1,4} files" -f $_.Name, $_.Count)
    $_.Group | Select-Object -First $PerGroup
}

# 3. Extract all embedded previews from the sample
foreach ($row in $sample) {
    & exiftool -q -q -a -b -Preview:all -W "$OutDir\extracted\%f.%e__%t%-c.%s" $row.SourceFile
}
cmd /c "exiftool -q -q -csv -FileName -FileSize# -FileType -ImageWidth -ImageHeight -EncodingProcess -YCbCrSubSampling `"$OutDir\extracted`" > `"$OutDir\previews.csv`""
$prev = @(Import-Csv "$OutDir\previews.csv")

Write-Host "`n=== Extracted previews ===" -ForegroundColor Cyan
$prev | Sort-Object FileName | Format-Table FileName, FileType, ImageWidth, ImageHeight, @{n='KB';e={[math]::Round([double]$_.FileSize/1KB)}} -AutoSize | Out-String | Write-Host
Write-Host "Results: $OutDir" -ForegroundColor Green
Write-Host "(metadata.csv, previews.csv, extracted\ - the extracted JPEGs can be deleted when done)"
