param([Parameter(Mandatory)][string]$Json)
$ErrorActionPreference = 'Stop'
$root = Split-Path $PSScriptRoot -Parent
$c = Get-Content (Join-Path $root '.aibro\connection.json') -Raw | ConvertFrom-Json
$body = [Text.Encoding]::UTF8.GetBytes($Json)
try {
    $response = Invoke-RestMethod -Uri "$($c.url)/command" -Method Post -Headers @{ Authorization = "Bearer $($c.token)" } -ContentType 'application/json; charset=utf-8' -Body $body -TimeoutSec 90
    $response | ConvertTo-Json -Depth 50
} catch {
    if ($_.ErrorDetails.Message) { Write-Output $_.ErrorDetails.Message } else { throw }
    exit 1
}
