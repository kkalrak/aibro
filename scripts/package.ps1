param([string]$Version = '0.1.0')
$ErrorActionPreference = 'Stop'
Set-Location (Split-Path $PSScriptRoot -Parent)
if ($Version -notmatch '^\d+\.\d+\.\d+([-+][A-Za-z0-9.-]+)?$') { throw 'Invalid version.' }
$bundle = Join-Path $PWD 'target/bundle'
$stage = Join-Path $PWD "target/packages/aibro-$Version-windows-x64"
if (Test-Path -LiteralPath $stage) { throw "Package staging directory already exists: $stage. Use a new version or inspect and remove it first." }
foreach ($required in @('aibro.exe','aibro.dll','aibro-agent.exe','libcef.dll','CREDITS.html','locales','third_party','LICENSE-MIT','LICENSE-APACHE')) {
    if (!(Test-Path -LiteralPath (Join-Path $bundle $required))) { throw "Missing bundle file: $required. Run scripts/build.ps1 -Release first." }
}
New-Item -ItemType Directory -Path $stage -Force | Out-Null
# Explicit runtime allowlist: never include profiles, connection tokens, logs or debug artifacts.
foreach ($pattern in @('*.exe','*.dll','*.pak','*.dat','*.bin','*.manifest','vk_swiftshader_icd.json','CREDITS.html','LICENSE-MIT','LICENSE-APACHE')) {
    Get-ChildItem -LiteralPath $bundle -Filter $pattern -File | Copy-Item -Destination $stage
}
foreach ($directory in @('locales','third_party')) {
    Copy-Item -LiteralPath (Join-Path $bundle $directory) -Destination $stage -Recurse
}
Copy-Item -LiteralPath README.md -Destination $stage
$archive = "$stage.zip"
Compress-Archive -LiteralPath $stage -DestinationPath $archive -CompressionLevel Optimal
$hash = (Get-FileHash -LiteralPath $archive -Algorithm SHA256).Hash.ToLowerInvariant()
[IO.File]::WriteAllText("$archive.sha256", "$hash  $([IO.Path]::GetFileName($archive))`n", [Text.UTF8Encoding]::new($false))
Get-Item -LiteralPath $archive,"$archive.sha256" | Select-Object FullName,Length
