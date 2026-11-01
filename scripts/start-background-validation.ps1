[CmdletBinding()]
param(
    [ValidateSet("Frontend", "Full", "Standalone", "All", "Smoke", "SpellCatalogLive")]
    [string]$Mode = "Full",
    [ValidateRange(1, 4)]
    [int]$Jobs = 2,
    [string]$TestPath = "",
    [string]$TargetDir = ""
)

$ErrorActionPreference = "Stop"
$worker = Join-Path $PSScriptRoot "validation-worker.ps1"
$logRoot = Join-Path $env:TEMP "everquest-loot-tracker-validation"
[IO.Directory]::CreateDirectory($logRoot) | Out-Null
$runId = (Get-Date -Format "yyyyMMdd-HHmmss") + "-" + ([Guid]::NewGuid().ToString("N").Substring(0, 6))
$stdout = Join-Path $logRoot "$runId.out.log"
$stderr = Join-Path $logRoot "$runId.err.log"
$result = Join-Path $logRoot "$runId.result.json"
$arguments = @("-NoLogo", "-NoProfile", "-NonInteractive", "-ExecutionPolicy", "Bypass", "-File", $worker, "-Mode", $Mode, "-Jobs", [string]$Jobs, "-ResultPath", $result)
if ($TestPath) { $arguments += @("-TestPath", $TestPath) }
if ($TargetDir) { $arguments += @("-TargetDir", $TargetDir) }
$process = Start-Process -FilePath "powershell.exe" -ArgumentList $arguments -WindowStyle Hidden -RedirectStandardOutput $stdout -RedirectStandardError $stderr -PassThru
try { $process.PriorityClass = "Normal" } catch { }

[pscustomobject]@{
    ProcessId = $process.Id
    Mode = $Mode
    Jobs = $Jobs
    StandardOutput = $stdout
    StandardError = $stderr
    Result = $result
}
