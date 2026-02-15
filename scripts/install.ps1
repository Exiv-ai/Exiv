# ============================================================
# Exiv Quick Installer for Windows
# Downloads a pre-built binary from GitHub Releases and installs it.
#
# Usage:
#   irm https://raw.githubusercontent.com/Exiv-ai/Exiv/master/scripts/install.ps1 | iex
#
# Environment variables:
#   EXIV_PREFIX   Install directory (default: C:\ProgramData\Exiv)
#   EXIV_VERSION  Version to install (default: latest)
#   EXIV_SERVICE  Set to "true" to register as Windows service
# ============================================================

$ErrorActionPreference = "Stop"

$Repo = "Exiv-ai/Exiv"
$InstallDir = if ($env:EXIV_PREFIX) { $env:EXIV_PREFIX } else { "C:\ProgramData\Exiv" }
$Version = if ($env:EXIV_VERSION) { $env:EXIV_VERSION } else { "latest" }
$SetupService = if ($env:EXIV_SERVICE -eq "true") { $true } else { $false }
$Platform = "windows-x64"

Write-Host "Exiv Installer" -ForegroundColor Cyan
Write-Host "  Platform: $Platform"

# --- Resolve version ---
if ($Version -eq "latest") {
    Write-Host "  Resolving latest version..."
    try {
        $Release = Invoke-RestMethod -Uri "https://api.github.com/repos/$Repo/releases/latest" -Headers @{ "User-Agent" = "Exiv-Installer" }
        $Version = $Release.tag_name
    } catch {
        Write-Host "Error: Failed to fetch latest release. Set EXIV_VERSION explicitly." -ForegroundColor Red
        exit 1
    }
}
$VersionNum = $Version -replace "^v", ""
# M-21: Validate version format (semver)
if ($VersionNum -notmatch '^\d+\.\d+\.\d+(-[a-zA-Z0-9.]+)?$') {
    Write-Host "Error: Invalid version format '$VersionNum'. Expected semver (e.g., 1.2.3 or 1.2.3-beta.1)" -ForegroundColor Red
    exit 1
}
Write-Host "  Version:  v$VersionNum"

# --- Download ---
$Archive = "exiv-$VersionNum-$Platform.zip"
$Url = "https://github.com/$Repo/releases/download/v$VersionNum/$Archive"
$ChecksumUrl = "$Url.sha256"

$TmpDir = Join-Path ([System.IO.Path]::GetTempPath()) "exiv-install-$(Get-Random)"
New-Item -ItemType Directory -Path $TmpDir -Force | Out-Null

try {
    Write-Host ""
    Write-Host "Downloading $Archive..." -ForegroundColor Cyan
    try {
        Invoke-WebRequest -Uri $Url -OutFile (Join-Path $TmpDir $Archive) -UseBasicParsing
    } catch {
        Write-Host "Error: Download failed. Check that version v$VersionNum exists at https://github.com/$Repo/releases" -ForegroundColor Red
        exit 1
    }

    # --- Verify checksum ---
    try {
        Invoke-WebRequest -Uri $ChecksumUrl -OutFile (Join-Path $TmpDir "$Archive.sha256") -UseBasicParsing
        $ExpectedLine = (Get-Content (Join-Path $TmpDir "$Archive.sha256") -Raw).Trim()
        $ExpectedHash = ($ExpectedLine -split "\s+")[0].ToLower()
        $ActualHash = (Get-FileHash (Join-Path $TmpDir $Archive) -Algorithm SHA256).Hash.ToLower()

        if ($ExpectedHash -ne $ActualHash) {
            Write-Host "Error: Checksum verification failed." -ForegroundColor Red
            Write-Host "  Expected: $ExpectedHash" -ForegroundColor Red
            Write-Host "  Actual:   $ActualHash" -ForegroundColor Red
            exit 1
        }
        Write-Host "Checksum verified." -ForegroundColor Green
    } catch {
        Write-Host "  (checksum file not available, skipping verification)" -ForegroundColor Yellow
    }

    # --- Extract ---
    Write-Host "Extracting..."
    $ExtractDir = Join-Path $TmpDir "extracted"
    Expand-Archive -Path (Join-Path $TmpDir $Archive) -DestinationPath $ExtractDir -Force

    $Binary = Join-Path $ExtractDir "exiv_system.exe"
    if (-not (Test-Path $Binary)) {
        # Archive may contain a subdirectory
        $Binary = Get-ChildItem -Path $ExtractDir -Recurse -Filter "exiv_system.exe" | Select-Object -First 1 -ExpandProperty FullName
        if (-not $Binary) {
            Write-Host "Error: Binary not found in archive." -ForegroundColor Red
            exit 1
        }
    }

    # --- Install via the binary's self-install command ---
    Write-Host ""
    Write-Host "Installing to $InstallDir..." -ForegroundColor Cyan

    $InstallArgs = @("install", "--prefix", $InstallDir)
    if ($SetupService) {
        $InstallArgs += "--service"
    }

    & $Binary @InstallArgs
    if ($LASTEXITCODE -ne 0) {
        Write-Host "Error: Installation failed." -ForegroundColor Red
        exit 1
    }

    Write-Host ""
    Write-Host "Exiv v$VersionNum installed successfully." -ForegroundColor Green
    Write-Host ""
    Write-Host "  Binary:    $InstallDir\exiv_system.exe" -ForegroundColor Cyan
    Write-Host "  Dashboard: http://localhost:8081" -ForegroundColor Cyan
    Write-Host "  Manage:    exiv_system.exe service start|stop|status" -ForegroundColor Cyan
    Write-Host "  Uninstall: exiv_system.exe uninstall" -ForegroundColor Cyan
    Write-Host ""

} finally {
    # Cleanup
    if (Test-Path $TmpDir) {
        Remove-Item -Path $TmpDir -Recurse -Force -ErrorAction SilentlyContinue
    }
}
