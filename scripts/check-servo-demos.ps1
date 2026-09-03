# Copyright 2026 Mark Alan Boykin
# This Source Code Form is subject to the terms of the Mozilla Public
# License, v. 2.0. If a copy of the MPL was not distributed with this
# file, You can obtain one at https://mozilla.org/MPL/2.0/.
# SPDX-License-Identifier: MPL-2.0

param(
    [ValidateSet("check", "build")]
    [string]$CargoCommand = "check",
    [string[]]$Packages = @(
        "demo-servo-winit",
        "demo-servo-xilem",
        "demo-servo-gpui",
        "demo-servo-bevy",
        "demo-servo-blitz",
        "demo-servo-egui",
        "demo-servo-slint"
    ),
    [switch]$IncludeIced,
    [string]$TargetDir,
    # WebRender 0.70's shader optimizer asks Cargo's jobserver for a worker.
    # `-j 1` leaves none available and stalls its build script indefinitely.
    [ValidateRange(2, 64)]
    [int]$Jobs = 2
)

$ErrorActionPreference = "Stop"

foreach ($command in @("cargo", "git", "cmake", "nasm")) {
    if (-not (Get-Command $command -ErrorAction SilentlyContinue)) {
        throw "$command not found on PATH; install the Windows Servo build prerequisites before compiling"
    }
}

if ($env:OS -eq "Windows_NT") {
    $vswhere = Join-Path ${env:ProgramFiles(x86)} "Microsoft Visual Studio\Installer\vswhere.exe"
    if (-not (Test-Path -LiteralPath $vswhere)) {
        throw "vswhere.exe not found; install Visual Studio with the MSVC x64 build tools"
    }
    $vsInstall = & $vswhere -latest -products * -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath
    if (-not $vsInstall) {
        throw "Visual Studio MSVC x64 build tools not found"
    }
    $vsVersion = & $vswhere -latest -products * -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationVersion
    $vsMajor = [int]($vsVersion -split "\.")[0]
    $cmakeGenerator = switch ($vsMajor) {
        17 { "Visual Studio 17 2022" }
        18 { "Visual Studio 18 2026" }
        default { throw "unsupported stable Visual Studio major $vsMajor; update the CMake generator mapping" }
    }
    if (-not ((& cmake --help) -match [regex]::Escape($cmakeGenerator))) {
        throw "installed CMake does not support generator '$cmakeGenerator'"
    }
    # CMake's auto-detection can prefer a newer Visual Studio Preview that the
    # installed CMake release cannot generate for. Bind native crates to the
    # stable MSVC installation selected above.
    $env:CMAKE_GENERATOR = $cmakeGenerator
    $windowsSdk = Get-ItemPropertyValue -Path "HKLM:\SOFTWARE\Microsoft\Windows Kits\Installed Roots" -Name KitsRoot10 -ErrorAction SilentlyContinue
    if (-not $windowsSdk -or -not (Test-Path -LiteralPath $windowsSdk)) {
        throw "Windows 10/11 SDK not found"
    }
    $longPaths = (& git config --global --get core.longpaths 2>$null).Trim()
    if ($LASTEXITCODE -ne 0 -or $longPaths -ne "true") {
        throw "git core.longpaths must be enabled globally before Cargo checks out Servo"
    }
    Write-Host "Using Visual Studio: $vsInstall"
    Write-Host "Using CMake generator: $env:CMAKE_GENERATOR"
    Write-Host "Using Windows SDK: $windowsSdk"
}

# ESP-IDF commonly leaves LIBCLANG_PATH pointing at its Xtensa build of
# libclang. Bindgen then parses desktop MSVC headers with the embedded-target
# frontend and reports basic constructs such as __declspec as invalid. Pick a
# desktop LLVM explicitly so a normal PowerShell session is reproducible.
$llvmCandidates = @(
    $env:LIBCLANG_PATH,
    (Join-Path $env:ProgramFiles "LLVM\bin"),
    (Join-Path $env:USERPROFILE "scoop\apps\llvm\current\bin"),
    (Join-Path ${env:ProgramFiles(x86)} "LLVM\bin")
) | Where-Object {
    $_ -and
    $_ -notmatch "(?i)(xtensa|esp-clang)" -and
    (Test-Path -LiteralPath (Join-Path $_ "libclang.dll"))
} | Select-Object -Unique

$desktopLibclangPath = $llvmCandidates | Select-Object -First 1
if (-not $desktopLibclangPath) {
    throw "desktop libclang.dll not found; install LLVM or set LIBCLANG_PATH to its bin directory"
}
$env:LIBCLANG_PATH = $desktopLibclangPath
Write-Host "Using desktop libclang: $env:LIBCLANG_PATH"

if ($TargetDir) {
    $resolvedTargetDir = [System.IO.Path]::GetFullPath($TargetDir)
    $env:CARGO_TARGET_DIR = $resolvedTargetDir
    Write-Host "Using Cargo target directory: $env:CARGO_TARGET_DIR"
}

if (-not $env:CARGO_NET_GIT_FETCH_WITH_CLI) {
    $env:CARGO_NET_GIT_FETCH_WITH_CLI = "true"
}

$repo = Resolve-Path (Join-Path $PSScriptRoot "..")
Push-Location $repo
try {
    # Keep the independent demo graphs independent. Bevy 0.19 enables
    # codespan-reporting's termcolor feature through naga_oil, while Xilem's
    # wgpu 26 Naga edge deliberately does not. Selecting both packages in one
    # Cargo invocation unifies those features and makes Naga 26 uncompilable,
    # even though both binaries compile correctly on their own.
    $failedPackages = [System.Collections.Generic.List[string]]::new()
    foreach ($package in $Packages) {
        Write-Host ""
        Write-Host "==> cargo $CargoCommand $package"
        & cargo $CargoCommand --locked -j $Jobs -p $package
        if ($LASTEXITCODE -ne 0) {
            $failedPackages.Add($package)
        }
    }

    if ($IncludeIced) {
        Write-Host ""
        Write-Host "==> cargo $CargoCommand demo-servo-iced"
        & cargo $CargoCommand --locked -j $Jobs --manifest-path demo-servo-iced/Cargo.toml
        if ($LASTEXITCODE -ne 0) {
            $failedPackages.Add("demo-servo-iced")
        }
    }

    Write-Host ""
    if ($failedPackages.Count -gt 0) {
        Write-Host "Failed demo $CargoCommand commands:"
        foreach ($package in $failedPackages) {
            Write-Host "  - $package"
        }
        exit 1
    }
    $demoCount = $Packages.Count + [int]$IncludeIced.IsPresent
    Write-Host "All $demoCount Servo demo $CargoCommand commands passed."
    exit 0
}
finally {
    Pop-Location
}
