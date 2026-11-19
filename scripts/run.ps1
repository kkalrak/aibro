param([string]$Url = 'https://example.com')
$ErrorActionPreference = 'Stop'
$root = Split-Path $PSScriptRoot -Parent
$exe = Join-Path $root 'target\bundle\aibro.exe'
if (!(Test-Path $exe)) { throw 'Run scripts/build.ps1 first.' }
$env:AIBRO_DATA_DIR = Join-Path $root '.aibro'
Start-Process -FilePath $exe -ArgumentList @("`"--url=$Url`"", "`"--aibro-data-dir=$env:AIBRO_DATA_DIR`"") -WorkingDirectory $root -WindowStyle Hidden
