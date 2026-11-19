param([switch]$Release)
$ErrorActionPreference = 'Stop'
Set-Location (Split-Path $PSScriptRoot -Parent)
$vswhere = Join-Path ${env:ProgramFiles(x86)} 'Microsoft Visual Studio\Installer\vswhere.exe'
$vs = & $vswhere -latest -products '*' -property installationPath
if (!$vs) { throw 'Visual Studio C++ Build Tools is required.' }
$cmakeRoot = Join-Path $vs 'Common7\IDE\CommonExtensions\Microsoft\CMake'
$env:PATH = "$cmakeRoot\CMake\bin;$cmakeRoot\Ninja;$env:PATH"
$env:CEF_PATH = Join-Path $env:LOCALAPPDATA 'aibro\cef'
if ($Release) { cargo run --locked --bin bundle -- --release }
else { cargo run --locked --bin bundle }
if ($LASTEXITCODE) { throw 'Build failed.' }
if ($Release) { cargo build --locked --release --bin aibro-agent; $profile = 'release' }
else { cargo build --locked --bin aibro-agent; $profile = 'debug' }
if ($LASTEXITCODE) { throw 'Agent build failed.' }
Copy-Item -LiteralPath "target/$profile/aibro-agent.exe" -Destination 'target/bundle/aibro-agent.exe' -Force
New-Item -ItemType Directory -Force 'target/bundle/third_party/cef-rs' | Out-Null
Copy-Item -Path 'third_party/cef-rs/LICENSE-*' -Destination 'target/bundle/third_party/cef-rs' -Force
Copy-Item -LiteralPath 'LICENSE-MIT','LICENSE-APACHE' -Destination 'target/bundle' -Force
