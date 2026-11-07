param(
    [string]$EqToolRoot = (Join-Path $PSScriptRoot '..\..\_reference_eqtool')
)

$ErrorActionPreference = 'Stop'
$sourceMaps = Join-Path $EqToolRoot 'EQTool\map_files'
$sourceZones = Join-Path $EqToolRoot 'EQToolShared\Zones.cs'
$sourceLicense = Join-Path $EqToolRoot 'LICENSE'
$target = Join-Path $PSScriptRoot '..\public\maps'

if (-not (Test-Path -LiteralPath $sourceMaps)) { throw "EqTool map files not found at $sourceMaps" }
if (-not (Test-Path -LiteralPath $sourceZones)) { throw "EqTool zone mappings not found at $sourceZones" }
if (-not (Test-Path -LiteralPath $sourceLicense)) { throw "EqTool license not found at $sourceLicense" }

New-Item -ItemType Directory -Path $target -Force | Out-Null
Get-ChildItem -LiteralPath $sourceMaps -Filter '*.txt' | Copy-Item -Destination $target -Force
Copy-Item -LiteralPath $sourceLicense -Destination (Join-Path $target 'EQTOOL-LICENSE.txt') -Force

$aliases = @{}
$zoneSource = Get-Content -LiteralPath $sourceZones -Raw
[regex]::Matches($zoneSource, 'ZoneNameMapper\.Add\("([^"]+)",\s*"([^"]+)"\)') | ForEach-Object {
    $longName = $_.Groups[1].Value
    $shortName = $_.Groups[2].Value.ToLowerInvariant()
    if (-not $aliases.ContainsKey($shortName)) { $aliases[$shortName] = [System.Collections.Generic.List[string]]::new() }
    $aliases[$shortName].Add($longName)
}

$groups = Get-ChildItem -LiteralPath $sourceMaps -Filter '*.txt' | Group-Object {
    [regex]::Replace($_.BaseName, '_\d+$', '').ToLowerInvariant()
}
$catalog = foreach ($group in $groups | Sort-Object Name) {
    $shortName = $group.Name
    $zoneAliases = if ($aliases.ContainsKey($shortName)) { @($aliases[$shortName] | Sort-Object -Unique) } else { @() }
    $label = if ($aliases.ContainsKey($shortName) -and $aliases[$shortName].Count -gt 0) {
        (Get-Culture).TextInfo.ToTitleCase($aliases[$shortName][0])
    } else {
        (Get-Culture).TextInfo.ToTitleCase(($shortName -replace '_', ' '))
    }
    [ordered]@{
        shortName = $shortName
        label = $label
        aliases = @($zoneAliases)
        files = @($group.Group | Sort-Object Name | ForEach-Object { $_.Name })
    }
}

$catalog | ConvertTo-Json -Depth 5 | Set-Content -LiteralPath (Join-Path $target 'catalog.json') -Encoding utf8
Write-Host "Generated $($catalog.Count) zones and $((Get-ChildItem -LiteralPath $target -Filter '*.txt').Count) map layers in $target"
