# Makes RawStride's installer (ADR 0031 RL9, task T305):
#
#     target\dist\RawStride-<version>-windows-x64.msi
#
# from the program folder tools\package-dist.ps1 makes (target\dist\RawStride-<version>-windows-x64\),
# with the WiX Toolset 7.0.1 and the source tools\msi\RawStride.wxs. The MSI installs every file of
# that folder unchanged except README.txt (it describes the zip, RL9-Q9), per user, into
# %LOCALAPPDATA%\Programs\RawStride, with one Start menu shortcut.
#
# Run it from the repository root, after .\tools\package-dist.ps1:
#
#     .\tools\package-msi.ps1                  the MSI with pw-loupe's version
#     .\tools\package-msi.ps1 -Version 0.4.1   the same files with another version, for the
#                                              upgrade test only
#
# It needs WiX 7.0.1 on PATH (dotnet tool install --global wix --version 7.0.1) and installs
# nothing. It validates the MSI (the ICE checks) and compares an administrative image of it with
# the program folder. Everything it writes is under target\dist\; it never installs or uninstalls
# anything. A release MSI is made by the release workflow, not by hand.

param(
    [string]$Version
)

$ErrorActionPreference = 'Stop'

# The release workflow (T306) installs the same WiX version. WiX 7 runs only with its EULA
# accepted; the script accepts it for each run only (ADR 0031 section 3.5), never for good.
$wixVersion = '7.0.1'
$wixEulaId = 'wix7'

# The UpgradeCode of tools\msi\RawStride.wxs (it must never change; the two must be equal, which
# the script checks after the build). The component GUIDs are made from it.
$upgradeCode = '9DA7D7C8-8D15-4CC5-9C80-2D12BB33937B'

$repositoryRoot = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$distRoot = Join-Path $repositoryRoot 'target\dist'
$msiWorkDir = Join-Path $distRoot 'msi'
$wxsPath = Join-Path $repositoryRoot 'tools\msi\RawStride.wxs'
$iconPath = Join-Path $repositoryRoot 'crates\pw-loupe\assets\rawstride.ico'
$utf8WithoutBom = New-Object System.Text.UTF8Encoding($false)

# Runs a native program and returns its standard output as an array of lines. Its standard error
# goes to the console. In Windows PowerShell 5.1 a line on a native program's standard error becomes
# a terminating error under 'Stop' when output is redirected, so 'Continue' is set around the call.
# Stops if the program fails, after printing its standard output (WiX writes its errors there).
# (package-dist.ps1's helper, which does not print the output.)
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
        # WiX writes its errors to standard output; show them before stopping.
        @($output) | ForEach-Object { Write-Output $_ }
        throw "$Program $($Arguments -join ' ') failed with exit code $exitCode."
    }
    return @($output)
}

# The component GUID of a folder, the same on every run and in every version: WiX generates none
# for a component holding files and a registry key path (error WIX0230). A name-based GUID from
# the SHA-256 of the UpgradeCode and the folder's relative path, with the version 5 and variant
# bits set (like a version 5 UUID, but SHA-256 instead of SHA-1). A folder keeps its GUID
# when its files change between versions; that is safe because MajorUpgrade removes the installed
# version before the new one is installed.
function Get-ComponentGuid {
    param([string]$RelativePath)

    $sha = [System.Security.Cryptography.SHA256]::Create()
    try {
        $bytes = $sha.ComputeHash([System.Text.Encoding]::UTF8.GetBytes("$upgradeCode\" + $RelativePath.ToLowerInvariant()))
    }
    finally {
        $sha.Dispose()
    }
    $bytes[6] = ($bytes[6] -band 0x0F) -bor 0x50
    $bytes[8] = ($bytes[8] -band 0x3F) -bor 0x80
    $hex = (($bytes[0..15] | ForEach-Object { $_.ToString('X2') }) -join '')
    return '{0}-{1}-{2}-{3}-{4}' -f $hex.Substring(0, 8), $hex.Substring(8, 4), $hex.Substring(12, 4), $hex.Substring(16, 4), $hex.Substring(20, 12)
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

# Stops unless the version is what Windows Installer accepts as a product version: three plain
# numbers, the first two 0-255 and the third 0-65535 (no pre-release suffix).
function Assert-MsiVersion {
    param(
        [string]$Value,
        [string]$Source
    )

    $match = [regex]::Match($Value, '^(\d{1,5})\.(\d{1,5})\.(\d{1,5})$')
    if (-not $match.Success) {
        throw "The version '$Value' ($Source) is not three plain numbers (for example 0.4.1); Windows Installer accepts no other form, a pre-release suffix such as -rc.1 included."
    }
    $major = [int]$match.Groups[1].Value
    $minor = [int]$match.Groups[2].Value
    $build = [int]$match.Groups[3].Value
    if ($major -gt 255 -or $minor -gt 255 -or $build -gt 65535) {
        throw "The version '$Value' ($Source) is out of Windows Installer's range: the first two numbers must be 0-255 and the third 0-65535."
    }
}

# A valid WiX id (letters, digits, '_' and '.', starting with a letter or '_', at most 72
# characters) made from a prefix and a folder's relative path, the same on every run. A long path
# keeps its start and gets a short hash of the whole path.
function Get-WixId {
    param(
        [string]$Prefix,
        [string]$RelativePath
    )

    if ($RelativePath -eq '') {
        return "$($Prefix)_root"
    }
    $body = [regex]::Replace($RelativePath.Replace('\', '.'), '[^A-Za-z0-9_.]', '_')
    $id = "$($Prefix)_$body"
    if ($id.Length -le 72) {
        return $id
    }
    $sha = [System.Security.Cryptography.SHA256]::Create()
    try {
        $hashBytes = $sha.ComputeHash([System.Text.Encoding]::UTF8.GetBytes($RelativePath))
    }
    finally {
        $sha.Dispose()
    }
    $hash = (($hashBytes[0..7] | ForEach-Object { $_.ToString('x2') }) -join '')
    $keep = 72 - $Prefix.Length - 1 - 1 - $hash.Length
    return "$($Prefix)_$($body.Substring(0, $keep))_$hash"
}

function ConvertTo-XmlText {
    param([string]$Text)

    return [System.Security.SecurityElement]::Escape($Text)
}

# Relative path -> SHA-256 of every file under a folder, keys compared without case.
function Get-FileTable {
    param(
        [string]$Root,
        [string[]]$SkipTopLevel
    )

    $prefix = $Root.TrimEnd('\') + '\'
    $table = New-Object 'System.Collections.Generic.Dictionary[string,string]' ([System.StringComparer]::OrdinalIgnoreCase)
    foreach ($file in Get-ChildItem -LiteralPath $Root -File -Recurse -Force) {
        $relative = $file.FullName.Substring($prefix.Length)
        if ($SkipTopLevel -contains $relative) {
            continue
        }
        $table[$relative] = Get-Sha256 $file.FullName
    }
    return , $table
}

# Reads one value from the Property table of an .msi, opened read-only.
function Get-MsiProperty {
    param(
        [string]$MsiPath,
        [string]$Name
    )

    $installer = New-Object -ComObject WindowsInstaller.Installer
    $database = $null
    $view = $null
    $record = $null
    try {
        $database = $installer.GetType().InvokeMember('OpenDatabase', 'InvokeMethod', $null, $installer, @($MsiPath, 0))
        $query = "SELECT ``Value`` FROM ``Property`` WHERE ``Property`` = '$Name'"
        $view = $database.GetType().InvokeMember('OpenView', 'InvokeMethod', $null, $database, @($query))
        $null = $view.GetType().InvokeMember('Execute', 'InvokeMethod', $null, $view, $null)
        $record = $view.GetType().InvokeMember('Fetch', 'InvokeMethod', $null, $view, $null)
        if ($null -eq $record) {
            return $null
        }
        return $record.GetType().InvokeMember('StringData', 'GetProperty', $null, $record, @(1))
    }
    finally {
        if ($view) {
            $null = $view.GetType().InvokeMember('Close', 'InvokeMethod', $null, $view, $null)
        }
        foreach ($comObject in @($record, $view, $database, $installer)) {
            if ($comObject) {
                $null = [System.Runtime.InteropServices.Marshal]::ReleaseComObject($comObject)
            }
        }
    }
}

$adminDir = $null
$intermediateDir = $null
Push-Location $repositoryRoot
try {
    # --- Version and repository URL: pw-loupe's, from cargo metadata. ---
    $metadataLines = Invoke-Native 'cargo' @('metadata', '--format-version', '1', '--no-deps', '--locked', '--offline')
    $metadata = ($metadataLines -join "`n") | ConvertFrom-Json
    $loupePackages = @($metadata.packages | Where-Object { $_.name -eq 'pw-loupe' })
    if ($loupePackages.Count -ne 1) {
        throw "cargo metadata lists $($loupePackages.Count) packages named pw-loupe, expected 1."
    }
    $loupeVersion = $loupePackages[0].version
    $repositoryUrl = $loupePackages[0].repository
    if ([string]::IsNullOrWhiteSpace($loupeVersion) -or [string]::IsNullOrWhiteSpace($repositoryUrl)) {
        throw "pw-loupe has no version or no repository in cargo metadata."
    }
    if ($Version) {
        Assert-MsiVersion $Version '-Version'
        $msiVersion = $Version
    }
    else {
        Assert-MsiVersion $loupeVersion "pw-loupe's version"
        $msiVersion = $loupeVersion
    }
    if ($msiVersion -ne $loupeVersion) {
        Write-Output "RawStride $loupeVersion's files as version $msiVersion (-Version, for the upgrade test only) ($repositoryUrl)"
    }
    else {
        Write-Output "RawStride $msiVersion ($repositoryUrl)"
    }

    # --- The program folder, made by package-dist.ps1. ---
    $programDir = Join-Path $distRoot "RawStride-$loupeVersion-windows-x64"
    if (-not (Test-Path -LiteralPath (Join-Path $programDir 'rawstride.exe') -PathType Leaf)) {
        throw "$programDir with rawstride.exe does not exist: run .\tools\package-dist.ps1 first."
    }
    foreach ($required in @($wxsPath, $iconPath)) {
        if (-not (Test-Path -LiteralPath $required -PathType Leaf)) {
            throw "$required is missing."
        }
    }

    # --- WiX. ---
    $wix = Get-Command 'wix' -CommandType Application -ErrorAction SilentlyContinue | Select-Object -First 1
    if (-not $wix) {
        throw "wix is not on PATH. Install WiX $wixVersion with: dotnet tool install --global wix --version $wixVersion (this script installs nothing)."
    }
    $wixVersionLines = @(Invoke-Native $wix.Source @('--version'))
    $wixVersionSeen = ''
    if ($wixVersionLines.Count -gt 0) {
        $wixVersionSeen = "$($wixVersionLines[0])".Trim()
    }
    if ($wixVersionSeen -notmatch ('^' + [regex]::Escape($wixVersion) + '(\+|$)')) {
        throw "wix on PATH ($($wix.Source)) is version '$wixVersionSeen', expected $wixVersion. Install it with: dotnet tool install --global wix --version $wixVersion (this script installs nothing)."
    }
    Write-Output "WiX $wixVersionSeen ($($wix.Source))"

    # --- The files fragment: one component per folder. ---
    New-Item -ItemType Directory -Force -Path $msiWorkDir | Out-Null
    $programRoot = (Resolve-Path -LiteralPath $programDir).Path.TrimEnd('\')
    if ($programRoot.Contains('$')) {
        throw "The program folder's path $programRoot holds '$', which WiX's preprocessor would read as a variable."
    }
    $skipped = @('README.txt')
    $ids = New-Object 'System.Collections.Generic.Dictionary[string,string]' ([System.StringComparer]::OrdinalIgnoreCase)
    $componentIds = New-Object System.Collections.Generic.List[string]
    $script:fileCount = 0

    function Register-Id {
        param([string]$Id, [string]$RelativePath)

        if ($ids.ContainsKey($Id)) {
            throw "The folders '$($ids[$Id])' and '$RelativePath' give the same WiX id $Id."
        }
        $ids[$Id] = $RelativePath
    }

    # Writes one folder's component and its subfolders' Directory elements.
    function Add-Folder {
        param(
            [System.Text.StringBuilder]$Builder,
            [string]$FullPath,
            [string]$RelativePath,
            [string]$DirectoryId,
            [string]$Indent
        )

        $componentId = Get-WixId 'c' $RelativePath
        $removeId = Get-WixId 'r' $RelativePath
        Register-Id $componentId $RelativePath
        Register-Id $removeId $RelativePath
        $componentIds.Add($componentId)
        $valueName = $RelativePath
        if ($valueName -eq '') {
            $valueName = '.'
        }

        $null = $Builder.AppendLine("$Indent<Component Id=`"$componentId`" Guid=`"$(Get-ComponentGuid $RelativePath)`">")
        $files = @(Get-ChildItem -LiteralPath $FullPath -File -Force | Sort-Object Name)
        foreach ($file in $files) {
            if ($RelativePath -eq '' -and $skipped -contains $file.Name) {
                continue
            }
            if ($file.Name.Contains('$')) {
                throw "$($file.FullName): a '$' in a file name would be read by WiX's preprocessor as a variable."
            }
            $relativeFile = if ($RelativePath -eq '') { $file.Name } else { "$RelativePath\$($file.Name)" }
            $null = $Builder.AppendLine("$Indent  <File Source=`"`$(var.ProgramDir)\$(ConvertTo-XmlText $relativeFile)`" />")
            $script:fileCount++
        }
        $null = $Builder.AppendLine("$Indent  <RemoveFolder Id=`"$removeId`" Directory=`"$DirectoryId`" On=`"uninstall`" />")
        $null = $Builder.AppendLine("$Indent  <RegistryValue Root=`"HKCU`" Key=`"Software\RawStride\Installer`" Name=`"$(ConvertTo-XmlText $valueName)`" Type=`"integer`" Value=`"1`" KeyPath=`"yes`" />")
        $null = $Builder.AppendLine("$Indent</Component>")

        foreach ($folder in @(Get-ChildItem -LiteralPath $FullPath -Directory -Force | Sort-Object Name)) {
            if ($folder.Name.Contains('$')) {
                throw "$($folder.FullName): a '$' in a folder name would be read by WiX's preprocessor as a variable."
            }
            $childRelative = if ($RelativePath -eq '') { $folder.Name } else { "$RelativePath\$($folder.Name)" }
            $childId = Get-WixId 'd' $childRelative
            Register-Id $childId $childRelative
            $null = $Builder.AppendLine("$Indent<Directory Id=`"$childId`" Name=`"$(ConvertTo-XmlText $folder.Name)`">")
            Add-Folder $Builder $folder.FullName $childRelative $childId "$Indent  "
            $null = $Builder.AppendLine("$Indent</Directory>")
        }
    }

    $builder = New-Object System.Text.StringBuilder
    $null = $builder.AppendLine('<?xml version="1.0" encoding="utf-8"?>')
    $null = $builder.AppendLine('<!-- Written by tools/package-msi.ps1 (ADR 0031 RL9, T305) from the program folder; do not edit. -->')
    $null = $builder.AppendLine('<Include xmlns="http://wixtoolset.org/schemas/v4/wxs">')
    $null = $builder.AppendLine('  <Fragment>')
    $null = $builder.AppendLine('    <DirectoryRef Id="INSTALLFOLDER">')
    Add-Folder $builder $programRoot '' 'INSTALLFOLDER' '      '
    $null = $builder.AppendLine('    </DirectoryRef>')
    $null = $builder.AppendLine('    <ComponentGroup Id="ProgramFiles">')
    foreach ($componentId in $componentIds) {
        $null = $builder.AppendLine("      <ComponentRef Id=`"$componentId`" />")
    }
    $null = $builder.AppendLine('    </ComponentGroup>')
    $null = $builder.AppendLine('  </Fragment>')
    $null = $builder.AppendLine('</Include>')
    $filesWxs = Join-Path $msiWorkDir 'files.wxs'
    [System.IO.File]::WriteAllText($filesWxs, $builder.ToString().Replace("`r`n", "`n").Replace("`n", "`r`n"), $utf8WithoutBom)
    Write-Output "Wrote $filesWxs`: $($script:fileCount) files in $($componentIds.Count) folders"

    # --- Build. ---
    $msiPath = Join-Path $distRoot "RawStride-$msiVersion-windows-x64.msi"
    Remove-IfPresent $msiPath
    $intermediateDir = Join-Path $msiWorkDir 'obj'
    Remove-IfPresent $intermediateDir
    $buildArguments = @(
        'build', '-nologo', '-acceptEula', $wixEulaId,
        '-arch', 'x64',
        '-d', "Version=$msiVersion",
        '-d', "ProgramDir=$programRoot",
        '-d', "IconPath=$iconPath",
        '-d', "RepositoryUrl=$repositoryUrl",
        '-includepath', $msiWorkDir,
        '-intermediatefolder', $intermediateDir,
        '-pdbtype', 'none',
        '-o', $msiPath,
        $wxsPath
    )
    Write-Output "Building: wix $($buildArguments -join ' ')"
    Invoke-Native $wix.Source $buildArguments | ForEach-Object { Write-Output $_ }
    if (-not (Test-Path -LiteralPath $msiPath -PathType Leaf)) {
        throw "wix build succeeded but $msiPath does not exist."
    }

    # --- Validation: Windows Installer's ICE checks; an error stops the script. The warning ICE61
    # from AllowSameVersionUpgrades (RL9-Q10) is expected. One ICE is suppressed: ICE91 warns, once
    # per file, that a file in the user's profile is not copied to each user's profile in a
    # per-machine installation; this package is per user only (Scope="perUser", RL9-Q3) and has no
    # per-machine installation, and its 500-odd lines would hide any other warning. ---
    $validateArguments = @('msi', 'validate', '-nologo', '-acceptEula', $wixEulaId, '-sice', 'ICE91', '-intermediatefolder', $intermediateDir, $msiPath)
    Write-Output "Validating: wix $($validateArguments -join ' ')"
    Invoke-Native $wix.Source $validateArguments | ForEach-Object { Write-Output $_ }

    # --- The contents, checked without installing: an administrative image. ---
    $adminDir = Join-Path $msiWorkDir 'admin-image'
    Remove-IfPresent $adminDir
    New-Item -ItemType Directory -Force -Path $adminDir | Out-Null
    $adminLog = Join-Path $msiWorkDir 'admin-image.log'
    Remove-IfPresent $adminLog
    $msiexecArguments = @('/a', "`"$msiPath`"", '/qn', "TARGETDIR=`"$adminDir`"", '/l*', "`"$adminLog`"")
    Write-Output "Administrative image: msiexec $($msiexecArguments -join ' ')"
    $process = Start-Process -FilePath 'msiexec.exe' -ArgumentList $msiexecArguments -Wait -PassThru
    if ($process.ExitCode -ne 0) {
        throw "msiexec /a failed with exit code $($process.ExitCode); see $adminLog."
    }
    $exeCopies = @(Get-ChildItem -LiteralPath $adminDir -Recurse -File -Force -Filter 'rawstride.exe')
    if ($exeCopies.Count -ne 1) {
        throw "The administrative image holds $($exeCopies.Count) rawstride.exe files, expected exactly 1."
    }
    $imageRoot = $exeCopies[0].DirectoryName
    $expected = Get-FileTable $programRoot $skipped
    $actual = Get-FileTable $imageRoot @()
    $differences = New-Object System.Collections.Generic.List[string]
    foreach ($name in $expected.Keys) {
        if (-not $actual.ContainsKey($name)) {
            $differences.Add("missing in the MSI: $name")
        }
        elseif ($actual[$name] -ne $expected[$name]) {
            $differences.Add("different SHA-256: $name")
        }
    }
    foreach ($name in $actual.Keys) {
        if (-not $expected.ContainsKey($name)) {
            $differences.Add("not in the program folder: $name")
        }
    }
    if ($differences.Count -gt 0) {
        throw "The MSI's files differ from $programRoot (without $($skipped -join ', ')):`r`n  $($differences -join "`r`n  ")"
    }
    Write-Output "Administrative image: the same $($expected.Count) files, each with the same SHA-256, in $($imageRoot.Substring($adminDir.Length + 1))"

    # --- Result. ---
    $productVersion = Get-MsiProperty $msiPath 'ProductVersion'
    if ($productVersion -ne $msiVersion) {
        throw "The MSI's ProductVersion is '$productVersion', expected $msiVersion."
    }
    $msiUpgradeCode = Get-MsiProperty $msiPath 'UpgradeCode'
    if ($msiUpgradeCode -ne "{$upgradeCode}") {
        throw "The MSI's UpgradeCode is '$msiUpgradeCode', expected {$upgradeCode} (RawStride.wxs and this script must agree)."
    }
    $msiItem = Get-Item -LiteralPath $msiPath
    Write-Output ''
    Write-Output "MSI:     $msiPath"
    Write-Output "Size:    $($msiItem.Length) bytes"
    Write-Output "SHA-256: $(Get-Sha256 $msiPath)"
    Write-Output "Version: $productVersion"
    Write-Output "Files:   $($expected.Count)"
}
finally {
    if ($adminDir) {
        Remove-IfPresent $adminDir
    }
    if ($intermediateDir) {
        Remove-IfPresent $intermediateDir
    }
    Pop-Location
}
