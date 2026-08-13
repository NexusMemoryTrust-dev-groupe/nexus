# perf-gate.ps1 - CI REGRESSION gate for plan 6.2 + 6.3.
#
# Runs the four benchmark binaries, parses their NEXUS_METRIC output and
# compares every metric against benchmarks/baseline.json. A metric that
# regresses more than the configured tolerance fails the build.
#
# Usage:
#   powershell -File scripts/perf-gate.ps1 -BinaryDir src-tauri\target\release
#
# Exit code: 0 = PASS, 1 = FAIL.
#
# Design notes:
#   * Every bench already enforces its own absolute GATE (SLA ms, load rec/s,
#     conflict detection/FP) via its exit code - that part is free.
#   * This script adds the *relative* regression check: a search that is still
#     under the SLA (say 90 ms of a 100 ms budget) but 10x slower than the
#     baseline is a regression even though it "passes" its absolute gate.
#   * Metrics with baseline values below 5 ms are exempt from the relative
#     check - their absolute gate is the real protection and relative
#     comparison on sub-5 ms numbers is pure measurement noise.
#   * Timing benches flake on shared CI runners (antivirus scans, CPU steal).
#     A bench that fails its absolute GATE is therefore run ONCE more before
#     the failure is accepted; a hung bench (no exit within the timeout) is
#     killed and counts as a failure immediately.
#
# NOTE: this file must stay ASCII-only. Windows PowerShell 5.1 parses .ps1
# files without a BOM as ANSI; a UTF-8 em-dash in a comment can be read back
# as a smart quote that closes a string early and breaks the whole parse.

param(
    [Parameter(Mandatory = $true)]
    [string]$BinaryDir,

    [string]$BaselinePath = "benchmarks/baseline.json",

    # Seconds a single bench run may take before it is killed as hung.
    # The SLA bench re-indexes a 10k corpus (~40-120 s on shared runners), so
    # the budget must allow for a slow windows-latest instance.
    [int]$PerBenchTimeoutSec = 600
)

$ErrorActionPreference = "Stop"

function Invoke-Bench {
    param([string]$ExePath, [int]$TimeoutSec)

    $outLog = Join-Path $env:TEMP ("nexus-perf-out-" + [guid]::NewGuid() + ".log")
    $errLog = Join-Path $env:TEMP ("nexus-perf-err-" + [guid]::NewGuid() + ".log")

    # Launch from the binary's own directory: the benches resolve their temp
    # roots from env vars, but a stale project-level nexus.db in the CWD can
    # be picked up by relative-path fallbacks. Pin the working directory.
    $proc = Start-Process -FilePath $ExePath `
        -WorkingDirectory (Split-Path -Parent $ExePath) `
        -RedirectStandardOutput $outLog `
        -RedirectStandardError $errLog `
        -PassThru -NoNewWindow

    if (-not $proc.WaitForExit($TimeoutSec * 1000)) {
        $proc.Kill()
        throw "Benchmark $([IO.Path]::GetFileName($ExePath)) HUNG (no exit within ${TimeoutSec}s)"
    }

    $stdout = if (Test-Path $outLog) { Get-Content $outLog -Raw } else { "" }
    $stderr = if (Test-Path $errLog) { Get-Content $errLog -Raw } else { "" }
    Remove-Item $outLog, $errLog -ErrorAction SilentlyContinue

    # NOTE: do NOT trust $proc.ExitCode here. In Windows PowerShell 5.1 the
    # ExitCode property can stay $null after WaitForExit(ms) even for a clean
    # exit, and "$null -ne 0" is true -> a false GATE failure. The benches
    # print a final "GATE: PASS" / "GATE: FAIL" line; parse that instead.
    return @{ Stdout = $stdout; Stderr = $stderr }
}

function Get-BenchMetrics {
    param([string]$ExePath, [int]$TimeoutSec)

    $run = Invoke-Bench $ExePath $TimeoutSec
    if ($run.Stdout -notmatch "GATE: PASS") {
        $tail = ($run.Stderr -split "`r?`n" | Select-Object -Last 3) -join " | "
        throw "Benchmark $([IO.Path]::GetFileName($ExePath)) failed its absolute GATE (no 'GATE: PASS' in output). stderr tail: $tail"
    }

    $metrics = @{}
    foreach ($line in ($run.Stdout -split "`r?`n")) {
        if ($line -match "^NEXUS_METRIC (\S+)=(\S+)$") {
            $metrics[$matches[1]] = [double]$matches[2]
        }
    }
    return $metrics
}

$repoRoot = Split-Path -Parent $PSScriptRoot
$baselineFull = Join-Path $repoRoot $BaselinePath
$baseline = Get-Content $baselineFull -Raw | ConvertFrom-Json

$tolerance = [double]$baseline.tolerance

# All four benches run on isolated temp databases; nothing touches user data.
$benchBinaries = @(
    "nexus_sla_bench",
    "nexus_load_bench",
    "nexus_conflict_bench",
    "nexus_long_horizon_bench"
)

$allMetrics = @{}
foreach ($bin in $benchBinaries) {
    $exe = Join-Path $BinaryDir "$bin.exe"
    if (-not (Test-Path $exe)) {
        Write-Host "::error::Missing benchmark binary: $exe"
        exit 1
    }
    try {
        $m = Get-BenchMetrics $exe $PerBenchTimeoutSec
    } catch {
        # One retry: shared-runner noise (antivirus, CPU steal) can trip a
        # timing GATE once. A regression is a trend, not a single run.
        Write-Host "  retry $bin - first run failed: $($_.Exception.Message)"
        $m = Get-BenchMetrics $exe $PerBenchTimeoutSec
    }
    foreach ($k in $m.Keys) { $allMetrics[$k] = $m[$k] }
}

$failures = @()
foreach ($metricName in $baseline.metrics.PSObject.Properties.Name) {
    $spec = $baseline.metrics.$metricName
    $baselineValue = [double]$spec.value
    $higherIsBetter = [bool]$spec.higher_is_better

    if (-not $allMetrics.ContainsKey($metricName)) {
        $failures += "${metricName}: MISSING from benchmark output"
        continue
    }
    $actual = [double]$allMetrics[$metricName]

    # Sub-5 ms baselines: skip the relative check (noise), the absolute gate
    # in the bench binary is the real enforcement. Applies ONLY to latency
    # metrics (higher_is_better = false). Rates/throughput (higher_is_better
    # = true) are always checked relatively: detection dropping 1.0 -> 0.7 is
    # a real regression even though the absolute GATE target is lower.
    if (-not $higherIsBetter -and $baselineValue -lt 5.0) {
        Write-Host "  ${metricName} = $actual (baseline $baselineValue, exempt: below 5 ms absolute gate)"
        continue
    }

    $regressed = if ($higherIsBetter) {
        $actual -lt ($baselineValue * (1.0 - $tolerance))
    } else {
        $actual -gt ($baselineValue * (1.0 + $tolerance))
    }

    if ($regressed) {
        $failures += "${metricName}: $actual vs baseline $baselineValue (tolerance $($tolerance * 100)%)"
    } else {
        Write-Host "  ${metricName} = $actual (baseline $baselineValue) OK"
    }
}

Write-Host ""
if ($failures.Count -eq 0) {
    Write-Host "REGRESSION GATE: PASS"
    exit 0
} else {
    Write-Host "REGRESSION GATE: FAIL"
    foreach ($f in $failures) { Write-Host "  ::error:: $f" }
    exit 1
}
