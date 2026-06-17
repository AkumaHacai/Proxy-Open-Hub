param(
    [switch]$SkipFlutter
)

$ErrorActionPreference = "Stop"
$Root = Resolve-Path (Join-Path $PSScriptRoot "..")
$Flutter = Join-Path $env:USERPROFILE "devtools\flutter\bin\flutter.bat"
$Dart = Join-Path $env:USERPROFILE "devtools\flutter\bin\dart.bat"
$Cargo = Join-Path $env:USERPROFILE ".cargo\bin\cargo.exe"

if (-not (Test-Path $Cargo)) {
    throw "cargo.exe was not found at $Cargo"
}

Push-Location $Root
try {
    & $Cargo fmt --all --check
    & $Cargo test --workspace

    if (-not $SkipFlutter) {
        if (-not (Test-Path $Flutter)) {
            throw "flutter.bat was not found at $Flutter"
        }
        if (-not (Test-Path $Dart)) {
            throw "dart.bat was not found at $Dart"
        }

        Push-Location (Join-Path $Root "apps\desktop_flutter")
        try {
            & $Dart format lib test --set-exit-if-changed
            & $Flutter analyze
            & $Flutter test
        }
        finally {
            Pop-Location
        }
    }
}
finally {
    Pop-Location
}
