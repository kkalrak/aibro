$ErrorActionPreference = 'Stop'
Set-Location (Split-Path $PSScriptRoot -Parent)
$vswhere = Join-Path ${env:ProgramFiles(x86)} 'Microsoft Visual Studio\Installer\vswhere.exe'
$vs = & $vswhere -latest -products '*' -property installationPath
if (!$vs) { throw 'Visual Studio C++ Build Tools is required.' }
$cmakeRoot = Join-Path $vs 'Common7\IDE\CommonExtensions\Microsoft\CMake'
$env:PATH = "$cmakeRoot\CMake\bin;$cmakeRoot\Ninja;$env:PATH"
$env:CEF_PATH = Join-Path $env:LOCALAPPDATA 'aibro\cef'
cargo run --bin bundle
if ($LASTEXITCODE) { throw 'Build failed.' }
cargo build --bin aibro-agent
if ($LASTEXITCODE) { throw 'Agent build failed.' }
Copy-Item -LiteralPath 'target/debug/aibro-agent.exe' -Destination 'target/bundle/aibro-agent.exe' -Force
New-Item -ItemType Directory -Force 'target/bundle/third_party/cef-rs' | Out-Null
Copy-Item -Path 'third_party/cef-rs/LICENSE-*' -Destination 'target/bundle/third_party/cef-rs' -Force
