[CmdletBinding()]
param(
    [ValidateSet("Frontend", "Full", "Standalone", "All", "Smoke", "SpellCatalogLive")]
    [string]$Mode = "Full",
    [ValidateRange(1, 4)]
    [int]$Jobs = 2,
    [string]$TestPath = "",
    [string]$TargetDir = "",
    [Parameter(Mandatory = $true)]
    [string]$ResultPath
)

$ErrorActionPreference = "Stop"
$repoRoot = Split-Path -Parent $PSScriptRoot
Set-Location -LiteralPath $repoRoot

# Keep validation in the hidden worker, but let npm, Node, Cargo, rustc, and
# linker children run at the normal Windows scheduling priority.
$current = Get-Process -Id $PID
$current.PriorityClass = "Normal"

$env:CARGO_BUILD_JOBS = [string]$Jobs
$env:RAYON_NUM_THREADS = [string]$Jobs
$env:GOMAXPROCS = [string]$Jobs
$env:CARGO_TARGET_DIR = if ($TargetDir) {
    [IO.Path]::GetFullPath((Join-Path $repoRoot $TargetDir))
} else {
    Join-Path $repoRoot "target-background-validation"
}

function Invoke-ValidationStep {
    param([string]$Label, [scriptblock]$Command)
    Write-Output "`n=== $Label ==="
    & $Command
    if ($LASTEXITCODE -ne 0) {
        throw "$Label failed with exit code $LASTEXITCODE"
    }
}

$startedAt = (Get-Date).ToUniversalTime()
try {
    if ($Mode -eq "Smoke") {
        Write-Output "Background validation worker ready: priority=$($current.PriorityClass), jobs=$Jobs"
    }
    if ($Mode -eq "SpellCatalogLive") {
        Push-Location -LiteralPath (Join-Path $repoRoot "src-tauri")
        try {
            Invoke-ValidationStep "Live P99 spell class catalog audit" { & cargo test infrastructure::spell_catalog::tests::downloads_the_complete_spell_category_in_batches --jobs $Jobs -- --ignored --exact --nocapture }
        } finally {
            Pop-Location
        }
    }    if ($Mode -in @("Frontend", "Full", "All")) {
        $testArguments = @("test", "--", "--maxWorkers=$Jobs", "--minWorkers=1")
        if ($TestPath) { $testArguments += $TestPath }
        Invoke-ValidationStep "Frontend tests" { & npm.cmd @testArguments }
        Invoke-ValidationStep "TypeScript production build" { & npm.cmd run build }
    }
    if ($Mode -in @("Full", "All")) {
        Push-Location -LiteralPath (Join-Path $repoRoot "src-tauri")
        try {
            Invoke-ValidationStep "Rust formatting" { & cargo fmt --all -- --check }
            Invoke-ValidationStep "Rust library tests" { & cargo test --lib --jobs $Jobs }
            Invoke-ValidationStep "Strict Clippy" { & cargo clippy --all-targets --all-features --jobs $Jobs -- -D warnings }
        } finally {
            Pop-Location
        }
    }
    if ($Mode -in @("Standalone", "All")) {
        Invoke-ValidationStep "Standalone application build" { & npm.cmd run build:standalone }
    }
    $result = [ordered]@{ status = "passed"; mode = $Mode; jobs = $Jobs; startedAt = $startedAt.ToString("o"); finishedAt = (Get-Date).ToUniversalTime().ToString("o") }
    [IO.File]::WriteAllText($ResultPath, ($result | ConvertTo-Json), (New-Object System.Text.UTF8Encoding($false)))
    exit 0
} catch {
    Write-Error $_
    $result = [ordered]@{ status = "failed"; mode = $Mode; jobs = $Jobs; message = $_.Exception.Message; startedAt = $startedAt.ToString("o"); finishedAt = (Get-Date).ToUniversalTime().ToString("o") }
    [IO.File]::WriteAllText($ResultPath, ($result | ConvertTo-Json), (New-Object System.Text.UTF8Encoding($false)))
    exit 1
}
