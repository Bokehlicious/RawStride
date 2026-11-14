param(
    [switch]$Ignored
)

$ErrorActionPreference = 'Continue'
$repositoryRoot = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$outputPath = Join-Path $repositoryRoot 'test-output.txt'
$utf8WithoutBom = New-Object System.Text.UTF8Encoding($false)
$originalLocation = Get-Location
$originalVcpkgInstalledDir = $env:VCPKG_INSTALLED_DIR
[System.IO.File]::WriteAllText($outputPath, '', $utf8WithoutBom)
$script:results = @()
$script:hasFailures = $false

function Write-ReportLine {
    param([string]$Message)

    Write-Output $Message
    [System.IO.File]::AppendAllText($script:outputPath, $Message + [Environment]::NewLine, $script:utf8WithoutBom)
}

function Invoke-CheckStep {
    param(
        [string]$Name,
        [string[]]$Arguments
    )

    Write-ReportLine ''
    Write-ReportLine "=== $Name ==="
    Write-ReportLine "> cargo $($Arguments -join ' ')"

    try {
        $commandOutput = & cargo @Arguments 2>&1
        $exitCode = $LASTEXITCODE
        foreach ($line in $commandOutput) {
            Write-ReportLine ([string]$line)
        }
    }
    catch {
        $exitCode = 1
        Write-ReportLine $_.Exception.Message
    }

    $status = if ($exitCode -eq 0) { 'OK' } else { 'FAILED' }
    Write-ReportLine "Exit code: $exitCode"
    $script:results += [pscustomobject]@{
        Name = $Name
        Status = $status
    }
    if ($exitCode -ne 0) {
        $script:hasFailures = $true
    }
}

try {
    Set-Location $repositoryRoot

    if ([string]::IsNullOrWhiteSpace($env:VCPKG_INSTALLED_DIR)) {
        # The repository's own folder (made by `vcpkg install` in the repository root) first, then
        # the main checkout's next to this worktree (docs/building.md).
        $ownVcpkgDir = Join-Path $repositoryRoot 'vcpkg_installed'
        $fallbackVcpkgDir = Join-Path (Split-Path $repositoryRoot -Parent) 'PhotoWorkflow\vcpkg_installed'
        if (Test-Path -LiteralPath $ownVcpkgDir -PathType Container) {
            $env:VCPKG_INSTALLED_DIR = (Resolve-Path -LiteralPath $ownVcpkgDir).Path
        }
        elseif (Test-Path -LiteralPath $fallbackVcpkgDir -PathType Container) {
            $env:VCPKG_INSTALLED_DIR = (Resolve-Path -LiteralPath $fallbackVcpkgDir).Path
        }
        else {
            throw "VCPKG_INSTALLED_DIR is not set and neither $ownVcpkgDir nor $fallbackVcpkgDir exists. Run 'vcpkg install' in the repository root (docs/building.md)."
        }
    }

    if (-not (Test-Path -LiteralPath $env:VCPKG_INSTALLED_DIR -PathType Container)) {
        throw "VCPKG_INSTALLED_DIR does not point to an existing folder: $env:VCPKG_INSTALLED_DIR"
    }

    Write-ReportLine "Using VCPKG_INSTALLED_DIR: $env:VCPKG_INSTALLED_DIR"

    Invoke-CheckStep -Name 'cargo fmt --all --check' -Arguments @('fmt', '--all', '--check')

    # --no-fail-fast: run every test binary even after a failure, so the report covers all crates.
    $testArguments = @('test', '--workspace', '--no-fail-fast')
    if ($Ignored) {
        $testArguments += @('--', '--ignored')
    }
    Invoke-CheckStep -Name 'cargo test --workspace' -Arguments $testArguments
    Invoke-CheckStep -Name 'cargo clippy --workspace --all-targets' -Arguments @('clippy', '--workspace', '--all-targets')

    Write-ReportLine ''
    Write-ReportLine 'Summary:'
    foreach ($result in $script:results) {
        Write-ReportLine "$($result.Name): $($result.Status)"
    }
}
catch {
    Write-ReportLine "ERROR: $($_.Exception.Message)"
    exit 1
}
finally {
    Set-Location $originalLocation
    $env:VCPKG_INSTALLED_DIR = $originalVcpkgInstalledDir
}

if ($script:hasFailures) {
    exit 1
}
exit 0
