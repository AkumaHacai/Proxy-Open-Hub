param(
    [switch]$Build,
    [ValidateSet("debug", "release")]
    [string]$RustProfile = "debug"
)

$ErrorActionPreference = "Stop"
$Root = Resolve-Path (Join-Path $PSScriptRoot "..")

if ($Build) {
    & (Join-Path $PSScriptRoot "build-desktop.ps1") -RustProfile $RustProfile
}

$ReleaseDir = Join-Path $Root "apps\desktop_flutter\build\windows\x64\runner\Release"
$Exe = Join-Path $ReleaseDir "proxy_open_hub.exe"
$Cli = Join-Path $ReleaseDir "poh_cli.exe"

if (-not (Test-Path $Exe)) {
    throw "proxy_open_hub.exe was not found. Run scripts\build-desktop.ps1 first."
}
if (-not (Test-Path $Cli)) {
    $FallbackCli = Join-Path $Root "target\debug\poh_cli.exe"
    if (Test-Path $FallbackCli) {
        $Cli = $FallbackCli
    }
    else {
        throw "poh_cli.exe was not found. Run scripts\build-desktop.ps1 first."
    }
}

$env:POH_CLI_PATH = $Cli
Start-Process -FilePath $Exe -WorkingDirectory $ReleaseDir
