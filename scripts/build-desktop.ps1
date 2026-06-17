param(
    [ValidateSet("debug", "release")]
    [string]$RustProfile = "debug"
)

$ErrorActionPreference = "Stop"
$Root = Resolve-Path (Join-Path $PSScriptRoot "..")
$Flutter = Join-Path $env:USERPROFILE "devtools\flutter\bin\flutter.bat"
$Cargo = Join-Path $env:USERPROFILE ".cargo\bin\cargo.exe"

if (-not (Test-Path $Flutter)) {
    throw "flutter.bat was not found at $Flutter"
}
if (-not (Test-Path $Cargo)) {
    throw "cargo.exe was not found at $Cargo"
}

Push-Location $Root
try {
    $cargoArgs = @("build", "-p", "poh_cli")
    if ($RustProfile -eq "release") {
        $cargoArgs += "--release"
    }
    & $Cargo @cargoArgs

    Push-Location (Join-Path $Root "apps\desktop_flutter")
    try {
        & $Flutter build windows
    }
    finally {
        Pop-Location
    }

    $profileDir = if ($RustProfile -eq "release") { "release" } else { "debug" }
    $Cli = Join-Path $Root "target\$profileDir\poh_cli.exe"
    $ReleaseDir = Join-Path $Root "apps\desktop_flutter\build\windows\x64\runner\Release"
    if (-not (Test-Path $Cli)) {
        throw "poh_cli.exe was not produced at $Cli"
    }
    if (-not (Test-Path $ReleaseDir)) {
        throw "Flutter release directory was not produced at $ReleaseDir"
    }

    Copy-Item -LiteralPath $Cli -Destination (Join-Path $ReleaseDir "poh_cli.exe") -Force
    $NativeSource = Join-Path $Root "native\bundled\win-x64"
    $NativeTarget = Join-Path $ReleaseDir "native\bundled\win-x64"
    if (Test-Path $NativeSource) {
        New-Item -ItemType Directory -Path $NativeTarget -Force | Out-Null
        Copy-Item -Path (Join-Path $NativeSource "*") -Destination $NativeTarget -Recurse -Force
    }
    Write-Host "Built Proxy Open Hub:"
    Write-Host "  $ReleaseDir\proxy_open_hub.exe"
    Write-Host "  $ReleaseDir\poh_cli.exe"
}
finally {
    Pop-Location
}
