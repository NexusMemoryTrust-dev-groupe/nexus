#Requires -RunAsAdministrator

<#
.SYNOPSIS
    Nexus Memory Trust — Windows Installer

.DESCRIPTION
    Downloads and installs Nexus Memory Trust desktop application.
    Creates desktop shortcut and Start Menu entry.

.EXAMPLE
    .\install-nexus.ps1
    # Run as Administrator
#>

$ErrorActionPreference = "Stop"

# ═══════════════════════════════════════════════════════════════
#  Configuration
# ═══════════════════════════════════════════════════════════════

$GITHUB_REPO = "NexusMemoryTrust-dev-groupe/nexus"
$APP_NAME = "Nexus Memory Trust"
$INSTALL_DIR = "$env:LOCALAPPDATA\Nexus"
$BINARY_NAME = "nexus.exe"

# ═══════════════════════════════════════════════════════════════
#  Functions
# ═══════════════════════════════════════════════════════════════

function Write-Header {
    Write-Host ""
    Write-Host "================================================================" -ForegroundColor Cyan
    Write-Host "           Nexus Memory Trust - Windows Installer              " -ForegroundColor Cyan
    Write-Host "================================================================" -ForegroundColor Cyan
    Write-Host ""
}

function Write-Step {
    param([string]$Message)
    Write-Host "> " -ForegroundColor Yellow -NoNewline
    Write-Host $Message
}

function Write-Success {
    param([string]$Message)
    Write-Host "  [OK] " -ForegroundColor Green -NoNewline
    Write-Host $Message
}

function Write-Error {
    param([string]$Message)
    Write-Host "  [FAIL] " -ForegroundColor Red -NoNewline
    Write-Host $Message
}

function Get-LatestVersion {
    Write-Step "Checking latest version..."
    
    try {
        $release = Invoke-RestMethod -Uri "https://api.github.com/repos/$GITHUB_REPO/releases/latest" -Headers @{ 'User-Agent' = 'nexus-installer' }
        $version = $release.tag_name
        Write-Success "Latest version: $version"
        return $version
    }
    catch {
        throw "Failed to check latest version: $_"
    }
}

function Download-Binary {
    param([string]$Version)
    
    Write-Step "Downloading Nexus..."
    
    $assetName = "nexus-windows-x64.exe"
    $downloadUrl = "https://github.com/$GITHUB_REPO/releases/download/$Version/$assetName"
    
    # Create install directory
    if (-not (Test-Path $INSTALL_DIR)) {
        New-Item -ItemType Directory -Path $INSTALL_DIR -Force | Out-Null
    }
    
    $binaryPath = Join-Path $INSTALL_DIR $BINARY_NAME
    
    try {
        # Download with progress
        $ProgressPreference = 'SilentlyContinue'
        Invoke-WebRequest -Uri $downloadUrl -OutFile $binaryPath -Headers @{ 'User-Agent' = 'nexus-installer' }
        $ProgressPreference = 'Continue'
        
        Write-Success "Downloaded to: $binaryPath"
        return $binaryPath
    }
    catch {
        throw "Failed to download Nexus: $_"
    }
}

function Add-ToPath {
    param([string]$Directory)
    
    Write-Step "Adding to PATH..."
    
    $currentPath = [Environment]::GetEnvironmentVariable("Path", "User")
    
    if ($currentPath -notlike "*$Directory*") {
        [Environment]::SetEnvironmentVariable("Path", "$currentPath;$Directory", "User")
        $env:Path = "$env:Path;$Directory"
        Write-Success "Added to user PATH"
    }
    else {
        Write-Success "Already in PATH"
    }
}

function Create-Shortcuts {
    param([string]$BinaryPath)
    
    Write-Step "Creating shortcuts..."
    
    $shell = New-Object -ComObject WScript.Shell
    
    # Desktop shortcut
    $desktopPath = [Environment]::GetFolderPath("Desktop")
    $shortcutPath = Join-Path $desktopPath "$APP_NAME.lnk"
    
    $shortcut = $shell.CreateShortcut($shortcutPath)
    $shortcut.TargetPath = $BinaryPath
    $shortcut.WorkingDirectory = $INSTALL_DIR
    $shortcut.Description = $APP_NAME
    $shortcut.IconLocation = "$BinaryPath,0"
    $shortcut.Save()
    
    Write-Success "Desktop shortcut created (with icon)"
    
    # Start Menu shortcut
    $startMenuPath = [Environment]::GetFolderPath("Programs")
    $startMenuShortcut = Join-Path $startMenuPath "$APP_NAME.lnk"
    
    $shortcut = $shell.CreateShortcut($startMenuShortcut)
    $shortcut.TargetPath = $BinaryPath
    $shortcut.WorkingDirectory = $INSTALL_DIR
    $shortcut.Description = $APP_NAME
    $shortcut.IconLocation = "$BinaryPath,0"
    $shortcut.Save()
    
    Write-Success "Start Menu shortcut created (with icon)"
}

function Create-Uninstaller {
    param([string]$BinaryPath)
    
    Write-Step "Creating uninstaller..."
    
    $uninstallScript = @"
# Nexus Memory Trust Uninstaller
Write-Host "Uninstalling Nexus Memory Trust..." -ForegroundColor Yellow

# Remove shortcuts
`$desktopPath = [Environment]::GetFolderPath("Desktop")
`$startMenuPath = [Environment]::GetFolderPath("Programs")

Remove-Item -Path (Join-Path `$desktopPath "$APP_NAME.lnk") -Force -ErrorAction SilentlyContinue
Remove-Item -Path (Join-Path `$startMenuPath "$APP_NAME.lnk") -Force -ErrorAction SilentlyContinue

# Remove installation directory
Remove-Item -Path "$INSTALL_DIR" -Recurse -Force -ErrorAction SilentlyContinue

# Remove from PATH
`$currentPath = [Environment]::GetEnvironmentVariable("Path", "User")
`$newPath = (`$currentPath -split ";" | Where-Object { `$_.Trim() -ne "$INSTALL_DIR" }) -join ";"
[Environment]::SetEnvironmentVariable("Path", `$newPath, "User")

Write-Host "Nexus Memory Trust has been uninstalled." -ForegroundColor Green
"@
    
    $uninstallPath = Join-Path $INSTALL_DIR "uninstall.ps1"
    Set-Content -Path $uninstallPath -Value $uninstallScript
    
    Write-Success "Uninstaller created: $uninstallPath"
}

# ═══════════════════════════════════════════════════════════════
#  Main
# ═══════════════════════════════════════════════════════════════

try {
    Write-Header
    
    # Check if already installed
    $existingBinary = Join-Path $INSTALL_DIR $BINARY_NAME
    if (Test-Path $existingBinary) {
        Write-Host "WARNING: Nexus is already installed at: $existingBinary" -ForegroundColor Yellow
        $response = Read-Host "Do you want to reinstall? (y/N)"
        if ($response -ne 'y' -and $response -ne 'Y') {
            Write-Host "Installation cancelled." -ForegroundColor Yellow
            exit 0
        }
    }
    
    # Get latest version
    $version = Get-LatestVersion
    
    # Download binary
    $binaryPath = Download-Binary -Version $version
    
    # Add to PATH
    Add-ToPath -Directory $INSTALL_DIR
    
    # Create shortcuts
    Create-Shortcuts -BinaryPath $binaryPath
    
    # Create uninstaller
    Create-Uninstaller -BinaryPath $binaryPath
    
    # Run first-launch setup
    Write-Host ""
    Write-Host "================================================================" -ForegroundColor Green
    Write-Host "                    Installation Complete!                      " -ForegroundColor Green
    Write-Host "================================================================" -ForegroundColor Green
    Write-Host ""
    Write-Host "Launch Nexus:" -ForegroundColor Cyan
    Write-Host "   - Double-click the desktop shortcut" -ForegroundColor White
    Write-Host "   - Or run: nexus" -ForegroundColor White
    Write-Host ""
    Write-Host "First launch:" -ForegroundColor Cyan
    Write-Host "   - The app will configure OpenCode CLI for you" -ForegroundColor White
    Write-Host "   - Choose your preferred AI model" -ForegroundColor White
    Write-Host ""
    Write-Host "Uninstall:" -ForegroundColor Cyan
    Write-Host "   - Run: $INSTALL_DIR\uninstall.ps1" -ForegroundColor White
    Write-Host ""
}
catch {
    Write-Error "Installation failed: $_"
    Write-Host ""
    Write-Host "Try downloading manually from:" -ForegroundColor Yellow
    Write-Host "   https://github.com/$GITHUB_REPO/releases" -ForegroundColor White
    exit 1
}
