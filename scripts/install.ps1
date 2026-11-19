<#
.SYNOPSIS
  Install CHRONOS-AGENT (chronos.exe + chronos-mcp.exe) on Windows x64 from GitHub Releases.
.EXAMPLE
  irm https://raw.githubusercontent.com/ptai-eng/chronos-agent/main/scripts/install.ps1 | iex
.EXAMPLE
  .\install.ps1 -Version v1.8.0 -InstallDir "$env:LOCALAPPDATA\chronos\bin"
#>
[CmdletBinding()]
param(
    [string]$Version = "",
    [string]$InstallDir = (Join-Path $env:LOCALAPPDATA "chronos\bin"),
    [switch]$NoPathUpdate
)

$ErrorActionPreference = "Stop"
$Repo = "ptai-eng/chronos-agent"
$ArchiveName = "chronos-windows-x64.zip"

if ([Environment]::Is64BitOperatingSystem -eq $false) {
    throw "CHRONOS prebuilt Windows binaries require 64-bit x64 Windows."
}

$arch = $env:PROCESSOR_ARCHITECTURE
if ($arch -ne "AMD64" -and $arch -ne "x86_64") {
    Write-Warning "Detected processor architecture '$arch'; falling back to x64 binary ($ArchiveName)."
}

if ([string]::IsNullOrWhiteSpace($Version)) {
    Write-Host "Resolving latest CHRONOS-AGENT release tag..." -ForegroundColor Cyan
    $release = Invoke-RestMethod -Uri "https://api.github.com/repos/$Repo/releases/latest" -Headers @{
        "User-Agent" = "chronos-installer"
        "Accept"     = "application/vnd.github+json"
    }
    $Version = $release.tag_name
}

if (-not $Version.StartsWith("v")) {
    $Version = "v$Version"
}

$downloadUrl = "https://github.com/$Repo/releases/download/$Version/$ArchiveName"
$checksumsUrl = "https://github.com/$Repo/releases/download/$Version/SHA256SUMS.txt"

$tempRoot = Join-Path ([System.IO.Path]::GetTempPath()) ("chronos-install-" + [System.Guid]::NewGuid().ToString("N"))
New-Item -ItemType Directory -Path $tempRoot -Force | Out-Null

try {
    $zipPath = Join-Path $tempRoot $ArchiveName
    Write-Host "Downloading $ArchiveName ($Version) from GitHub Releases..." -ForegroundColor Cyan
    Invoke-WebRequest -Uri $downloadUrl -OutFile $zipPath -UseBasicParsing

    # Verify SHA-256 checksum automatically when SHA256SUMS.txt is published on the release
    $sumsPath = Join-Path $tempRoot "SHA256SUMS.txt"
    try {
        Invoke-WebRequest -Uri $checksumsUrl -OutFile $sumsPath -UseBasicParsing -ErrorAction Stop
        $expectedLine = Get-Content $sumsPath | Where-Object { $_ -match "\s+\*?$([regex]::Escape($ArchiveName))$" } | Select-Object -First 1
        if ($expectedLine) {
            $expectedHash = ($expectedLine -split '\s+')[0].Trim().ToUpperInvariant()
            $actualHash = (Get-FileHash -Path $zipPath -Algorithm SHA256).Hash.ToUpperInvariant()
            if ($expectedHash -ne $actualHash) {
                throw "SHA-256 verification failed for $ArchiveName (expected $expectedHash, got $actualHash)"
            }
            Write-Host "Verified SHA-256 checksum ($actualHash)." -ForegroundColor Green
        }
    }
    catch [System.Net.WebException] {
        # Older releases (e.g. v1.8.0) do not ship SHA256SUMS.txt
    }
    catch {
        if ($_.Exception.Message -match "SHA-256 verification failed") { throw }
    }

    $extractDir = Join-Path $tempRoot "extracted"
    Expand-Archive -Path $zipPath -DestinationPath $extractDir -Force

    New-Item -ItemType Directory -Path $InstallDir -Force | Out-Null
    foreach ($binName in @("chronos.exe", "chronos-mcp.exe")) {
        $src = Join-Path $extractDir $binName
        if (-not (Test-Path $src)) {
            throw "Expected binary '$binName' was missing from archive $ArchiveName"
        }
        Copy-Item -Path $src -Destination (Join-Path $InstallDir $binName) -Force
    }

    if (-not $NoPathUpdate) {
        $userPath = [Environment]::GetEnvironmentVariable("Path", "User")
        $entries = @()
        if (-not [string]::IsNullOrWhiteSpace($userPath)) {
            $entries = $userPath -split ';' | Where-Object { -not [string]::IsNullOrWhiteSpace($_) }
        }
        $normalizedTarget = $InstallDir.TrimEnd('\')
        $alreadyPresent = $entries | Where-Object { $_.TrimEnd('\') -ieq $normalizedTarget }
        if (-not $alreadyPresent) {
            $newUserPath = ($entries + $InstallDir) -join ';'
            [Environment]::SetEnvironmentVariable("Path", $newUserPath, "User")
            if (-not (($env:Path -split ';') | Where-Object { $_.TrimEnd('\') -ieq $normalizedTarget })) {
                $env:Path = "$InstallDir;$env:Path"
            }
            Write-Host "Added '$InstallDir' to User PATH." -ForegroundColor Green
        }
    }

    $chronosBin = Join-Path $InstallDir "chronos.exe"
    $versionOut = & $chronosBin --version
    Write-Host "`nInstalled $versionOut to '$InstallDir'." -ForegroundColor Green
    Write-Host "Next step inside any repo: chronos init --client all --with-hooks" -ForegroundColor Cyan
}
finally {
    Remove-Item -Path $tempRoot -Recurse -Force -ErrorAction SilentlyContinue
}
