param(
    [string[]]$Packages = @(
        "demo-servo-winit",
        "demo-servo-xilem",
        "demo-servo-gpui",
        "demo-servo-bevy",
        "demo-servo-blitz",
        "demo-servo-egui",
        "demo-servo-slint"
    ),
    # WebRender 0.70's shader optimizer asks Cargo's jobserver for a worker.
    # `-j 1` leaves none available and stalls its build script indefinitely.
    [ValidateRange(2, 64)]
    [int]$Jobs = 2
)

$ErrorActionPreference = "Stop"

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

if ($llvmCandidates.Count -eq 0) {
    throw "desktop libclang.dll not found; install LLVM or set LIBCLANG_PATH to its bin directory"
}
$env:LIBCLANG_PATH = $llvmCandidates[0]
Write-Host "Using desktop libclang: $env:LIBCLANG_PATH"

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
        Write-Host "==> cargo check $package"
        & cargo check --locked -j $Jobs -p $package
        if ($LASTEXITCODE -ne 0) {
            $failedPackages.Add($package)
        }
    }

    Write-Host ""
    if ($failedPackages.Count -gt 0) {
        Write-Host "Failed demo checks:"
        foreach ($package in $failedPackages) {
            Write-Host "  - $package"
        }
        exit 1
    }
    Write-Host "All $($Packages.Count) root-workspace Servo demos passed."
    exit 0
}
finally {
    Pop-Location
}
