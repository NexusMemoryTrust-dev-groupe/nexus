# Fetches the real OSS corpora used by the retrieval benchmark.
# Run from the repo root: powershell -ExecutionPolicy Bypass -File benchmarks/retrieval/fetch-projects.ps1
$ErrorActionPreference = 'Stop'
$dir = Join-Path $PSScriptRoot 'projects'
New-Item -ItemType Directory -Path $dir -Force | Out-Null

$repos = @(
    @{ Name = 'requests';  Url = 'https://github.com/psf/requests' },
    @{ Name = 'rust-log';  Url = 'https://github.com/rust-lang/log' }
)

foreach ($r in $repos) {
    $target = Join-Path $dir $r.Name
    if (Test-Path (Join-Path $target '.git')) { Write-Host "skip $($r.Name) (already present)"; continue }
    Write-Host "cloning $($r.Name)…"
    git clone --depth 1 $r.Url $target
}

$mui = Join-Path $dir 'mui'
if (-not (Test-Path (Join-Path $mui '.git'))) {
    Write-Host 'cloning mui (sparse: mui-material/src + material-ui/src)…'
    git clone --depth 1 --filter=blob:none --sparse https://github.com/mui/material-ui $mui
    git -C $mui sparse-checkout set packages/mui-material/src packages/material-ui/src
}

Write-Host 'Done. Corpus ready for:'
Write-Host '  cargo run --release --bin nexus_bench -- --projects benchmarks/retrieval/projects --cases benchmarks/retrieval/cases.json'
