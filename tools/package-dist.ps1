# Makes RawStride's portable zip (ADR 0031 RL4, task T228):
#
#     target\dist\RawStride-<version>-windows-x64\      the program folder
#     target\dist\RawStride-<version>-windows-x64.zip   the same folder as a zip
#
# The folder holds rawstride.exe, the four Microsoft Visual C++ runtime DLLs it imports (copied
# app-locally from the Visual Studio or Build Tools redistributable folder), ExifTool unmodified
# (exiftool.exe and exiftool_files, ADR 0002 section 2.5), LICENSE, THIRD-PARTY-NOTICES.txt and a
# short README.txt. Unpacking the zip is the installation.
#
# Run it from the repository root (docs/distribution.md):
#
#     .\tools\package-dist.ps1                      builds the release exe first
#     .\tools\package-dist.ps1 -NoBuild             uses the existing target\release\rawstride.exe
#     .\tools\package-dist.ps1 -ExifToolZip <path>  uses a local exiftool-13.59_64.zip
#
# Everything it writes is under target\dist\. A zip made on a developer's machine is for testing
# only; a release zip is made by the release workflow (ADR 0031 RL6).

param(
    [switch]$NoBuild,
    [string]$ExifToolZip
)

$ErrorActionPreference = 'Stop'

# Both must match EXIFTOOL_VERSION / EXIFTOOL_SHA256 in .github/workflows/ci.yml and the ExifTool
# version in tools/third-party-notices.ps1.
$exifToolVersion = '13.59'
$exifToolSha256 = '44b512b25af500724ba579d0a53c8fc5851628b692dd5e5d94ae4a15c2cba9ec'

# The Microsoft DLLs rawstride.exe needs; THIRD-PARTY-NOTICES.txt's Microsoft paragraph names the
# same four.
$expectedDlls = @('MSVCP140.dll', 'VCOMP140.DLL', 'VCRUNTIME140.dll', 'VCRUNTIME140_1.dll')

$repositoryRoot = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$distRoot = Join-Path $repositoryRoot 'target\dist'
$utf8WithoutBom = New-Object System.Text.UTF8Encoding($false)

# Runs a native program and returns its standard output as an array of lines. Its standard error
# goes to the console. In Windows PowerShell 5.1 a line on a native program's standard error becomes
# a terminating error under 'Stop' when output is redirected, so 'Continue' is set around the call.
# Stops if the program fails.
function Invoke-Native {
    param(
        [string]$Program,
        [string[]]$Arguments
    )

    $previousPreference = $ErrorActionPreference
    $ErrorActionPreference = 'Continue'
    try {
        $output = & $Program @Arguments
        $exitCode = $LASTEXITCODE
    }
    finally {
        $ErrorActionPreference = $previousPreference
    }
    if ($exitCode -ne 0) {
        throw "$Program $($Arguments -join ' ') failed with exit code $exitCode."
    }
    return @($output)
}

# The DLL names a PE file imports, from dumpbin.
function Get-Imports {
    param(
        [string]$Dumpbin,
        [string]$File
    )

    $lines = Invoke-Native $Dumpbin @('/nologo', '/dependents', $File)
    return @($lines | ForEach-Object { "$_".Trim() } | Where-Object { $_ -match '^[\w\-\.]+\.dll$' })
}

# True for a DLL that is part of Windows itself (never shipped).
function Test-SystemDll {
    param([string]$Dll)

    $name = $Dll.ToLowerInvariant()
    if ($name.StartsWith('api-ms-win-') -or $name.StartsWith('ext-ms-')) {
        return $true
    }
    $inSystem32 = Test-Path -LiteralPath (Join-Path $env:WINDIR "System32\$Dll") -PathType Leaf
    return $inSystem32 -and -not ($name -match '^(vcruntime|msvcp|vcomp|concrt|vccorlib)')
}

function Get-Sha256 {
    param([string]$Path)

    return (Get-FileHash -LiteralPath $Path -Algorithm SHA256).Hash.ToLowerInvariant()
}

function Remove-IfPresent {
    param([string]$Path)

    if (Test-Path -LiteralPath $Path) {
        Remove-Item -LiteralPath $Path -Recurse -Force
    }
}

$unpackDir = $null
Push-Location $repositoryRoot
try {
    # --- Version and repository URL: pw-loupe's, from cargo metadata. ---
    $metadataLines = Invoke-Native 'cargo' @('metadata', '--format-version', '1', '--no-deps', '--locked', '--offline')
    $metadata = ($metadataLines -join "`n") | ConvertFrom-Json
    $loupePackages = @($metadata.packages | Where-Object { $_.name -eq 'pw-loupe' })
    if ($loupePackages.Count -ne 1) {
        throw "cargo metadata lists $($loupePackages.Count) packages named pw-loupe, expected 1."
    }
    $version = $loupePackages[0].version
    $repositoryUrl = $loupePackages[0].repository
    if ([string]::IsNullOrWhiteSpace($version) -or [string]::IsNullOrWhiteSpace($repositoryUrl)) {
        throw "pw-loupe has no version or no repository in cargo metadata."
    }
    $packageName = "RawStride-$version-windows-x64"
    Write-Output "RawStride $version ($repositoryUrl)"

    # --- The exe. ---
    $exe = Join-Path $repositoryRoot 'target\release\rawstride.exe'
    if ($NoBuild) {
        if (-not (Test-Path -LiteralPath $exe -PathType Leaf)) {
            throw "-NoBuild was given but $exe does not exist. Build it first (cargo build --release --locked -p pw-loupe) or run without -NoBuild."
        }
        Write-Output "Using the existing $exe, last written $((Get-Item -LiteralPath $exe).LastWriteTime.ToString('yyyy-MM-dd HH:mm:ss'))"
    }
    else {
        Write-Output 'Building: cargo build --release --locked -p pw-loupe'
        $null = Invoke-Native 'cargo' @('build', '--release', '--locked', '-p', 'pw-loupe')
        if (-not (Test-Path -LiteralPath $exe -PathType Leaf)) {
            throw "The build succeeded but $exe does not exist."
        }
    }

    $licensePath = Join-Path $repositoryRoot 'LICENSE'
    $noticesPath = Join-Path $repositoryRoot 'THIRD-PARTY-NOTICES.txt'
    foreach ($required in @($licensePath, $noticesPath)) {
        if (-not (Test-Path -LiteralPath $required -PathType Leaf)) {
            throw "$required is missing in the repository root."
        }
    }

    # --- Output folders. ---
    New-Item -ItemType Directory -Force -Path $distRoot | Out-Null
    $programDir = Join-Path $distRoot $packageName
    $zipPath = Join-Path $distRoot "$packageName.zip"
    Remove-IfPresent $programDir
    Remove-IfPresent $zipPath
    New-Item -ItemType Directory -Force -Path $programDir | Out-Null
    Copy-Item -LiteralPath $exe -Destination $programDir

    # --- The Microsoft DLLs. ---
    $vswhere = Join-Path ${env:ProgramFiles(x86)} 'Microsoft Visual Studio\Installer\vswhere.exe'
    if (-not (Test-Path -LiteralPath $vswhere -PathType Leaf)) {
        throw "vswhere.exe not found at $vswhere (Visual Studio Installer)."
    }
    $vsLines = Invoke-Native $vswhere @('-latest', '-products', '*', '-property', 'installationPath')
    $vsPath = @($vsLines | Where-Object { -not [string]::IsNullOrWhiteSpace($_) } | Select-Object -First 1)
    if ($vsPath.Count -ne 1) {
        throw 'vswhere found no Visual Studio or Build Tools installation.'
    }
    $vsPath = "$($vsPath[0])".Trim()

    function Get-VersionFolders {
        param([string]$Path)

        if (-not (Test-Path -LiteralPath $Path -PathType Container)) {
            return @()
        }
        $folders = foreach ($folder in Get-ChildItem -LiteralPath $Path -Directory) {
            $parsed = $null
            if ([version]::TryParse($folder.Name, [ref]$parsed)) {
                [pscustomobject]@{ Folder = $folder; Version = $parsed }
            }
        }
        return @($folders | Sort-Object Version -Descending)
    }

    $toolsFolders = Get-VersionFolders (Join-Path $vsPath 'VC\Tools\MSVC')
    if ($toolsFolders.Count -eq 0) {
        throw "No VC\Tools\MSVC\<version> folder under $vsPath."
    }
    $dumpbin = Join-Path $toolsFolders[0].Folder.FullName 'bin\Hostx64\x64\dumpbin.exe'
    if (-not (Test-Path -LiteralPath $dumpbin -PathType Leaf)) {
        throw "dumpbin.exe not found at $dumpbin."
    }

    $redistFolder = Get-VersionFolders (Join-Path $vsPath 'VC\Redist\MSVC') |
        Where-Object { Test-Path -LiteralPath (Join-Path $_.Folder.FullName 'x64') -PathType Container } |
        Select-Object -First 1
    if (-not $redistFolder) {
        throw "No VC\Redist\MSVC\<version>\x64 folder under $vsPath."
    }
    $redistX64 = Join-Path $redistFolder.Folder.FullName 'x64'
    $redistDirs = @(Get-ChildItem -LiteralPath $redistX64 -Directory |
        Where-Object { $_.Name -like '*.CRT' -or $_.Name -like '*.OpenMP' })
    if ($redistDirs.Count -eq 0) {
        throw "No *.CRT or *.OpenMP folder under $redistX64."
    }
    Write-Output "dumpbin: $dumpbin"
    Write-Output "Redistributable folders: $(($redistDirs | ForEach-Object { $_.FullName }) -join ', ')"

    $queue = New-Object System.Collections.Queue
    $queue.Enqueue((Join-Path $programDir 'rawstride.exe'))
    $seen = @{}
    $copied = New-Object System.Collections.Generic.List[string]
    while ($queue.Count -gt 0) {
        $file = $queue.Dequeue()
        foreach ($dll in Get-Imports $dumpbin $file) {
            $key = $dll.ToLowerInvariant()
            if ($seen.ContainsKey($key)) {
                continue
            }
            $seen[$key] = $true
            if (Test-SystemDll $dll) {
                continue
            }
            $source = $null
            foreach ($dir in $redistDirs) {
                $match = @(Get-ChildItem -LiteralPath $dir.FullName -File | Where-Object { $_.Name -eq $dll })
                if ($match.Count -gt 0) {
                    $source = $match[0]
                    break
                }
            }
            if (-not $source) {
                throw "$([System.IO.Path]::GetFileName($file)) imports $dll, which is not part of Windows and not in the redistributable folders under $redistX64."
            }
            Copy-Item -LiteralPath $source.FullName -Destination $programDir
            Write-Output "Copied $($source.Name) <- $($source.FullName)"
            $copied.Add($source.Name)
            $queue.Enqueue((Join-Path $programDir $source.Name))
        }
    }
    $copiedSorted = @($copied | ForEach-Object { $_.ToLowerInvariant() } | Sort-Object)
    $expectedSorted = @($expectedDlls | ForEach-Object { $_.ToLowerInvariant() } | Sort-Object)
    if (($copiedSorted -join ',') -ne ($expectedSorted -join ',')) {
        throw "The DLLs copied ($($copied -join ', ')) are not exactly $($expectedDlls -join ', '). THIRD-PARTY-NOTICES.txt's Microsoft paragraph and this script's list must then be updated."
    }

    # --- ExifTool. ---
    $exifToolName = "exiftool-$($exifToolVersion)_64"
    if ($ExifToolZip) {
        if (-not (Test-Path -LiteralPath $ExifToolZip -PathType Leaf)) {
            throw "-ExifToolZip: $ExifToolZip does not exist."
        }
        $exifZip = (Resolve-Path -LiteralPath $ExifToolZip).Path
        $isCache = $false
    }
    else {
        $downloadDir = Join-Path $distRoot 'download'
        New-Item -ItemType Directory -Force -Path $downloadDir | Out-Null
        $exifZip = Join-Path $downloadDir "$exifToolName.zip"
        $isCache = $true
        if (Test-Path -LiteralPath $exifZip -PathType Leaf) {
            Write-Output "Using the cached $exifZip"
        }
        else {
            $partPath = "$exifZip.part"
            Remove-IfPresent $partPath
            $url = "https://sourceforge.net/projects/exiftool/files/$exifToolName.zip/download"
            Write-Output "Downloading $url"
            $null = Invoke-Native 'curl.exe' @('-L', '--fail', '--silent', '--show-error', '--retry', '3', '-o', $partPath, $url)
            Move-Item -LiteralPath $partPath -Destination $exifZip
        }
    }
    $actualSha256 = Get-Sha256 $exifZip
    if ($actualSha256 -ne $exifToolSha256.ToLowerInvariant()) {
        if ($isCache) {
            Remove-Item -LiteralPath $exifZip -Force
        }
        throw "SHA-256 mismatch for $exifZip`: expected $exifToolSha256, got $actualSha256."
    }

    $unpackDir = Join-Path $distRoot 'unpack-exiftool'
    Remove-IfPresent $unpackDir
    Expand-Archive -LiteralPath $exifZip -DestinationPath $unpackDir
    $exifSourceDir = Join-Path $unpackDir $exifToolName
    $exifSourceExe = Join-Path $exifSourceDir 'exiftool(-k).exe'
    $exifSourceFiles = Join-Path $exifSourceDir 'exiftool_files'
    if (-not (Test-Path -LiteralPath $exifSourceExe -PathType Leaf)) {
        throw "exiftool(-k).exe is missing in $exifSourceDir."
    }
    if (-not (Test-Path -LiteralPath $exifSourceFiles -PathType Container)) {
        throw "exiftool_files is missing in $exifSourceDir."
    }
    $exifExe = Join-Path $programDir 'exiftool.exe'
    Copy-Item -LiteralPath $exifSourceExe -Destination $exifExe
    Copy-Item -LiteralPath $exifSourceFiles -Destination (Join-Path $programDir 'exiftool_files') -Recurse
    $exifVersionLines = @(Invoke-Native $exifExe @('-ver'))
    $exifVersionSeen = ''
    if ($exifVersionLines.Count -gt 0) {
        $exifVersionSeen = "$($exifVersionLines[0])".Trim()
    }
    if ($exifVersionSeen -ne $exifToolVersion) {
        throw "The copied exiftool.exe reports version '$exifVersionSeen', expected $exifToolVersion."
    }
    Write-Output "ExifTool $exifVersionSeen copied from $exifZip"

    # --- LICENSE, notices and README.txt. ---
    Copy-Item -LiteralPath $licensePath -Destination $programDir
    Copy-Item -LiteralPath $noticesPath -Destination $programDir

    $readme = @'
RawStride <version>

A photo browser and metadata editor for Windows 10 and 11 (x64).

Installing: unpack this folder anywhere you like and start rawstride.exe. Nothing else
is needed; ExifTool and the Microsoft Visual C++ runtime files are included.

The program is not signed yet, so Windows SmartScreen may warn the first time it is
started ("Windows protected your PC": choose "More info", then "Run anyway"). Compare
the zip's SHA-256 checksum with the one on the release page first.

Settings are kept in %LOCALAPPDATA%\RawStride. To remove RawStride, delete this folder
and that one. The program writes nothing into its own folder.

RawStride is free software under the GNU General Public License, version 3 or later;
see LICENSE. It comes with no warranty. The notices of the software it includes are in
THIRD-PARTY-NOTICES.txt. Source code, releases and the change log:
<repository>
'@
    $readme = $readme.Replace("`r`n", "`n").Replace("`n", "`r`n")
    $readme = $readme.Replace('<version>', $version).Replace('<repository>', $repositoryUrl) + "`r`n"
    [System.IO.File]::WriteAllText((Join-Path $programDir 'README.txt'), $readme, $utf8WithoutBom)

    # --- Check the folder. ---
    $expectedTop = @('rawstride.exe') + $expectedDlls + @('exiftool.exe', 'exiftool_files', 'LICENSE', 'THIRD-PARTY-NOTICES.txt', 'README.txt')
    $actualTop = @(Get-ChildItem -LiteralPath $programDir -Force | ForEach-Object { $_.Name.ToLowerInvariant() } | Sort-Object)
    $expectedTopSorted = @($expectedTop | ForEach-Object { $_.ToLowerInvariant() } | Sort-Object)
    if (($actualTop -join ',') -ne ($expectedTopSorted -join ',')) {
        throw "The program folder holds $($actualTop -join ', '), expected exactly $($expectedTop -join ', ')."
    }
    $exifFilesDir = Get-Item -LiteralPath (Join-Path $programDir 'exiftool_files')
    if (-not $exifFilesDir.PSIsContainer) {
        throw 'exiftool_files in the program folder is not a folder.'
    }
    $emptyDirs = @(Get-ChildItem -LiteralPath $programDir -Directory -Recurse -Force |
        Where-Object { @(Get-ChildItem -LiteralPath $_.FullName -Force).Count -eq 0 })
    if ($emptyDirs.Count -gt 0) {
        throw "The program folder holds empty folders, which the zip would lose: $(($emptyDirs | ForEach-Object { $_.FullName }) -join ', ')"
    }

    # --- Zip: one entry per file, names with '/' under the package folder. ---
    Add-Type -AssemblyName System.IO.Compression, System.IO.Compression.FileSystem
    $files = @(Get-ChildItem -LiteralPath $programDir -File -Recurse -Force)
    $programDirPrefix = $programDir.TrimEnd('\') + '\'
    $zipStream = [System.IO.File]::Open($zipPath, [System.IO.FileMode]::CreateNew)
    try {
        $archive = New-Object System.IO.Compression.ZipArchive($zipStream, [System.IO.Compression.ZipArchiveMode]::Create)
        try {
            foreach ($file in $files) {
                if (-not $file.FullName.StartsWith($programDirPrefix, [System.StringComparison]::OrdinalIgnoreCase)) {
                    throw "$($file.FullName) is not under $programDir."
                }
                $relative = $file.FullName.Substring($programDirPrefix.Length).Replace('\', '/')
                $entryName = "$packageName/$relative"
                $null = [System.IO.Compression.ZipFileExtensions]::CreateEntryFromFile($archive, $file.FullName, $entryName, [System.IO.Compression.CompressionLevel]::Optimal)
            }
        }
        finally {
            $archive.Dispose()
        }
    }
    finally {
        $zipStream.Dispose()
    }

    # --- Check the zip. ---
    $readArchive = [System.IO.Compression.ZipFile]::OpenRead($zipPath)
    try {
        $entries = @($readArchive.Entries)
        if ($entries.Count -ne $files.Count) {
            throw "The zip has $($entries.Count) entries, but the program folder has $($files.Count) files."
        }
        $backslashEntries = @($entries | Where-Object { $_.FullName.Contains('\') })
        if ($backslashEntries.Count -gt 0) {
            throw "Zip entry names hold '\': $(($backslashEntries | ForEach-Object { $_.FullName }) -join ', ')"
        }
    }
    finally {
        $readArchive.Dispose()
    }

    $zipItem = Get-Item -LiteralPath $zipPath
    Write-Output ''
    Write-Output "Folder:  $programDir"
    Write-Output "Zip:     $zipPath"
    Write-Output "Size:    $($zipItem.Length) bytes"
    Write-Output "SHA-256: $(Get-Sha256 $zipPath)"
    Write-Output "Entries: $($files.Count)"
}
finally {
    if ($unpackDir) {
        Remove-IfPresent $unpackDir
    }
    Pop-Location
}
