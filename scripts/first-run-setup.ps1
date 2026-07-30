<#
.SYNOPSIS
    Nexus Memory Trust — First-Run Setup Wizard
.DESCRIPTION
    Runs automatically on first launch or manually via setup command.
    Checks all dependencies, installs what's missing, configures API key and model.
    Can be embedded in Tauri app or run standalone.
.NOTES
    Usage: .\first-run-setup.ps1 [-Force] [-Headless]
#>

param(
    [switch]$Force,
    [switch]$Headless
)

$ErrorActionPreference = "Continue"
$ProgressPreference = "SilentlyContinue"

$OPENCODE_PACKAGE = "opencode-ai"
$CONFIG_DIR = Join-Path $env:USERPROFILE ".nexus"
$CONFIG_FILE = Join-Path $CONFIG_DIR "setup.json"
$DEFAULT_MODEL = "opencode/deepseek-v4-flash-free"

# ═══════════════════════════════════════════════════════════════
#  Helpers
# ═══════════════════════════════════════════════════════════════

function Test-CommandExists {
    param([string]$Command)
    $null -ne (Get-Command $Command -ErrorAction SilentlyContinue)
}

function Refresh-Path {
    $env:Path = [System.Environment]::GetEnvironmentVariable("Path", "Machine") + ";" + [System.Environment]::GetEnvironmentVariable("Path", "User")
}

function Get-SetupStatus {
    if (Test-Path $CONFIG_FILE) {
        return Get-Content $CONFIG_FILE -Raw | ConvertFrom-Json
    }
    return $null
}

function Save-SetupStatus {
    param([hashtable]$Status)
    if (-not (Test-Path $CONFIG_DIR)) {
        New-Item -ItemType Directory -Path $CONFIG_DIR -Force | Out-Null
    }
    $Status | ConvertTo-Json | Set-Content $CONFIG_FILE
}

function Write-Status {
    param([string]$Label, [string]$Value, [string]$Color = "White")
    Write-Host "  $Label" -NoNewline -ForegroundColor Gray
    Write-Host " $Value" -ForegroundColor $Color
}

# ═══════════════════════════════════════════════════════════════
#  Check if already set up
# ═══════════════════════════════════════════════════════════════

$status = Get-SetupStatus
if ($status -and -not $Force -and $status.completed -eq $true) {
    if (-not $Headless) {
        Write-Host ""
        Write-Host "  Nexus Memory Trust is already configured." -ForegroundColor Green
        Write-Host "  Run with -Force to reconfigure." -ForegroundColor Gray
        Write-Host ""
    }
    exit 0
}

Write-Host ""
Write-Host "  ╔══════════════════════════════════════════════╗" -ForegroundColor Cyan
Write-Host "  ║   NEXUS MEMORY TRUST — FIRST-RUN SETUP      ║" -ForegroundColor Cyan
Write-Host "  ╚══════════════════════════════════════════════╝" -ForegroundColor Cyan
Write-Host ""

$setupResult = @{
    timestamp = (Get-Date).ToString("yyyy-MM-dd HH:mm:ss")
    node_ok = $false
    npm_ok = $false
    opencode_ok = $false
    api_configured = $false
    model_selected = $false
    completed = $false
}

# ═══════════════════════════════════════════════════════════════
#  1. Check & Install Node.js
# ═══════════════════════════════════════════════════════════════

Write-Host "  [1/5] Node.js" -ForegroundColor Yellow

Refresh-Path
if (Test-CommandExists "node") {
    $nodeVer = & node --version 2>&1
    Write-Status "→" "Found: $nodeVer" "Green"
    $setupResult.node_ok = $true
} else {
    Write-Status "→" "Not found — installing..." "Yellow"
    try {
        $url = "https://nodejs.org/dist/v20.18.1/node-v20.18.1-x64.msi"
        $msi = "$env:TEMP\node-setup.msi"
        [Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12
        Invoke-WebRequest -Uri $url -OutFile $msi -UseBasicParsing
        Start-Process msiexec.exe -ArgumentList "/i `"$msi`" /qn /norestart" -Wait -NoNewWindow -Verb RunAs
        Remove-Item $msi -ErrorAction SilentlyContinue
        Refresh-Path
        if (Test-CommandExists "node") {
            Write-Status "→" "Installed: $(node --version)" "Green"
            $setupResult.node_ok = $true
        }
    } catch {
        Write-Status "→" "Failed to install" "Red"
    }
}

# ═══════════════════════════════════════════════════════════════
#  2. Check npm
# ═══════════════════════════════════════════════════════════════

Write-Host "  [2/5] npm" -ForegroundColor Yellow

if (Test-CommandExists "npm") {
    $npmVer = & npm --version 2>&1
    Write-Status "→" "Found: v$npmVer" "Green"
    $setupResult.npm_ok = $true
} else {
    Write-Status "→" "Not found (requires Node.js)" "Red"
}

# ═══════════════════════════════════════════════════════════════
#  3. Check & Install OpenCode CLI
# ═══════════════════════════════════════════════════════════════

Write-Host "  [3/5] OpenCode AI CLI" -ForegroundColor Yellow

Refresh-Path
if (Test-CommandExists "opencode") {
    Write-Status "→" "Found: $(opencode --version 2>&1)" "Green"
    $setupResult.opencode_ok = $true
} else {
    Write-Status "→" "Not found — installing globally..." "Yellow"
    if ($setupResult.npm_ok) {
        try {
            & npm install -g $OPENCODE_PACKAGE 2>&1 | Out-Null
            Refresh-Path
            if (Test-CommandExists "opencode") {
                Write-Status "→" "Installed: $(opencode --version 2>&1)" "Green"
                $setupResult.opencode_ok = $true
            } else {
                Write-Status "→" "Install succeeded but not in PATH" "Yellow"
            }
        } catch {
            Write-Status "→" "Failed — run: npm install -g $OPENCODE_PACKAGE" "Red"
        }
    } else {
        Write-Status "→" "Cannot install (npm unavailable)" "Red"
    }
}

# ═══════════════════════════════════════════════════════════════
#  4. Configure API Key
# ═══════════════════════════════════════════════════════════════

Write-Host "  [4/5] AI API Key" -ForegroundColor Yellow

if ($Headless) {
    Write-Status "→" "Skipped (headless mode)" "Gray"
} else {
    $existingKey = $null
    if ($setupResult.opencode_ok) {
        try {
            $existingKey = & opencode config get api.key 2>&1
            if ($LASTEXITCODE -ne 0) { $existingKey = $null }
        } catch { }
    }

    if ($existingKey -and $existingKey -ne "" -and $existingKey -ne "null") {
        $masked = $existingKey.Substring(0, [Math]::Min(8, $existingKey.Length)) + "..." + $existingKey.Substring([Math]::Max(0, $existingKey.Length - 4))
        Write-Status "→" "Already configured: $masked" "Green"
        $setupResult.api_configured = $true
    } else {
        Write-Host ""
        Write-Host "  To use AI features, you need an API key." -ForegroundColor Gray
        Write-Host "  Supported providers:" -ForegroundColor Gray
        Write-Host "    • OpenAI     — https://platform.openai.com/api-keys" -ForegroundColor Gray
        Write-Host "    • Anthropic  — https://console.anthropic.com/" -ForegroundColor Gray
        Write-Host "    • Google     — https://aistudio.google.com/apikey" -ForegroundColor Gray
        Write-Host "    • OpenRouter — https://openrouter.ai/keys" -ForegroundColor Gray
        Write-Host ""
        $apiKey = Read-Host "  Paste your API key (or press Enter to skip)"

        if ($apiKey -and $apiKey.Trim() -ne "") {
            if ($setupResult.opencode_ok) {
                try {
                    & opencode config set api.key "$($apiKey.Trim())" 2>&1 | Out-Null
                    Write-Status "→" "API key saved" "Green"
                    $setupResult.api_configured = $true
                } catch {
                    Write-Status "→" "Failed to save key" "Red"
                }
            } else {
                Write-Status "→" "OpenCode not available — key not saved" "Yellow"
            }
        } else {
            Write-Status "→" "Skipped — you can configure later" "Gray"
        }
    }
}

# ═══════════════════════════════════════════════════════════════
#  5. Select Default Model
# ═══════════════════════════════════════════════════════════════

Write-Host "  [5/5] Default AI Model" -ForegroundColor Yellow

if ($Headless) {
    Write-Status "→" "Skipped (headless mode)" "Gray"
} else {
    $existingModel = $null
    if ($setupResult.opencode_ok) {
        try {
            $existingModel = & opencode config get ai.model 2>&1
            if ($LASTEXITCODE -ne 0) { $existingModel = $null }
        } catch { }
    }

    if ($existingModel -and $existingModel -ne "" -and $existingModel -ne "null") {
        Write-Status "→" "Already configured: $existingModel" "Green"
        $setupResult.model_selected = $true
    } else {
        Write-Host ""
        Write-Host "  Available models:" -ForegroundColor Gray
        Write-Host "    1. opencode/deepseek-v4-flash-free  (FREE — recommended)" -ForegroundColor White
        Write-Host "    2. openai/gpt-4o                     (paid)" -ForegroundColor White
        Write-Host "    3. anthropic/claude-sonnet-4         (paid)" -ForegroundColor White
        Write-Host "    4. google/gemini-2.5-flash           (free tier)" -ForegroundColor White
        Write-Host ""
        $choice = Read-Host "  Select model [1-4] (default: 1)"

        $selectedModel = switch ($choice) {
            "2" { "openai/gpt-4o" }
            "3" { "anthropic/claude-sonnet-4" }
            "4" { "google/gemini-2.5-flash" }
            default { $DEFAULT_MODEL }
        }

        if ($setupResult.opencode_ok) {
            try {
                & opencode config set ai.model "$selectedModel" 2>&1 | Out-Null
                Write-Status "→" "Model set: $selectedModel" "Green"
                $setupResult.model_selected = $true
            } catch {
                Write-Status "→" "Failed to set model" "Red"
            }
        } else {
            Write-Status "→" "OpenCode not available — model not saved" "Yellow"
        }
    }
}

# ═══════════════════════════════════════════════════════════════
#  Save & Summary
# ═══════════════════════════════════════════════════════════════

$setupResult.completed = $setupResult.node_ok -and $setupResult.npm_ok
Save-SetupStatus $setupResult

Write-Host ""
Write-Host "  ╔══════════════════════════════════════════════╗" -ForegroundColor Cyan
Write-Host "  ║   SETUP COMPLETE                             ║" -ForegroundColor Cyan
Write-Host "  ╚══════════════════════════════════════════════╝" -ForegroundColor Cyan
Write-Host ""

if ($setupResult.completed) {
    Write-Host "  All systems ready. Launch Nexus to start using AI memory." -ForegroundColor Green
} else {
    Write-Host "  Partial setup complete. Some features may be limited." -ForegroundColor Yellow
    if (-not $setupResult.node_ok) { Write-Host "  → Install Node.js: https://nodejs.org/" -ForegroundColor Gray }
    if (-not $setupResult.opencode_ok) { Write-Host "  → Install OpenCode: npm install -g $OPENCODE_PACKAGE" -ForegroundColor Gray }
}

Write-Host ""
