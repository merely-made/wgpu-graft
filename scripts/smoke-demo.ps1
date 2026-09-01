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

if (-not $Binary) {
    $Binary = $Package
}
$binaryFile = if ($IsWindows) { "$Binary.exe" } else { $Binary }
$targetRoot = if ($env:CARGO_TARGET_DIR) { $env:CARGO_TARGET_DIR } else { Join-Path $repo "target" }
if (-not [System.IO.Path]::IsPathRooted($targetRoot)) {
    $targetRoot = Join-Path $repo $targetRoot
}
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
