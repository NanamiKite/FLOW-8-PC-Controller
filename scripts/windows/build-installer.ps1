param(
    [string]$OutputDir
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

if ($env:OS -ne 'Windows_NT') {
    throw 'Build this installer on the Windows host.'
}
if (-not $env:LOCALAPPDATA) {
    throw 'LOCALAPPDATA is required for native Windows build artifacts.'
}

$repoRoot = (Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path
$env:CARGO_TARGET_DIR = Join-Path $env:LOCALAPPDATA 'flow8-rust-target'
if (-not $OutputDir) {
    $OutputDir = Join-Path $env:LOCALAPPDATA 'FLOW 8 PC Controller\installer-output'
}

$rustHostOutput = & rustc -vV 2>&1
$rustcExitCode = $LASTEXITCODE
if ($rustcExitCode -ne 0) {
    throw "rustc -vV failed (exit $rustcExitCode): $($rustHostOutput -join ' ')"
}
$rustHostMatch = [regex]::Match(($rustHostOutput -join "`n"), '(?m)^[ \t]*host:[ \t]*(\S+)[ \t]*\r?$')
if (-not $rustHostMatch.Success) {
    throw "Could not find a host line in rustc -vV output: $($rustHostOutput -join ' ')"
}
$rustHostTriple = $rustHostMatch.Groups[1].Value
if ($rustHostTriple -notin @('x86_64-pc-windows-gnu', 'x86_64-pc-windows-msvc')) {
    throw "This package needs a native Windows x64 Rust host; found $rustHostTriple."
}

Push-Location $repoRoot
try {
    $metadataJson = & cargo metadata --locked --no-deps --format-version 1
    if ($LASTEXITCODE -ne 0) { throw 'cargo metadata failed.' }
    $metadata = $metadataJson | ConvertFrom-Json
    $guiPackage = $metadata.packages | Where-Object { $_.name -eq 'flow8-gui' } | Select-Object -First 1
    if (-not $guiPackage) { throw 'flow8-gui package is missing from the workspace.' }
    $appVersion = [string]$guiPackage.version

    # Do not use an explicit --target on a matching Windows host: that creates
    # a second copy of release artifacts under the same target directory.
    & cargo build --release --locked -p flow8-gui
    if ($LASTEXITCODE -ne 0) { throw "cargo build failed (exit $LASTEXITCODE)." }
} finally {
    Pop-Location
}

$binaryDir = Join-Path $env:CARGO_TARGET_DIR 'release'
$guiExe = Join-Path $binaryDir 'flow8-gui.exe'
if (-not (Test-Path -LiteralPath $guiExe -PathType Leaf)) {
    throw "Windows GUI binary is missing: $guiExe"
}

$compiler = Get-Command ISCC.exe -ErrorAction SilentlyContinue
if ($compiler) {
    $iscc = $compiler.Source
} else {
    $programFilesX86 = [Environment]::GetFolderPath('ProgramFilesX86')
    $programFiles = [Environment]::GetFolderPath('ProgramFiles')
    $candidates = @(
        (Join-Path $programFilesX86 'Inno Setup 7\ISCC.exe'),
        (Join-Path $programFiles 'Inno Setup 7\ISCC.exe'),
        (Join-Path $programFilesX86 'Inno Setup 6\ISCC.exe'),
        (Join-Path $programFiles 'Inno Setup 6\ISCC.exe')
    )
    $iscc = $candidates | Where-Object { Test-Path -LiteralPath $_ -PathType Leaf } | Select-Object -First 1
}
if (-not $iscc) {
    throw 'ISCC.exe was not found. Install Inno Setup 6.4 or newer on the Windows host.'
}

New-Item -ItemType Directory -Force -Path $OutputDir | Out-Null
$installerScript = Join-Path $repoRoot 'installer\flow8.iss'
& $iscc "/DFlow8BinaryDir=$binaryDir" "/DFlow8Version=$appVersion" "/O$OutputDir" $installerScript
if ($LASTEXITCODE -ne 0) { throw "Inno Setup compilation failed (exit $LASTEXITCODE)." }

$installer = Join-Path $OutputDir "FLOW-8-PC-Controller-$appVersion-windows-x64.exe"
if (-not (Test-Path -LiteralPath $installer -PathType Leaf)) {
    throw "Inno Setup reported success but installer is missing: $installer"
}

Write-Host "Installer: $installer"
Write-Host "GUI SHA-256: $((Get-FileHash -LiteralPath $guiExe -Algorithm SHA256).Hash)"
Write-Host "Installer SHA-256: $((Get-FileHash -LiteralPath $installer -Algorithm SHA256).Hash)"
