# Builds a RawStride release (ADR 0031 RL6, task T229). In this order, stopping at the first
# failure:
#
#   1. reads pw-loupe's version and the workspace root from cargo metadata; a -Section other than
#      Unreleased must equal that version;
#   2. writes the release notes, target\dist\release-notes.md, from CHANGELOG.md's section
#      -Section and a fixed Download text;
#   3. unless -SkipNotices: fetches every package in Cargo.lock, regenerates
#      THIRD-PARTY-NOTICES.txt (tools/third-party-notices.ps1, needs cargo-about 0.9.3) and stops
#      if it differs from the committed file;
#   4. builds the release exe with the build machine's paths remapped (--remap-path-prefix through
#      CARGO_ENCODED_RUSTFLAGS, for that build only; Cargo.toml's release profile is not changed);
#   5. checks that the exe holds no repository, cargo home or user folder path and that it names
#      its PDB as rawstride.pdb only;
#   6. makes the portable zip (tools/package-dist.ps1 -NoBuild);
#   7. writes target\dist\SHA256SUMS.txt for the zip.
#
# Run it from the repository root (docs/distribution.md):
#
#     .\tools\release-build.ps1 -Section Unreleased
#     .\tools\release-build.ps1 -Section 0.1.0
#     .\tools\release-build.ps1 -Section Unreleased -SkipNotices   (without cargo-about)
#
# The release workflow (.github/workflows/release.yml) runs it on a pushed v* tag; a zip made on a
# developer's machine is for testing only. A local run rebuilds target\release\ with the remapping;
# a later plain cargo build --release rebuilds it back.

param(
    [Parameter(Mandatory = $true)]
    [string]$Section,
    [switch]$SkipNotices
)

$ErrorActionPreference = 'Stop'

$utf8WithoutBom = New-Object System.Text.UTF8Encoding($false)

# Runs a native program, shows its standard output (or hands it back through -Output) and returns
# its exit code. Its standard error goes to the console. In Windows PowerShell 5.1 a line on a
# native program's standard error becomes a terminating error under 'Stop' when output is
# redirected, so 'Continue' is set around the call. Stops unless the exit code is one of
# -AllowedExitCodes (default only 0).
function Invoke-Native {
    param(
        [string]$Program,
        [string[]]$Arguments,
        [int[]]$AllowedExitCodes = @(0),
        [ref]$Output
    )

    $previousPreference = $ErrorActionPreference
    $ErrorActionPreference = 'Continue'
    try {
        $lines = @(& $Program @Arguments)
        $exitCode = $LASTEXITCODE
    }
    finally {
        $ErrorActionPreference = $previousPreference
    }
    if ($PSBoundParameters.ContainsKey('Output')) {
        $Output.Value = $lines
    }
    elseif ($lines.Count -gt 0) {
        $lines | Out-Host
    }
    if ($AllowedExitCodes -notcontains $exitCode) {
        throw "$Program $($Arguments -join ' ') failed with exit code $exitCode."
    }
    return $exitCode
}

# The lines of CHANGELOG.md's section $Name (without its heading), trailing whitespace removed,
# blank lines removed from the start and end.
function Get-ChangelogSection {
    param(
        [string]$Path,
        [string]$Name
    )

    $text = [System.IO.File]::ReadAllText($Path, $utf8WithoutBom)
    $text = $text.Replace("`r`n", "`n").Replace("`r", "`n")
    $lines = @($text.Split("`n") | ForEach-Object { $_.TrimEnd() })

    $exactHeading = "## [$Name]"
    $datedHeading = "## [$Name] - "
    $headingIndexes = @()
    for ($i = 0; $i -lt $lines.Count; $i++) {
        $line = $lines[$i]
        if ($line.Equals($exactHeading, [System.StringComparison]::Ordinal) -or
            $line.StartsWith($datedHeading, [System.StringComparison]::Ordinal)) {
            $headingIndexes += $i
        }
    }
    if ($headingIndexes.Count -eq 0) {
        throw "CHANGELOG.md has no section '$exactHeading'."
    }
    if ($headingIndexes.Count -gt 1) {
        throw "CHANGELOG.md has $($headingIndexes.Count) headings for the section '$exactHeading', expected 1."
    }

    $body = New-Object System.Collections.Generic.List[string]
    for ($i = $headingIndexes[0] + 1; $i -lt $lines.Count; $i++) {
        $line = $lines[$i]
        if ($line.StartsWith('## ', [System.StringComparison]::Ordinal) -or $line -match '^\[[^\]]*\]: ') {
            break
        }
        $body.Add($line)
    }
    while ($body.Count -gt 0 -and $body[0].Length -eq 0) {
        $body.RemoveAt(0)
    }
    while ($body.Count -gt 0 -and $body[$body.Count - 1].Length -eq 0) {
        $body.RemoveAt($body.Count - 1)
    }
    if ($body.Count -eq 0) {
        throw "CHANGELOG.md's section '$exactHeading' is empty."
    }
    return , $body.ToArray()
}

# The path of dumpbin.exe in the newest MSVC tools folder of the newest Visual Studio or Build
# Tools installation, found as tools/package-dist.ps1 finds it.
function Get-Dumpbin {
    $vswhere = Join-Path ${env:ProgramFiles(x86)} 'Microsoft Visual Studio\Installer\vswhere.exe'
    if (-not (Test-Path -LiteralPath $vswhere -PathType Leaf)) {
        throw "vswhere.exe not found at $vswhere (Visual Studio Installer)."
    }
    $vsLines = $null
    $null = Invoke-Native $vswhere @('-latest', '-products', '*', '-property', 'installationPath') -Output ([ref]$vsLines)
    $vsPath = @($vsLines | Where-Object { -not [string]::IsNullOrWhiteSpace($_) } | Select-Object -First 1)
    if ($vsPath.Count -ne 1) {
        throw 'vswhere found no Visual Studio or Build Tools installation.'
    }
    $toolsRoot = Join-Path "$($vsPath[0])".Trim() 'VC\Tools\MSVC'
    if (-not (Test-Path -LiteralPath $toolsRoot -PathType Container)) {
        throw "No VC\Tools\MSVC folder in the Visual Studio installation."
    }
    $toolsFolders = @(foreach ($folder in Get-ChildItem -LiteralPath $toolsRoot -Directory) {
            $parsed = $null
            if ([version]::TryParse($folder.Name, [ref]$parsed)) {
                [pscustomobject]@{ Folder = $folder; Version = $parsed }
            }
        })
    $newest = @($toolsFolders | Sort-Object Version -Descending | Select-Object -First 1)
    if ($newest.Count -ne 1) {
        throw "No VC\Tools\MSVC\<version> folder in the Visual Studio installation."
    }
    $dumpbin = Join-Path $newest[0].Folder.FullName 'bin\Hostx64\x64\dumpbin.exe'
    if (-not (Test-Path -LiteralPath $dumpbin -PathType Leaf)) {
        throw "dumpbin.exe not found at $dumpbin."
    }
    return $dumpbin
}

Push-Location (Join-Path $PSScriptRoot '..')
try {
    # --- 1. Version and workspace root, from one call of cargo metadata. ---
    $metadataLines = $null
    $null = Invoke-Native 'cargo' @('metadata', '--format-version', '1', '--no-deps', '--locked', '--offline') -Output ([ref]$metadataLines)
    $metadata = ($metadataLines -join "`n") | ConvertFrom-Json
    $loupePackages = @($metadata.packages | Where-Object { $_.name -eq 'pw-loupe' })
    if ($loupePackages.Count -ne 1) {
        throw "cargo metadata lists $($loupePackages.Count) packages named pw-loupe, expected 1."
    }
    $version = $loupePackages[0].version
    # As cargo and rustc spell it: --remap-path-prefix matches with case.
    $repositoryRoot = $metadata.workspace_root
    if ([string]::IsNullOrWhiteSpace($version) -or [string]::IsNullOrWhiteSpace($repositoryRoot)) {
        throw 'cargo metadata gives no version for pw-loupe or no workspace_root.'
    }
    $repositoryRoot = $repositoryRoot.TrimEnd('\', '/')
    if ($Section -cne 'Unreleased' -and $Section -cne $version) {
        throw "-Section '$Section' is neither 'Unreleased' nor pw-loupe's version $version."
    }
    Write-Output "RawStride $version, release notes from CHANGELOG.md section [$Section]"

    $distRoot = Join-Path $repositoryRoot 'target\dist'
    $zipName = "RawStride-$version-windows-x64.zip"
    $zipPath = Join-Path $distRoot $zipName
    $sumsPath = Join-Path $distRoot 'SHA256SUMS.txt'
    $notesPath = Join-Path $distRoot 'release-notes.md'
    New-Item -ItemType Directory -Force -Path $distRoot | Out-Null
    # A checksum file from an earlier run must not stand next to a zip from this one.
    if (Test-Path -LiteralPath $sumsPath) {
        Remove-Item -LiteralPath $sumsPath -Force
    }

    # --- 2. Release notes. ---
    $sectionLines = Get-ChangelogSection (Join-Path $repositoryRoot 'CHANGELOG.md') $Section
    $download = @'
## Download

`RawStride-<version>-windows-x64.zip`: unpack it anywhere and start `rawstride.exe`; the zip's
`README.txt` says more. Windows 10 or 11, x64.

The program is not signed yet, so Windows SmartScreen may warn the first time it is started
("Windows protected your PC": choose "More info", then "Run anyway"). Compare the zip's
SHA-256 checksum with `SHA256SUMS.txt` first, for example with
`Get-FileHash RawStride-<version>-windows-x64.zip` in PowerShell.

The source code of this version is the tag's source archive below; how to build it is in
`docs/building.md`.
'@
    $download = $download.Replace("`r`n", "`n").Replace('<version>', $version)
    $notes = ($sectionLines -join "`n") + "`n`n" + $download.TrimEnd("`n") + "`n"
    [System.IO.File]::WriteAllText($notesPath, $notes, $utf8WithoutBom)
    Write-Output "Release notes: $notesPath"

    # --- 3. THIRD-PARTY-NOTICES.txt must be up to date. ---
    if ($SkipNotices) {
        Write-Output 'Notices check skipped (-SkipNotices).'
    }
    else {
        Write-Output 'Fetching every package in Cargo.lock: cargo fetch --locked'
        $null = Invoke-Native 'cargo' @('fetch', '--locked')
        & (Join-Path $repositoryRoot 'tools\third-party-notices.ps1')
        $diffCode = Invoke-Native 'git' @('diff', '--exit-code', '--stat', 'HEAD', '--', 'THIRD-PARTY-NOTICES.txt') -AllowedExitCodes @(0, 1)
        if ($diffCode -eq 1) {
            throw 'THIRD-PARTY-NOTICES.txt is out of date: run .\tools\third-party-notices.ps1 and commit the result'
        }
        Write-Output 'THIRD-PARTY-NOTICES.txt is up to date.'
    }

    # --- 4. The release exe, with the build machine's paths remapped. ---
    foreach ($name in @('RUSTFLAGS', 'CARGO_ENCODED_RUSTFLAGS')) {
        if (-not [string]::IsNullOrEmpty([System.Environment]::GetEnvironmentVariable($name))) {
            throw "$name is set; this script sets CARGO_ENCODED_RUSTFLAGS itself and would replace it. Unset it and run again."
        }
    }
    if ([string]::IsNullOrWhiteSpace($env:USERPROFILE)) {
        throw 'USERPROFILE is not set.'
    }
    $userProfile = $env:USERPROFILE.TrimEnd('\', '/')
    if ([string]::IsNullOrWhiteSpace($env:CARGO_HOME)) {
        $cargoHome = Join-Path $userProfile '.cargo'
    }
    else {
        $cargoHome = $env:CARGO_HOME.TrimEnd('\', '/')
    }
    $rustFlags = @(
        "--remap-path-prefix=$repositoryRoot=rawstride",
        "--remap-path-prefix=$cargoHome=cargo",
        '-Clink-arg=/PDBALTPATH:%_PDB%'
    )
    Write-Output 'Building: cargo build --release --locked -p pw-loupe (paths remapped)'
    $env:CARGO_ENCODED_RUSTFLAGS = $rustFlags -join [char]0x1F
    try {
        $null = Invoke-Native 'cargo' @('build', '--release', '--locked', '-p', 'pw-loupe')
    }
    finally {
        Remove-Item -LiteralPath 'Env:CARGO_ENCODED_RUSTFLAGS' -ErrorAction SilentlyContinue
    }
    $exe = Join-Path $repositoryRoot 'target\release\rawstride.exe'
    if (-not (Test-Path -LiteralPath $exe -PathType Leaf)) {
        throw "The build succeeded but $exe does not exist."
    }

    # --- 5. Check the exe: no build-machine paths, the PDB named without its folder. ---
    # Latin-1 maps each byte to one character, so a byte search becomes a string search.
    $latin1 = [System.Text.Encoding]::GetEncoding(28591)
    $exeText = $latin1.GetString([System.IO.File]::ReadAllBytes($exe))
    $folders = @(
        [pscustomobject]@{ Label = 'the repository root'; Path = $repositoryRoot },
        [pscustomobject]@{ Label = 'the cargo home folder'; Path = $cargoHome },
        [pscustomobject]@{ Label = '%USERPROFILE%'; Path = $userProfile }
    )
    $encodings = @(
        [pscustomobject]@{ Label = 'ASCII'; Encoding = $utf8WithoutBom },
        [pscustomobject]@{ Label = 'UTF-16LE'; Encoding = [System.Text.Encoding]::Unicode }
    )
    foreach ($folder in $folders) {
        $forms = @(
            [pscustomobject]@{ Label = "with '\'"; Text = $folder.Path.Replace('/', '\') },
            [pscustomobject]@{ Label = "with '/'"; Text = $folder.Path.Replace('\', '/') }
        )
        foreach ($form in $forms) {
            foreach ($encoding in $encodings) {
                $needle = $latin1.GetString($encoding.Encoding.GetBytes($form.Text))
                if ($exeText.IndexOf($needle, [System.StringComparison]::OrdinalIgnoreCase) -ge 0) {
                    # The label only, never the path: the log must hold no user folder.
                    throw "rawstride.exe holds $($folder.Label), written $($form.Label), as $($encoding.Label)."
                }
            }
        }
    }
    $exeText = $null
    Write-Output 'rawstride.exe holds no repository root, cargo home or user folder path.'

    $dumpbin = Get-Dumpbin
    $headerLines = $null
    $null = Invoke-Native $dumpbin @('/nologo', '/headers', $exe) -Output ([ref]$headerLines)
    $rsdsLines = @($headerLines | ForEach-Object { "$_".Trim() } | Where-Object { $_.Contains('Format: RSDS') })
    if ($rsdsLines.Count -ne 1) {
        throw "dumpbin /headers shows $($rsdsLines.Count) lines with 'Format: RSDS', expected 1."
    }
    $rsdsLine = $rsdsLines[0]
    $lastSeparator = $rsdsLine.LastIndexOf(', ')
    if ($lastSeparator -lt 0 -or $rsdsLine.Substring($lastSeparator + 2) -cne 'rawstride.pdb') {
        throw "The exe's debug directory does not name its PDB as rawstride.pdb alone (the line is not shown, it may hold a path)."
    }
    Write-Output "PDB: $rsdsLine"

    # --- 6. The portable zip. ---
    & (Join-Path $repositoryRoot 'tools\package-dist.ps1') -NoBuild
    if (-not (Test-Path -LiteralPath $zipPath -PathType Leaf)) {
        throw "tools\package-dist.ps1 made no $zipPath."
    }

    # --- 7. SHA256SUMS.txt. ---
    $zipSha256 = (Get-FileHash -LiteralPath $zipPath -Algorithm SHA256).Hash.ToLowerInvariant()
    $sums = "$zipSha256  $zipName`n"
    [System.IO.File]::WriteAllText($sumsPath, $sums, $utf8WithoutBom)
    Write-Output ''
    Write-Output "$sumsPath`:"
    Write-Output $sums.TrimEnd("`n")
}
finally {
    Pop-Location
}
