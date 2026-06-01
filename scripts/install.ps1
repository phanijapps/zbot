param(
    [string]$Version = "",
    [string]$Repo = "phanijapps/zbot",
    [string]$InstallDir = "$HOME\bin",
    [switch]$NoPathUpdate,
    [switch]$DryRun
)

$ErrorActionPreference = "Stop"

function Invoke-GitHubApi {
    param([string]$Path)
    $headers = @{
        "Accept" = "application/vnd.github+json"
        "X-GitHub-Api-Version" = "2022-11-28"
    }
    Invoke-RestMethod -Headers $headers -Uri "https://api.github.com/repos/$Repo$Path"
}

function Resolve-Version {
    if ($Version) {
        return $Version
    }
    $release = Invoke-GitHubApi "/releases/latest"
    if (-not $release.tag_name) {
        throw "Could not resolve latest release for $Repo"
    }
    return $release.tag_name
}

function Confirm-Checksum {
    param(
        [string]$File,
        [string]$Checksums
    )
    $name = Split-Path -Leaf $File
    $line = Get-Content $Checksums | Where-Object {
        $parts = $_ -split "\s+"
        $parts.Count -ge 2 -and $parts[1] -eq $name
    } | Select-Object -First 1
    if (-not $line) {
        throw "No checksum entry found for $name"
    }
    $expected = ($line -split "\s+")[0].ToLowerInvariant()
    $actual = (Get-FileHash -Algorithm SHA256 $File).Hash.ToLowerInvariant()
    if ($expected -ne $actual) {
        throw "Checksum mismatch for $name. Expected $expected, got $actual"
    }
}

if ($env:PROCESSOR_ARCHITECTURE -notin @("AMD64", "x86_64")) {
    throw "Unsupported Windows architecture: $env:PROCESSOR_ARCHITECTURE"
}

$resolvedVersion = Resolve-Version
$archive = "zbot-$resolvedVersion-windows-x86_64.zip"
$releaseUrl = "https://github.com/$Repo/releases/download/$resolvedVersion"

Write-Host "Repository: $Repo"
Write-Host "Version:    $resolvedVersion"
Write-Host "Archive:    $archive"
Write-Host "Install:    $InstallDir"

if ($DryRun) {
    exit 0
}

$tmp = Join-Path ([System.IO.Path]::GetTempPath()) ("zbot-install-" + [System.Guid]::NewGuid())
New-Item -ItemType Directory -Path $tmp | Out-Null

try {
    $archivePath = Join-Path $tmp $archive
    $checksumsPath = Join-Path $tmp "checksums.sha256"
    Invoke-WebRequest -Uri "$releaseUrl/$archive" -OutFile $archivePath
    Invoke-WebRequest -Uri "$releaseUrl/checksums.sha256" -OutFile $checksumsPath
    Confirm-Checksum -File $archivePath -Checksums $checksumsPath

    $extractDir = Join-Path $tmp "extract"
    Expand-Archive -Path $archivePath -DestinationPath $extractDir
    $root = Join-Path $extractDir "zbot-$resolvedVersion"

    New-Item -ItemType Directory -Force -Path $InstallDir | Out-Null
    Copy-Item -Force (Join-Path $root "zbotd.exe") (Join-Path $InstallDir "zbotd.exe")
    Copy-Item -Force (Join-Path $root "zbot.exe") (Join-Path $InstallDir "zbot.exe")

    $shareDir = Join-Path $env:LOCALAPPDATA "zbot"
    $distDir = Join-Path $shareDir "dist"
    New-Item -ItemType Directory -Force -Path $distDir | Out-Null
    Get-ChildItem -Path $distDir -Force | Remove-Item -Recurse -Force
    Copy-Item -Recurse -Force (Join-Path $root "dist\*") $distDir

    $dataDir = Join-Path ([Environment]::GetFolderPath("MyDocuments")) "zbot"
    New-Item -ItemType Directory -Force -Path $dataDir | Out-Null

    if (-not $NoPathUpdate) {
        $current = [Environment]::GetEnvironmentVariable("Path", "User")
        $parts = @()
        if ($current) {
            $parts = $current -split ";"
        }
        if ($parts -notcontains $InstallDir) {
            $newPath = if ($current) { "$current;$InstallDir" } else { $InstallDir }
            [Environment]::SetEnvironmentVariable("Path", $newPath, "User")
            Write-Host "Added $InstallDir to the user PATH. Open a new terminal to use it."
        }
    }

    Write-Host ""
    Write-Host "zbot $resolvedVersion installed."
    Write-Host "Binaries: $InstallDir\zbotd.exe and $InstallDir\zbot.exe"
    Write-Host "Dashboard assets: $distDir"
}
finally {
    Remove-Item -Recurse -Force $tmp -ErrorAction SilentlyContinue
}
