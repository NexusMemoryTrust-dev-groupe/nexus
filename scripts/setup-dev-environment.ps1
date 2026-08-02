#Requires -RunAsAdministrator
<#
.SYNOPSIS
    Nexus Memory Trust — Master Installer
.DESCRIPTION
    Installs all dependencies for Nexus Memory Trust:
    - Node.js 20 LTS (if not installed)
    - npm (comes with Node.js)
    - OpenCode CLI (npm i -g opencode-ai)
    - Validates all prerequisites
.NOTES
    Run as Administrator: Right-click → Run as Administrator
    Or from PowerShell: .\install-nexus.ps1
#>

$ErrorActionPreference = "Stop"
$ProgressPreference = "SilentlyContinue"

# ═══════════════════════════════════════════════════════════════
#  Configuration
# ═══════════════════════════════════════════════════════════════

$NODE_VERSION = "20"
$NODE_INSTALLER_URL = "https://nodejs.org/dist/v20.18.1/node-v20.18.1-x64.msi"
$NODE_INSTALLER_PATH = "$env:TEMP\node-install.msi"
$OPENCODE_PACKAGE = "opencode-ai"
$NEXUS_DIR = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
$MIN_MEMORY_MB = 4096
$MIN_DISK_GB = 2

# ═══════════════════════════════════════════════════════════════
#  Helper Functions
# ═══════════════════════════════════════════════════════════════

function Write-Header {
    param([string]$Text)
    Write-Host ""
    Write-Host ("=" * 60) -ForegroundColor DarkCyan
    Write-Host "  $Text" -ForegroundColor Cyan
    Write-Host ("=" * 60) -ForegroundColor DarkCyan
    Write-Host ""
}

function Write-Step {
    param([string]$Text)
    Write-Host "  → $Text" -ForegroundColor Yellow
}

function Write-OK {
    param([string]$Text)
    Write-Host "  ✓ $Text" -ForegroundColor Green
}

function Write-Fail {
    param([string]$Text)
    Write-Host "  ✗ $Text" -ForegroundColor Red
}

function Write-Warn {
    param([string]$Text)
    Write-Host "  ⚠ $Text" -ForegroundColor DarkYellow
}

function Test-CommandExists {
    param([string]$Command)
    $null -ne (Get-Command $Command -ErrorAction SilentlyContinue)
}

function Refresh-Path {
    $env:Path = [System.Environment]::GetEnvironmentVariable("Path", "Machine") + ";" + [System.Environment]::GetEnvironmentVariable("Path", "User")
}

# ═══════════════════════════════════════════════════════════════
#  System Checks
# ═══════════════════════════════════════════════════════════════

Write-Header "NEXUS MEMORY TRUST — INSTALLER"

Write-Step "Checking system requirements..."
Write-Host ""

# OS Check
$osInfo = Get-CimInstance Win32_OperatingSystem
$osVersion = [System.Environment]::OSVersion.Version
if ($osVersion.Major -lt 10) {
    Write-Fail "Windows 10 or later required. Found: $($osInfo.Caption)"
    exit 1
}
Write-OK "OS: $($osInfo.Caption) ($($osVersion.Major).$($osVersion.Minor))"

# Architecture Check
$arch = $env:PROCESSOR_ARCHITECTURE
if ($arch -ne "AMD64") {
    Write-Fail "64-bit system required. Found: $arch"
    exit 1
}
Write-OK "Architecture: 64-bit ($arch)"

# Memory Check
$totalMemoryMB = [math]::Round($osInfo.TotalVisibleMemorySize / 1024)
if ($totalMemoryMB -lt $MIN_MEMORY_MB) {
    Write-Warn "Low memory: ${totalMemoryMB}MB (recommended: ${MIN_MEMORY_MB}MB+)"
} else {
    Write-OK "Memory: ${totalMemoryMB}MB"
}

# Disk Check
$systemDrive = $env:SystemDrive
$freeSpaceGB = [math]::Round((Get-PSDrive ($systemDrive.TrimEnd(':')).Free / 1GB), 1)
if ($freeSpaceGB -lt $MIN_DISK_GB) {
    Write-Fail "Low disk space: ${freeSpaceGB}GB (need: ${MIN_DISK_GB}GB+)"
    exit 1
}
Write-OK "Disk space: ${freeSpaceGB}GB free on $systemDrive"

Write-Host ""

# ═══════════════════════════════════════════════════════════════
#  Step 1: Node.js
# ═══════════════════════════════════════════════════════════════

Write-Header "STEP 1/4 — Node.js"

$nodeInstalled = $false
if (Test-CommandExists "node") {
    $currentNode = & node --version 2>&1
    $currentMajor = ($currentNode -replace 'v', '') -split '\.' | Select-Object -First 1
    if ([int]$currentMajor -ge [int]$NODE_VERSION) {
        Write-OK "Node.js $currentNode already installed"
        $nodeInstalled = $true
    } else {
        Write-Warn "Node.js $currentNode is too old (need v${NODE_VERSION}+)"
    }
}

if (-not $nodeInstalled) {
    Write-Step "Downloading Node.js v${NODE_VERSION} LTS..."
    try {
        [Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12
        Invoke-WebRequest -Uri $NODE_INSTALLER_URL -OutFile $NODE_INSTALLER_PATH -UseBasicParsing
        Write-OK "Downloaded Node.js installer"
    } catch {
        Write-Fail "Failed to download Node.js: $_"
        Write-Host ""
        Write-Host "  Manual install: https://nodejs.org/en/download/" -ForegroundColor Gray
        exit 1
    }

    Write-Step "Installing Node.js (this may take a minute)..."
    try {
        Start-Process msiexec.exe -ArgumentList "/i `"$NODE_INSTALLER_PATH`" /qn /norestart" -Wait -NoNewWindow -Verb RunAs
        Refresh-Path
        if (Test-CommandExists "node") {
            Write-OK "Node.js installed: $(node --version)"
            $nodeInstalled = $true
        } else {
            Write-Fail "Node.js installation succeeded but 'node' not found in PATH"
            Write-Warn "Try restarting your terminal or running: refreshenv"
        }
    } catch {
        Write-Fail "Failed to install Node.js: $_"
        Write-Host ""
        Write-Host "  Manual install: https://nodejs.org/en/download/" -ForegroundColor Gray
        exit 1
    }

    # Cleanup
    Remove-Item -Path $NODE_INSTALLER_PATH -ErrorAction SilentlyContinue
}

# ═══════════════════════════════════════════════════════════════
#  Step 2: npm
# ═══════════════════════════════════════════════════════════════

Write-Header "STEP 2/4 — npm"

if (Test-CommandExists "npm") {
    $npmVersion = & npm --version 2>&1
    Write-OK "npm $npmVersion installed"
} else {
    Write-Fail "npm not found (should come with Node.js)"
    Write-Host "  Try restarting your terminal" -ForegroundColor Gray
    exit 1
}

# ═══════════════════════════════════════════════════════════════
#  Step 3: OpenCode CLI
# ═══════════════════════════════════════════════════════════════

Write-Header "STEP 3/4 — OpenCode AI CLI"

$opencodeInstalled = $false
if (Test-CommandExists "opencode") {
    $ocVersion = & opencode --version 2>&1
    Write-OK "OpenCode $ocVersion already installed"
    $opencodeInstalled = $true
}

if (-not $opencodeInstalled) {
    Write-Step "Installing OpenCode AI CLI globally..."
    try {
        & npm install -g $OPENCODE_PACKAGE 2>&1 | Out-Null
        Refresh-Path
        if (Test-CommandExists "opencode") {
            Write-OK "OpenCode installed: $(opencode --version 2>&1)"
            $opencodeInstalled = $true
        } else {
            Write-Fail "OpenCode installation succeeded but 'opencode' not found in PATH"
            Write-Warn "Try: npm install -g $OPENCODE_PACKAGE"
        }
    } catch {
        Write-Fail "Failed to install OpenCode: $_"
        Write-Warn "Manual install: npm install -g $OPENCODE_PACKAGE"
    }
}

# ═══════════════════════════════════════════════════════════════
#  Step 4: Nexus App Verification
# ═══════════════════════════════════════════════════════════════

Write-Header "STEP 4/4 — Nexus Memory Trust App"

$nexusExe = Join-Path $NEXUS_DIR "src-tauri\target\release\nexus.exe"
$nexusReleaseDir = Join-Path $NEXUS_DIR "src-tauri\target\release\bundle"

# Check if app is built
if (Test-Path $nexusExe) {
    Write-OK "Nexus executable found"
} else {
    Write-Warn "Nexus not built yet — app will work after first build"
    Write-Host "  Build with: cargo tauri build" -ForegroundColor Gray
}

# Check for MSI/NSIS installer
$msiFiles = Get-ChildItem -Path $nexusReleaseDir -Filter "*.msi" -Recurse -ErrorAction SilentlyContinue
$nsisFiles = Get-ChildItem -Path $nexusReleaseDir -Filter "*.exe" -Recurse -ErrorAction SilentlyContinue | Where-Object { $_.Name -like "*Setup*" -or $_.Name -like "*Install*" }
if ($msiFiles -or $nsisFiles) {
    Write-OK "Installer found in bundle directory"
} else {
    Write-Warn "No installer built yet"
}

# ═══════════════════════════════════════════════════════════════
#  Summary
# ═══════════════════════════════════════════════════════════════

Write-Header "INSTALLATION SUMMARY"

$allGood = $nodeInstalled -and $opencodeInstalled

if ($allGood) {
    Write-OK "Node.js: $(node --version)"
    Write-OK "npm: $(npm --version)"
    Write-OK "OpenCode: installed"
    Write-Host ""
    Write-Host "  All dependencies installed successfully!" -ForegroundColor Green
    Write-Host ""
    Write-Host "  Next steps:" -ForegroundColor Cyan
    Write-Host "    1. Build the app:  cd $NEXUS_DIR && cargo tauri build" -ForegroundColor White
    Write-Host "    2. Or run in dev:  cargo tauri dev" -ForegroundColor White
    Write-Host "    3. OpenCode will prompt for API key on first use" -ForegroundColor White
} else {
    Write-Fail "Some dependencies failed to install"
    Write-Host ""
    if (-not $nodeInstalled) { Write-Warn "  → Install Node.js manually: https://nodejs.org/" }
    if (-not $opencodeInstalled) { Write-Warn "  → Install OpenCode: npm install -g $OPENCODE_PACKAGE" }
}

Write-Host ""
