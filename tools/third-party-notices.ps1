# Makes THIRD-PARTY-NOTICES.txt in the repository root (ADR 0031 RL3, task T227): the notices of
# everything in a RawStride release that is not RawStride's own. The C libraries come from the vcpkg
# packages' copyright files, the fonts from egui's epaint_default_fonts, the Rust packages from
# cargo-about with about.toml and tools/third-party-notices.hbs.
#
# Needs cargo-about 0.9.3 (cargo install cargo-about --locked --version 0.9.3 --features cli) and
# the vcpkg packages (docs/building.md). Run it from the repository root, again whenever Cargo.lock,
# vcpkg.json or ExifTool's version changes, and commit the result:
#
#     .\tools\third-party-notices.ps1
#
# A run on an unchanged tree gives the same bytes (no date in the file).

$ErrorActionPreference = 'Stop'

$cargoAboutVersion = '0.9.3'
# Must match EXIFTOOL_VERSION in .github/workflows/ci.yml.
$exifToolVersion = '13.64'
$triplet = 'x64-windows-static-md'

# The packages that ship no licence file of their own: cargo-about writes its standard licence text
# for them, and this script adds a copyright line built from their Cargo.toml `authors`.
$packagesWithoutLicenceFile = @(
    'accesskit',
    'accesskit_consumer',
    'accesskit_windows',
    'accesskit_winit',
    'clipboard-win',
    'ecolor',
    'eframe',
    'egui',
    'egui-winit',
    'egui_glow',
    'emath',
    'epaint',
    'epaint_default_fonts',
    'profiling'
)

# The libraries vcpkg builds into rawstride.exe, in the order they are listed.
$libraries = @(
    [pscustomobject]@{ Name = 'LibRaw'; Port = 'libraw'; Url = 'https://www.libraw.org' },
    [pscustomobject]@{ Name = 'LittleCMS'; Port = 'lcms'; Url = 'https://www.littlecms.com' },
    [pscustomobject]@{ Name = 'JasPer'; Port = 'jasper'; Url = 'https://jasper-software.github.io/jasper' },
    [pscustomobject]@{ Name = 'libjpeg-turbo'; Port = 'libjpeg-turbo'; Url = 'https://libjpeg-turbo.org' },
    [pscustomobject]@{ Name = 'libspng'; Port = 'libspng'; Url = 'https://libspng.org' },
    [pscustomobject]@{ Name = 'zlib'; Port = 'zlib'; Url = 'https://zlib.net' }
)

$fontFiles = @(
    'Hack-Regular.txt',
    'OFL.txt',
    'UFL.txt',
    'emoji-icon-font-mit-license.txt',
    'egui-icons.txt'
)

$repositoryRoot = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$outputPath = Join-Path $repositoryRoot 'THIRD-PARTY-NOTICES.txt'
$utf8WithoutBom = New-Object System.Text.UTF8Encoding($false)
$doubleRule = '=' * 80
$singleRule = '-' * 80

function Read-TextFile {
    param([string]$Path)

    if (-not (Test-Path -LiteralPath $Path -PathType Leaf)) {
        throw "File not found: $Path"
    }
    $text = [System.IO.File]::ReadAllText($Path, $script:utf8WithoutBom)
    return ConvertTo-LfText $text
}

function ConvertTo-LfText {
    param([string]$Text)

    $text = $Text.Replace("`r`n", "`n").Replace("`r", "`n")
    if ($text.Length -gt 0 -and $text[0] -eq [char]0xFEFF) {
        $text = $text.Substring(1)
    }
    # Each part ends in exactly one line break; the parts are joined with blank lines.
    return $text.TrimEnd("`n")
}

# Runs cargo with the given arguments and returns its standard output as one string (UTF-8). Its
# standard error goes to the console. Stops if cargo fails.
function Invoke-Cargo {
    param([string[]]$Arguments)

    $previousPreference = $ErrorActionPreference
    $previousEncoding = [Console]::OutputEncoding
    $ErrorActionPreference = 'Continue'
    try {
        [Console]::OutputEncoding = $script:utf8WithoutBom
        $output = & cargo @Arguments
        $exitCode = $LASTEXITCODE
    }
    finally {
        [Console]::OutputEncoding = $previousEncoding
        $ErrorActionPreference = $previousPreference
    }
    if ($exitCode -ne 0) {
        throw "cargo $($Arguments -join ' ') failed with exit code $exitCode."
    }
    return ($output -join "`n")
}

function Get-Section {
    param(
        [string]$Heading,
        [string]$Body
    )

    return "$doubleRule`n$Heading`n$doubleRule`n`n$Body"
}

Push-Location $repositoryRoot
try {
    # cargo-about, the exact version this file was made with.
    $installHint = "Install it with: cargo install cargo-about --locked --version $cargoAboutVersion --features cli"
    $versionText = ''
    try {
        $versionText = Invoke-Cargo @('about', '--version')
    }
    catch {
        throw "cargo-about is not installed or does not run. $installHint"
    }
    if ($versionText.Trim() -ne "cargo-about $cargoAboutVersion") {
        throw "cargo-about $cargoAboutVersion is needed, found '$($versionText.Trim())'. $installHint"
    }

    # The vcpkg packages, found as tools/check.ps1 finds them.
    $vcpkgInstalledDir = $env:VCPKG_INSTALLED_DIR
    if ([string]::IsNullOrWhiteSpace($vcpkgInstalledDir)) {
        $ownVcpkgDir = Join-Path $repositoryRoot 'vcpkg_installed'
        $fallbackVcpkgDir = Join-Path (Split-Path $repositoryRoot -Parent) 'PhotoWorkflow\vcpkg_installed'
        if (Test-Path -LiteralPath $ownVcpkgDir -PathType Container) {
            $vcpkgInstalledDir = (Resolve-Path -LiteralPath $ownVcpkgDir).Path
        }
        elseif (Test-Path -LiteralPath $fallbackVcpkgDir -PathType Container) {
            $vcpkgInstalledDir = (Resolve-Path -LiteralPath $fallbackVcpkgDir).Path
        }
        else {
            throw "VCPKG_INSTALLED_DIR is not set and neither $ownVcpkgDir nor $fallbackVcpkgDir exists. Run 'vcpkg install' in the repository root (docs/building.md)."
        }
    }
    if (-not (Test-Path -LiteralPath $vcpkgInstalledDir -PathType Container)) {
        throw "VCPKG_INSTALLED_DIR does not point to an existing folder: $vcpkgInstalledDir"
    }
    $vcpkgInfoDir = Join-Path $vcpkgInstalledDir 'vcpkg\info'
    $vcpkgShareDir = Join-Path $vcpkgInstalledDir "$triplet\share"

    foreach ($library in $libraries) {
        $copyrightPath = Join-Path $vcpkgShareDir "$($library.Port)\copyright"
        if (-not (Test-Path -LiteralPath $copyrightPath -PathType Leaf)) {
            throw "The vcpkg package $($library.Port) has no copyright file at $copyrightPath. Run 'vcpkg install' in the repository root (docs/building.md)."
        }
        # Only the triplet's own list; the folder also has lists for the host tools.
        $listPattern = '^' + [regex]::Escape($library.Port) + '_(.+)_' + [regex]::Escape($triplet) + '\.list$'
        $lists = @(Get-ChildItem -LiteralPath $vcpkgInfoDir -File | Where-Object { $_.Name -match $listPattern })
        if ($lists.Count -ne 1) {
            throw "Expected one $($library.Port)_<version>_$triplet.list in $vcpkgInfoDir, found $($lists.Count)."
        }
        $null = $lists[0].Name -match $listPattern
        $library | Add-Member -NotePropertyName Version -NotePropertyValue $Matches[1]
        $library | Add-Member -NotePropertyName Text -NotePropertyValue (Read-TextFile $copyrightPath)
    }

    # pw-loupe's version, epaint_default_fonts' folder and the authors, from cargo metadata.
    $metadata = Invoke-Cargo @('metadata', '--format-version', '1', '--locked', '--offline', '--filter-platform', 'x86_64-pc-windows-msvc') | ConvertFrom-Json
    $loupePackages = @($metadata.packages | Where-Object { $_.name -eq 'pw-loupe' })
    if ($loupePackages.Count -ne 1) {
        throw "cargo metadata lists $($loupePackages.Count) packages named pw-loupe, expected 1."
    }
    $rawStrideVersion = $loupePackages[0].version
    $fontPackages = @($metadata.packages | Where-Object { $_.name -eq 'epaint_default_fonts' })
    if ($fontPackages.Count -ne 1) {
        throw "cargo metadata lists $($fontPackages.Count) packages named epaint_default_fonts, expected 1."
    }
    $fontDir = Join-Path (Split-Path $fontPackages[0].manifest_path -Parent) 'fonts'

    # The Rust packages: cargo-about fills the template; only pw-loupe's packages, not the tools'.
    $rustTempPath = [System.IO.Path]::GetTempFileName()
    try {
        $null = Invoke-Cargo @('about', 'generate', '--locked', '--offline', '--fail',
            '-m', 'crates\pw-loupe\Cargo.toml', '-c', 'about.toml',
            '-o', $rustTempPath, 'tools\third-party-notices.hbs')
        $rustText = Read-TextFile $rustTempPath
    }
    finally {
        Remove-Item -LiteralPath $rustTempPath -Force -ErrorAction SilentlyContinue
    }

    # Each "#copyright <name> <version>" line from the template becomes the package's copyright line
    # if it ships no licence file, and is removed otherwise.
    $rustLines = New-Object System.Collections.Generic.List[string]
    $rustPackages = New-Object 'System.Collections.Generic.SortedSet[string]'
    $copyrightAdded = New-Object 'System.Collections.Generic.HashSet[string]'
    $licenceTextCount = 0
    foreach ($line in $rustText.Split("`n")) {
        if ($line -eq $singleRule) {
            $licenceTextCount++
        }
        if (-not $line.StartsWith('#copyright')) {
            $rustLines.Add($line)
            continue
        }
        if ($line -notmatch '^#copyright (\S+) (\S+)$') {
            throw "Unexpected line in cargo-about's output: $line"
        }
        $name = $Matches[1]
        $version = $Matches[2]
        $null = $rustPackages.Add("$name $version")
        if ($packagesWithoutLicenceFile -notcontains $name) {
            continue
        }
        $package = @($metadata.packages | Where-Object { $_.name -eq $name -and $_.version -eq $version })
        if ($package.Count -ne 1) {
            throw "cargo metadata has no single package $name $version."
        }
        # The names only; e-mail addresses in `authors` are left out.
        $authors = @($package[0].authors | ForEach-Object { ($_ -replace '\s*<[^>]*>', '').Trim() } | Where-Object { $_ -ne '' })
        if ($authors.Count -eq 0) {
            throw "$name $version ships no licence file and has no authors in its Cargo.toml."
        }
        $rustLines.Add("    Copyright (c) $($authors -join ', ')")
        $null = $copyrightAdded.Add($name)
    }
    foreach ($name in $packagesWithoutLicenceFile) {
        if (-not $copyrightAdded.Contains($name)) {
            throw "$name is listed as shipping no licence file but is not in cargo-about's output; update the list in this script."
        }
    }
    $rustPart = ConvertTo-LfText ($rustLines -join "`n")

    # libspng's copyright file is its LICENSE section followed by the whole of spng.c; only the
    # section and the libpng-2.0 notice in spng.c are taken.
    $spng = $libraries | Where-Object { $_.Port -eq 'libspng' }
    if (-not $spng.Text.StartsWith("LICENSE:`n")) {
        throw "libspng's copyright file does not start with 'LICENSE:'."
    }
    $sourceStart = $spng.Text.IndexOf("`nspng.c:`n")
    if ($sourceStart -lt 0) {
        throw "libspng's copyright file has no 'spng.c:' line where the C source starts."
    }
    $spngLicence = ConvertTo-LfText ($spng.Text.Substring("LICENSE:`n".Length, $sourceStart - "LICENSE:`n".Length).Trim("`n"))
    $libpngFirstLine = '* PNG Reference Library License version 2'
    $libpngStart = $spng.Text.IndexOf("`n$libpngFirstLine`n")
    if ($libpngStart -lt 0) {
        throw "libspng's copyright file has no libpng-2.0 notice starting with '$libpngFirstLine'."
    }
    $libpngEnd = $spng.Text.IndexOf("`n*/", $libpngStart)
    if ($libpngEnd -lt 0) {
        throw "libspng's libpng-2.0 notice has no end of comment."
    }
    $libpngLines = $spng.Text.Substring($libpngStart + 1, $libpngEnd - $libpngStart - 1).Split("`n") |
        ForEach-Object { $_ -replace '^\* ?', '' }
    $spng.Text = "$spngLicence`n`nThe libpng-2.0 notice in libspng's source (spng.c):`n`n" + (ConvertTo-LfText ($libpngLines -join "`n"))

    $libraryEntries = foreach ($library in $libraries) {
        $entry = "$($library.Name) $($library.Version)`n$($library.Url)"
        if ($library.Port -eq 'libraw') {
            $entry += "`n`nRawStride uses LibRaw under the GNU Lesser General Public License 2.1. Its source is at https://github.com/LibRaw/LibRaw (tag $($library.Version)), built with the vcpkg port at the baseline in RawStride's vcpkg.json."
        }
        if ($library.Port -eq 'libjpeg-turbo') {
            $entry += "`n`nThis software is based in part on the work of the Independent JPEG Group."
        }
        "$entry`n`n$($library.Text)"
    }

    $fontEntries = foreach ($fontFile in $fontFiles) {
        "$fontFile`n`n" + (Read-TextFile (Join-Path $fontDir $fontFile))
    }

    $header = @"
Third-party notices for RawStride $rawStrideVersion

RawStride itself is licensed under the GNU General Public License, version 3 or later
(GPL-3.0-or-later); see the file LICENSE. This file holds the notices of the software and data in
RawStride that are not RawStride's own. It is generated by tools/third-party-notices.ps1.
"@

    # The copyright sign as a code point: Windows PowerShell reads a script without a BOM as ANSI.
    $copyrightSign = [char]0x00A9

    $msvc = @"
VCRUNTIME140.dll, VCRUNTIME140_1.dll, MSVCP140.dll and VCOMP140.DLL are $copyrightSign Microsoft Corporation
and are redistributed under the Microsoft Visual Studio license terms for distributable code.
"@

    $exifTool = @"
ExifTool $exifToolVersion by Phil Harvey, https://exiftool.org, is shipped unmodified as exiftool.exe
and the exiftool_files folder. Its README says: "This is free software; you can redistribute it
and/or modify it under the same terms as Perl itself (either the Perl Artistic License or GPL)."

The Perl in exiftool_files comes with its licence in exiftool_files\LICENSE, and the licences of
Strawberry Perl's parts are in exiftool_files\Licenses_Strawberry_Perl.zip.
"@

    $iptc = @"
RawStride's IPTC property table and its Scene, Genre and Digital Source Type vocabularies are
derived from the IPTC Photo Metadata Standard 2025.1 TechReference and the IPTC NewsCodes,
Copyright $copyrightSign IPTC, the International Press Telecommunications Council, https://iptc.org, used
under the Creative Commons Attribution 4.0 licence
(https://creativecommons.org/licenses/by/4.0/); only property names, identifiers and codes were
taken and turned into program tables.

The codes of the PLUS controlled vocabularies are those of the PLUS License Data Format by the
PLUS Coalition, https://www.useplus.com.
"@

    $rustIntro = "The Rust packages built into rawstride.exe, grouped by licence text: the packages that use a text, then the text."

    $sections = @(
        (ConvertTo-LfText $header),
        (Get-Section 'Libraries built into rawstride.exe' ($libraryEntries -join "`n`n$singleRule`n")),
        (Get-Section 'Fonts' ($fontEntries -join "`n`n$singleRule`n")),
        (Get-Section 'Microsoft Visual C++ runtime' (ConvertTo-LfText $msvc)),
        (Get-Section 'ExifTool' (ConvertTo-LfText $exifTool)),
        (Get-Section 'IPTC and PLUS' (ConvertTo-LfText $iptc)),
        (Get-Section 'Rust packages' "$rustIntro`n`n$rustPart")
    )
    # Here-strings in this file may have CRLF line breaks; the output is LF only.
    $notices = (ConvertTo-LfText ($sections -join "`n`n")) + "`n"
    [System.IO.File]::WriteAllText($outputPath, $notices, $utf8WithoutBom)

    Write-Output "Wrote $outputPath"
    Write-Output "Rust packages: $($rustPackages.Count); licence texts: $licenceTextCount"
}
finally {
    Pop-Location
}
