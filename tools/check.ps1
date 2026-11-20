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

    # Personal data (AGENTS.md §2): tracked files and untracked files that are not ignored must hold
    # no path into a user's folder and no real e-mail address, so that they do not come back after
    # the repository is public. Only the shape is searched for, never a particular name: a drive,
    # `Users` and a name that starts with a letter, digit, `.`, `_` or `-` (a placeholder such as
    # `%USERPROFILE%` or `C:\Users\<name>` passes), the same in Git Bash form, and an e-mail address
    # at a domain not in the list below.
    Write-ReportLine ''
    Write-ReportLine '=== personal data ==='
    # Test and placeholder domains; an address is allowed when it ends in `@` + one of these exactly.
    $allowedEmailDomains = @(
        'example.com',
        'example.org',
        'example.net',
        'b.se', # test data in pw-metadata
        'users.noreply.github.com'
    )
    $emailPattern = '[a-z0-9._%+-]+@[a-z0-9.-]+\.[a-z]{2,}'
    $personalDataPatterns = @(
        [pscustomobject]@{ Pattern = '[a-z]:[\\/]+users[\\/]+[a-z0-9._-]'; IsEmail = $false },
        [pscustomobject]@{ Pattern = '/[a-z]/users/[a-z0-9._-]'; IsEmail = $false },
        [pscustomobject]@{ Pattern = $emailPattern; IsEmail = $true }
    )
    $emailRegex = New-Object System.Text.RegularExpressions.Regex($emailPattern, [System.Text.RegularExpressions.RegexOptions]::IgnoreCase)
    $personalDataExitCode = 0
    foreach ($personalDataPattern in $personalDataPatterns) {
        # -z separates path, line number and text with NUL, so each hit splits cleanly.
        $grepArguments = @('grep', '--untracked', '-n', '-I', '-i', '-z', '-E', $personalDataPattern.Pattern)
        if ($personalDataPattern.IsEmail) {
            # THIRD-PARTY-NOTICES.txt holds other people's copyright notices verbatim, with their
            # e-mail addresses (owner, 2026-11-20, T227); it is skipped for e-mail addresses only.
            # The path patterns above still search it.
            $grepArguments += @('--', '.', ':(exclude)THIRD-PARTY-NOTICES.txt')
        }
        Write-ReportLine "> git $($grepArguments -join ' ')"
        $grepOutput = & git @grepArguments 2>&1
        $grepExitCode = $LASTEXITCODE
        if ($grepExitCode -gt 1) {
            foreach ($line in $grepOutput) {
                Write-ReportLine ([string]$line)
            }
            Write-ReportLine "git grep failed with exit code $grepExitCode."
            $personalDataExitCode = $grepExitCode
            continue
        }
        if ($grepExitCode -eq 1) {
            continue
        }
        foreach ($line in $grepOutput) {
            $parts = ([string]$line).Split([char]0, 3)
            if ($parts.Count -lt 3) {
                Write-ReportLine ([string]$line)
                $personalDataExitCode = 1
                continue
            }
            $text = $parts[2]
            if ($personalDataPattern.IsEmail) {
                $hasOtherAddress = $false
                foreach ($match in $emailRegex.Matches($text)) {
                    $domain = $match.Value.Substring($match.Value.LastIndexOf('@') + 1)
                    if (-not ($allowedEmailDomains -contains $domain.ToLowerInvariant())) {
                        $hasOtherAddress = $true
                    }
                }
                if (-not $hasOtherAddress) {
                    continue
                }
            }
            if ($text.Length -gt 200) {
                $text = $text.Substring(0, 200) + '...'
            }
            Write-ReportLine "$($parts[0]):$($parts[1]):$text"
            $personalDataExitCode = 1
        }
    }
    Write-ReportLine "Exit code: $personalDataExitCode"
    $personalDataStatus = if ($personalDataExitCode -eq 0) { 'OK' } else { 'FAILED' }
    $script:results += [pscustomobject]@{
        Name = 'personal data'
        Status = $personalDataStatus
    }
    if ($personalDataExitCode -ne 0) {
        $script:hasFailures = $true
    }

    # Checks Cargo.lock against the RustSec advisory database (fetched over the network). A known
    # vulnerability fails the step; warnings such as unmaintained crates do not. Skipped, without
    # failing the check, when cargo-audit is not installed.
    & cargo audit --version *> $null
    if ($LASTEXITCODE -eq 0) {
        Invoke-CheckStep -Name 'cargo audit' -Arguments @('audit')
    }
    else {
        Write-ReportLine ''
        Write-ReportLine '=== cargo audit ==='
        Write-ReportLine 'cargo-audit is not installed (cargo install cargo-audit --locked); step skipped.'
        $script:results += [pscustomobject]@{
            Name = 'cargo audit'
            Status = 'SKIPPED'
        }
    }

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
