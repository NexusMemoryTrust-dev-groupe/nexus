<#
.SYNOPSIS
    Nexus Memory Trust - Windows installer bootstrapper.

.DESCRIPTION
    Downloads the signed NSIS installer from GitHub Releases and launches it.

    The installer itself (built by Tauri) is what actually installs the app, so
    the user keeps the normal Windows experience: choosing the target drive,
    per-user vs per-machine, Start Menu folder, and a proper entry in
    "Apps & features" for clean uninstallation.

    Why this script no longer downloads a bare .exe:
    the previous version fetched a hardcoded asset named `nexus-windows-x64.exe`
    which the release pipeline has never produced - every run failed with HTTP
    404. Assets are now discovered from the Releases API and matched by pattern.

.PARAMETER Silent
    Install without UI, accepting the default location. Skips drive selection.

.PARAMETER PerMachine
    Install for all users (requires elevation). Default is per-user.

.PARAMETER Version
    Install a specific tag (e.g. v1.0.0) instead of the latest release.

.EXAMPLE
    .\install-nexus.ps1
    Downloads the latest installer and opens it so you can pick the drive.

.EXAMPLE
    .\install-nexus.ps1 -Silent -PerMachine
    Unattended install for all users - intended for IT deployment.
#>

[CmdletBinding()]
param(
    [switch]$Silent,
    [switch]$PerMachine,
    [string]$Version
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

# ===============================================================
#  Configuration
# ===============================================================

$GithubRepo = if ($env:NEXUS_REPO) { $env:NEXUS_REPO } else { 'NexusMemoryTrust-dev-groupe/nexus' }
$AppName    = 'Nexus Memory Trust'
$CacheDir   = Join-Path $env:LOCALAPPDATA 'Nexus\installer'
$UserAgent  = 'nexus-installer'

# ===============================================================
#  Output helpers
# ===============================================================

function Write-Banner {
    Write-Host ''
    Write-Host '  +==========================================================+' -ForegroundColor DarkYellow
    Write-Host '  |   NEXUS MEMORY TRUST - Windows Installer                  |' -ForegroundColor Yellow
    Write-Host '  +==========================================================+' -ForegroundColor DarkYellow
    Write-Host ''
}

function Write-Step    { param([string]$m) Write-Host '  -> ' -ForegroundColor Yellow -NoNewline; Write-Host $m }
function Write-Ok      { param([string]$m) Write-Host '     OK  ' -ForegroundColor Green  -NoNewline; Write-Host $m }
function Write-Warn    { param([string]$m) Write-Host '     !   ' -ForegroundColor Yellow -NoNewline; Write-Host $m }
function Write-Failure { param([string]$m) Write-Host '     X   ' -ForegroundColor Red    -NoNewline; Write-Host $m }

# ===============================================================
#  Environment checks
# ===============================================================

<#
  Nexus requires 64-bit Windows. 32-bit (x86) is deliberately unsupported:
  the ONNX runtime powering semantic search publishes no 32-bit binaries, and
  Windows 11 has no 32-bit edition at all.
#>
function Get-TargetArchitecture {
    $arch = $env:PROCESSOR_ARCHITECTURE
    if ($env:PROCESSOR_ARCHITEW6432) { $arch = $env:PROCESSOR_ARCHITEW6432 }

    switch -Wildcard ($arch) {
        'AMD64'   { return 'x64' }
        'ARM64'   { return 'arm64' }
        'x86'     { throw "32-bit Windows is not supported. Nexus requires 64-bit Windows 10 (1809+) or Windows 11." }
        default   { throw "Unrecognised processor architecture '$arch'. Nexus requires 64-bit Windows." }
    }
}

function Assert-WindowsVersion {
    $os = [Environment]::OSVersion.Version
    # Windows 10 1809 = build 17763. Earlier builds lack WebView2 support.
    if ($os.Major -lt 10 -or ($os.Major -eq 10 -and $os.Build -lt 17763)) {
        throw "Windows 10 build 17763 (version 1809) or newer is required. Detected build $($os.Build)."
    }
    $name = if ($os.Build -ge 22000) { 'Windows 11' } else { 'Windows 10' }
    Write-Ok "$name (build $($os.Build))"
}

function Test-IsElevated {
    $id = [Security.Principal.WindowsIdentity]::GetCurrent()
    (New-Object Security.Principal.WindowsPrincipal $id).IsInRole(
        [Security.Principal.WindowsBuiltInRole]::Administrator)
}

# ===============================================================
#  Release resolution
# ===============================================================

function Get-Release {
    param([string]$Tag)

    $uri = if ($Tag) {
        "https://api.github.com/repos/$GithubRepo/releases/tags/$Tag"
    } else {
        "https://api.github.com/repos/$GithubRepo/releases/latest"
    }

    try {
        # TLS 1.2 is not the default on stock Windows 10 PowerShell 5.1.
        [Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12
        return Invoke-RestMethod -Uri $uri -Headers @{ 'User-Agent' = $UserAgent }
    }
    catch {
        $status = $null
        if ($_.Exception.PSObject.Properties['Response'] -and $_.Exception.Response) {
            $status = [int]$_.Exception.Response.StatusCode
        }
        switch ($status) {
            404 { throw "No published release found$(if($Tag){" for tag '$Tag'"}). It may still be a draft." }
            403 { throw "GitHub API rate limit reached. Retry in a few minutes." }
            default { throw "Could not reach GitHub Releases: $($_.Exception.Message)" }
        }
    }
}

<#
  Pick the NSIS installer for this architecture.

  Matched by substring rather than an exact filename, so `Nexus_1.0.0_x64-setup.exe`
  and any future rename keep working. Falls back to the MSI, which some managed
  environments prefer.
#>
function Select-InstallerAsset {
    param($Assets, [string]$Arch)

    $setup = $Assets | Where-Object {
        $_.name -match '(?i)\.exe$' -and $_.name -match '(?i)setup' -and $_.name -match "(?i)$Arch"
    } | Select-Object -First 1
    if ($setup) { return $setup }

    $msi = $Assets | Where-Object {
        $_.name -match '(?i)\.msi$' -and $_.name -match "(?i)$Arch"
    } | Select-Object -First 1
    if ($msi) { return $msi }

    $available = ($Assets | ForEach-Object { $_.name }) -join ', '
    if (-not $available) { $available = '(none)' }
    throw "This release has no $Arch Windows installer.`n     Available assets: $available"
}

# ===============================================================
#  Download + integrity
# ===============================================================

function Save-Asset {
    param($Asset, [string]$Destination)

    if ((Test-Path $Destination) -and ((Get-Item $Destination).Length -eq $Asset.size)) {
        Write-Ok "Already downloaded: $($Asset.name)"
        return
    }

    $mb = [math]::Round($Asset.size / 1MB, 1)
    Write-Step "Downloading $($Asset.name) ($mb MB)..."

    New-Item -ItemType Directory -Path (Split-Path $Destination) -Force | Out-Null

    # Write to .partial first so an interrupted download never leaves a
    # truncated file that looks complete on the next run.
    $partial = "$Destination.partial"
    $progress = $ProgressPreference
    try {
        $ProgressPreference = 'SilentlyContinue'
        Invoke-WebRequest -Uri $Asset.browser_download_url -OutFile $partial `
            -Headers @{ 'User-Agent' = $UserAgent }
    }
    finally { $ProgressPreference = $progress }

    Move-Item -Path $partial -Destination $Destination -Force
    Write-Ok "Saved to $Destination"
}

<#
  Verify the download against the release checksum file when one exists.
  A missing checksum file is a warning, not a failure - but a *mismatch* aborts.
#>
function Test-AssetChecksum {
    param($Assets, $Asset, [string]$Path)

    $sums = $Assets | Where-Object { $_.name -match '(?i)sha256|checksums' } | Select-Object -First 1
    if (-not $sums) {
        Write-Warn 'No checksum file in release - integrity not verified'
        return
    }

    $text   = (Invoke-WebRequest -Uri $sums.browser_download_url -Headers @{ 'User-Agent' = $UserAgent }).Content
    $actual = (Get-FileHash -Path $Path -Algorithm SHA256).Hash.ToLower()

    foreach ($line in ($text -split "`r?`n")) {
        if (-not $line.Trim()) { continue }
        $parts = $line.Trim() -split '[\s\*]+'
        if ($parts.Count -lt 2) { continue }
        $hash = $parts[0].ToLower()
        $file = Split-Path $parts[-1] -Leaf
        if ($Asset.name -ieq $file) {
            if ($hash -ne $actual) {
                throw "Checksum mismatch for $($Asset.name).`n     expected: $hash`n     actual:   $actual`n     Refusing to run a tampered installer."
            }
            Write-Ok 'Checksum verified (SHA-256)'
            return
        }
    }
    Write-Warn "$($Asset.name) not listed in checksum file - integrity not verified"
}

# ===============================================================
#  Install
# ===============================================================

function Invoke-Installer {
    param([string]$Path, [switch]$Unattended, [switch]$AllUsers)

    $isMsi = $Path -match '(?i)\.msi$'
    $args  = @()

    if ($isMsi) {
        $args += @('/i', "`"$Path`"")
        if ($Unattended) { $args += '/quiet' }
        if ($AllUsers)   { $args += 'ALLUSERS=1' }
        Write-Step 'Launching MSI installer...'
        $proc = Start-Process -FilePath 'msiexec.exe' -ArgumentList $args -PassThru -Wait
    }
    else {
        # Tauri's NSIS installer flags: /S silent, /NCRC skip CRC, /D target dir.
        # /CURRENTUSER and /ALLUSERS choose the scope when built with installMode "both".
        if ($Unattended) { $args += '/S' }
        if ($AllUsers)   { $args += '/ALLUSERS' } else { $args += '/CURRENTUSER' }

        if ($Unattended) {
            Write-Step 'Installing silently...'
        } else {
            Write-Step 'Launching installer - choose your drive in the wizard...'
        }
        $proc = Start-Process -FilePath $Path -ArgumentList $args -PassThru -Wait
    }

    if ($proc.ExitCode -ne 0) {
        # 1602 = user cancelled, 3010 = success but reboot required.
        switch ($proc.ExitCode) {
            1602 { throw 'Installation cancelled by user.' }
            3010 { Write-Warn 'Installed - a reboot is required to finish.'; return }
            default { throw "Installer exited with code $($proc.ExitCode)." }
        }
    }
    Write-Ok 'Installer finished'
}

<#
  Read the install location back from the uninstall registry key.
  This is how we honour a custom drive: whatever the user picked in the wizard
  is recorded here, so nothing downstream has to assume C:.
#>
function Get-InstalledLocation {
    $roots = @(
        'HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall',
        'HKLM:\Software\Microsoft\Windows\CurrentVersion\Uninstall'
    )
    foreach ($root in $roots) {
        foreach ($leaf in @('com.nexus.memorytrust', 'Nexus')) {
            $key = Join-Path $root $leaf
            if (Test-Path $key) {
                $loc = (Get-ItemProperty -Path $key -ErrorAction SilentlyContinue).InstallLocation
                if ($loc -and (Test-Path $loc)) { return $loc }
            }
        }
    }
    foreach ($guess in @(
        (Join-Path $env:LOCALAPPDATA 'Programs\Nexus'),
        (Join-Path $env:ProgramFiles 'Nexus')
    )) {
        if ($guess -and (Test-Path (Join-Path $guess 'Nexus.exe'))) { return $guess }
    }
    return $null
}

# ===============================================================
#  Main
# ===============================================================

try {
    Write-Banner

    Write-Step 'Checking system...'
    Assert-WindowsVersion
    $arch = Get-TargetArchitecture
    Write-Ok "Architecture: $arch"

    if ($PerMachine -and -not (Test-IsElevated)) {
        throw 'Installing for all users requires an elevated PowerShell. Re-run as Administrator, or omit -PerMachine for a per-user install.'
    }

    Write-Step 'Resolving release...'
    $release = Get-Release -Tag $Version
    Write-Ok "Version: $($release.tag_name)"

    $assets = @($release.assets)
    $asset  = Select-InstallerAsset -Assets $assets -Arch $arch
    $target = Join-Path $CacheDir $asset.name

    Save-Asset -Asset $asset -Destination $target
    Test-AssetChecksum -Assets $assets -Asset $asset -Path $target

    Invoke-Installer -Path $target -Unattended:$Silent -AllUsers:$PerMachine

    $location = Get-InstalledLocation
    Write-Host ''
    Write-Host '  +==========================================================+' -ForegroundColor DarkGreen
    Write-Host '  |   Installation complete                                   |' -ForegroundColor Green
    Write-Host '  +==========================================================+' -ForegroundColor DarkGreen
    Write-Host ''
    if ($location) { Write-Host "  Installed to: $location" -ForegroundColor Gray }
    Write-Host ''
    Write-Host '  Next steps' -ForegroundColor Cyan
    Write-Host '    1. Launch Nexus from the desktop shortcut or Start Menu' -ForegroundColor White
    Write-Host '    2. The setup wizard checks Node.js, OpenCode CLI and your API key' -ForegroundColor White
    Write-Host '    3. Nexus registers itself with OpenCode so any AI can use your memory' -ForegroundColor White
    Write-Host ''
    Write-Host '  Uninstall: Settings > Apps > Nexus Memory Trust' -ForegroundColor Gray
    Write-Host ''
}
catch {
    Write-Host ''
    Write-Failure $_.Exception.Message
    Write-Host ''
    Write-Host '  Download manually:' -ForegroundColor Yellow
    Write-Host "    https://github.com/$GithubRepo/releases/latest" -ForegroundColor White
    Write-Host ''
    exit 1
}
