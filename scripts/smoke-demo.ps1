# Copyright 2026 Mark Alan Boykin
# This Source Code Form is subject to the terms of the Mozilla Public
# License, v. 2.0. If a copy of the MPL was not distributed with this
# file, You can obtain one at https://mozilla.org/MPL/2.0/.
# SPDX-License-Identifier: MPL-2.0

param(
    [string]$Package = "demo-servo-winit",
    [string]$Binary = "",
    [int]$Seconds = 90,
    [string[]]$DemoArgs = @("--smoke"),
    [string]$Receipt = "GRAFT DEMO SMOKE PASS",
    [switch]$SurvivalOnly
)

$ErrorActionPreference = "Stop"

# ESP-IDF commonly leaves LIBCLANG_PATH pointing at its Xtensa libclang.
# mozangle's bindgen step needs a desktop frontend to parse the MSVC headers.
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

if ($llvmCandidates.Count -eq 0) {
    throw "desktop libclang.dll not found; install LLVM or set LIBCLANG_PATH to its bin directory"
}
$env:LIBCLANG_PATH = $llvmCandidates[0]
Write-Host "Using desktop libclang: $env:LIBCLANG_PATH"

$repo = Resolve-Path (Join-Path $PSScriptRoot "..")
Write-Host "Building deterministic smoke target: cargo build --locked -p $Package"
& cargo build --locked -p $Package
if ($LASTEXITCODE -ne 0) {
    exit $LASTEXITCODE
}

$metadataJson = & cargo metadata --locked --no-deps --format-version 1
if ($LASTEXITCODE -ne 0) {
    exit $LASTEXITCODE
}
$targetRoot = ($metadataJson | ConvertFrom-Json).target_directory
if (-not $targetRoot) {
    throw "cargo metadata did not report a target directory"
}

# A shared target directory may already contain unrelated libEGL/libGLESv2
# files (CEF ships DLLs with the same names). Always install the pair produced
# by this build's newest mozangle output before launching the standalone exe.
$buildRoot = Join-Path $targetRoot "debug\build"
$angleRuntime = Get-ChildItem -LiteralPath $buildRoot -Directory -Filter "mozangle-*" |
    ForEach-Object {
        $egl = Join-Path $_.FullName "out\libEGL.dll"
        $gles = Join-Path $_.FullName "out\libGLESv2.dll"
        if ((Test-Path -LiteralPath $egl -PathType Leaf) -and
            (Test-Path -LiteralPath $gles -PathType Leaf)) {
            [pscustomobject]@{
                Directory = Join-Path $_.FullName "out"
                WrittenAt = [Math]::Max(
                    (Get-Item -LiteralPath $egl).LastWriteTimeUtc.Ticks,
                    (Get-Item -LiteralPath $gles).LastWriteTimeUtc.Ticks
                )
            }
        }
    } |
    Sort-Object WrittenAt -Descending |
    Select-Object -First 1
if (-not $angleRuntime) {
    throw "mozangle runtime DLLs were not produced under $buildRoot"
}
foreach ($dll in @("libEGL.dll", "libGLESv2.dll")) {
    Copy-Item -LiteralPath (Join-Path $angleRuntime.Directory $dll) `
        -Destination (Join-Path $targetRoot "debug\$dll") -Force
}
Write-Host "Installed ANGLE runtime from: $($angleRuntime.Directory)"

if (-not $Binary) {
    $Binary = $Package
}
$binaryFile = if ($IsWindows) { "$Binary.exe" } else { $Binary }
$binaryPath = [System.IO.Path]::GetFullPath((Join-Path $targetRoot "debug\$binaryFile"))
if (-not (Test-Path -LiteralPath $binaryPath -PathType Leaf)) {
    throw "built demo executable not found at $binaryPath"
}

$startInfo = [System.Diagnostics.ProcessStartInfo]::new()
$startInfo.FileName = $binaryPath
$startInfo.WorkingDirectory = $repo
$startInfo.UseShellExecute = $false
$startInfo.RedirectStandardOutput = $true
$startInfo.RedirectStandardError = $true
foreach ($argument in $DemoArgs) {
    $startInfo.ArgumentList.Add($argument)
}

Write-Host "Starting deterministic smoke: $binaryPath $($DemoArgs -join ' ')"
$process = [System.Diagnostics.Process]::new()
$process.StartInfo = $startInfo
if (-not $process.Start()) {
    throw "failed to start $Package"
}

$stdoutTask = $process.StandardOutput.ReadToEndAsync()
$stderrTask = $process.StandardError.ReadToEndAsync()
$timedOut = -not $process.WaitForExit($Seconds * 1000)
if ($timedOut) {
    $process.Kill($true)
    $process.WaitForExit()
}

$stdout = $stdoutTask.GetAwaiter().GetResult()
$stderr = $stderrTask.GetAwaiter().GetResult()
if ($stdout) { Write-Host $stdout.TrimEnd() }
if ($stderr) { [Console]::Error.WriteLine($stderr.TrimEnd()) }

if ($timedOut) {
    if ($SurvivalOnly) {
        Write-Host "Process survived $Seconds seconds; survival-only smoke passed."
        exit 0
    }
    throw "demo timed out after $Seconds seconds without a deterministic receipt"
}
if ($process.ExitCode -ne 0) {
    exit $process.ExitCode
}
if (-not $SurvivalOnly -and -not (($stdout + $stderr).Contains($Receipt))) {
    throw "demo exited successfully without required receipt: $Receipt"
}

Write-Host "Deterministic smoke passed."
