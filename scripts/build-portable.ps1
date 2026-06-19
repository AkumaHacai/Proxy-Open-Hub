# Build ONE canonical portable Windows bundle from the CURRENT git commit.
#
# Why this exists: builds were being made from random working trees, so the app
# "layered" into different versions. This script always builds release artifacts
# and stamps the output zip + README with the exact commit hash, so every build
# is traceable to a single committed state. Use THIS, do not build by hand.
#
# Usage:  pwsh scripts/build-portable.ps1   (run from repo root or anywhere)

$ErrorActionPreference = "Stop"
$root = Resolve-Path (Join-Path $PSScriptRoot "..")
$flutter = Join-Path $env:USERPROFILE "devtools\flutter\bin\flutter.bat"
$cargo = Join-Path $env:USERPROFILE ".cargo\bin\cargo.exe"

Push-Location $root
try {
    $commit = (git rev-parse --short HEAD).Trim()
    $branch = (git rev-parse --abbrev-ref HEAD).Trim()
    $dirty = (git status --porcelain -- crates apps/desktop_flutter core-registry Cargo.toml Cargo.lock)
    if ($dirty) {
        Write-Warning "Working tree has UNCOMMITTED source changes - this build will NOT match commit $commit exactly:"
        Write-Output $dirty
    }
    $date = Get-Date -Format "yyyy-MM-dd"
    Write-Output "Building portable bundle from branch '$branch' commit $commit ..."

    # 1) Rust backend (release)
    & $cargo build --release -p poh_cli
    if ($LASTEXITCODE -ne 0) { throw "cargo build failed" }

    # 2) Flutter GUI (release)
    Push-Location (Join-Path $root "apps\desktop_flutter")
    try {
        & $flutter build windows --release
        if ($LASTEXITCODE -ne 0) { throw "flutter build failed" }
    } finally { Pop-Location }

    $rel = Join-Path $root "apps\desktop_flutter\build\windows\x64\runner\Release"
    $stage = Join-Path $root "ProxyOpenHub-portable-win-x64"

    # Keep the BUILD folder self-consistent too: refresh the poh_cli.exe that sits
    # next to the GUI there, so running proxy_open_hub.exe directly from build/ also
    # uses the matching backend (avoids the stale-binary / --gui-pid mismatch).
    Copy-Item -Force (Join-Path $root "target\release\poh_cli.exe") (Join-Path $rel "poh_cli.exe")

    # 3) Assemble: copy Release, excluding the stale bundled cli + dev junk
    robocopy $rel $stage /E /XF "poh-visual-check-*.png" "poh_cli.exe" "native_assets.json" /NFL /NDL /NJH /NJS /NP | Out-Null
    if ($LASTEXITCODE -ge 8) { throw "robocopy failed: $LASTEXITCODE" }

    # 4) Fresh release poh_cli.exe next to the GUI (found by _findPohCli)
    Copy-Item -Force (Join-Path $root "target\release\poh_cli.exe") (Join-Path $stage "poh_cli.exe")

    # 5) VC++ runtime so it launches on a clean Windows box
    $sys = "$env:WINDIR\System32"
    foreach ($d in @("msvcp140.dll","vcruntime140.dll","vcruntime140_1.dll")) {
        Copy-Item -Force (Join-Path $sys $d) (Join-Path $stage $d)
    }

    # 6) Stamp README with the exact build identity
    $readme = @"
Proxy Open Hub - portable Windows build
=======================================
Build date: $date   branch: $branch   commit: $commit

Run proxy_open_hub.exe (poh_cli.exe must stay next to it).
- "Local Proxy Gate" needs no admin; TUN / system proxy / DNS need Administrator.
- Only TrustTunnel core is bundled; other cores download on demand.
- Unsigned internal build (SmartScreen may warn: More info -> Run anyway).

This bundle was produced by scripts/build-portable.ps1 from the commit above.
If you report a bug, please include this commit hash so we test the same build.
"@
    Set-Content -Path (Join-Path $stage "README.txt") -Value $readme -Encoding utf8

    # 7) Zip, named with commit so versions never get confused again
    $zip = Join-Path $root ("ProxyOpenHub-portable-$date-$commit-win-x64.zip")
    Compress-Archive -Path $stage -DestinationPath $zip -Force -CompressionLevel Optimal

    # 8) Clean staging (keep only the zip)
    Remove-Item -Recurse -Force $stage

    Write-Output ""
    Write-Output "DONE. Canonical bundle:"
    Get-Item $zip | Select-Object Name, @{n="MB";e={[math]::Round($_.Length/1MB,1)}}, FullName | Format-List
}
finally { Pop-Location }
