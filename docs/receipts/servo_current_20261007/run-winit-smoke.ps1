# Bounded native donor receipt. This is not a full Servo shutdown proof.
param([string]$BindingPath = (Join-Path $PSScriptRoot 'native30-build-binding.json'))
$RawPresentControl = $false
$ErrorActionPreference = 'Stop'
$receiptDir = $PSScriptRoot
$outputPrefix = 'current-servo-winit-wgpu30-smoke'
$stdoutPath = Join-Path $receiptDir ($outputPrefix + '.stdout.log')
$stderrPath = Join-Path $receiptDir ($outputPrefix + '.stderr.log')
$provenancePath = Join-Path $receiptDir ($outputPrefix + '-provenance.json')
foreach ($outputPath in @($stdoutPath, $stderrPath, $provenancePath)) {
    if (Test-Path -LiteralPath $outputPath) { throw "Receipt already exists: $outputPath" }
}
$binding = Get-Content -LiteralPath $BindingPath -Raw | ConvertFrom-Json
$binaryPath = $binding.executable
$binaryDir = Split-Path -Parent $binaryPath
$expectedBinaryHash = $binding.executable_sha256
if ((Get-FileHash -LiteralPath $binaryPath -Algorithm SHA256).Hash -ne $expectedBinaryHash) {
    throw 'Donor executable differs from the completed current-Servo native build.'
}
$angleDir = $binding.angle_out_dir
$expectedHashes = @{}
foreach ($dll in $binding.angle_dlls) { $expectedHashes[$dll.name] = $dll.sha256 }
if ($expectedHashes.Count -ne 2) { throw 'Build binding must contain both ANGLE DLLs.' }
if (Get-Process -Name 'demo-servo-winit' -ErrorAction SilentlyContinue) {
    throw 'An existing donor process owns the executable; refusing a second run.'
}
foreach ($dllName in $expectedHashes.Keys) {
    $sourceDll = Join-Path $angleDir $dllName
    if ((Get-FileHash -LiteralPath $sourceDll -Algorithm SHA256).Hash -ne $expectedHashes[$dllName]) {
        throw "Unexpected ANGLE source bytes: $sourceDll"
    }
    Copy-Item -LiteralPath $sourceDll -Destination (Join-Path $binaryDir $dllName) -Force
}
$env:WGPU_BACKEND = 'dx12'
$ownedArguments = if ($RawPresentControl) { @('--smoke', '--raw-present-control') } else { @('--smoke') }
$ownedProcess = Start-Process -FilePath $binaryPath -ArgumentList $ownedArguments -WorkingDirectory $binaryDir -WindowStyle Hidden -PassThru -RedirectStandardOutput $stdoutPath -RedirectStandardError $stderrPath
$ownedProcess.PriorityClass = 'BelowNormal'
$loadedModules = @{}
$timer = [System.Diagnostics.Stopwatch]::StartNew()
$timedOut = $false
while (-not $ownedProcess.HasExited -and $timer.Elapsed.TotalSeconds -lt 40) {
    try {
        $ownedProcess.Refresh()
        foreach ($module in $ownedProcess.Modules) {
            if ($module.ModuleName -in @('libEGL.dll', 'libGLESv2.dll')) {
                $moduleVersion = (Get-Item -LiteralPath $module.FileName).VersionInfo
                $loadedModules[$module.FileName] = [pscustomobject]@{
                    process_id = $ownedProcess.Id
                    module = $module.ModuleName
                    path = $module.FileName
                    sha256 = (Get-FileHash -LiteralPath $module.FileName -Algorithm SHA256).Hash
                    file_version = $moduleVersion.FileVersion
                    product_version = $moduleVersion.ProductVersion
                }
            }
        }
    } catch {
        if (-not $ownedProcess.HasExited) { Write-Warning $_ }
    }
    Start-Sleep -Milliseconds 100
}
if (-not $ownedProcess.HasExited) {
    Stop-Process -Id $ownedProcess.Id
    $ownedProcess.WaitForExit()
    $timedOut = $true
}
$ownedProcess.WaitForExit()
$binaryVersion = (Get-Item -LiteralPath $binaryPath).VersionInfo
$result = [ordered]@{
    build_binding = $BindingPath
    build_binding_sha256 = (Get-FileHash -LiteralPath $BindingPath -Algorithm SHA256).Hash
    upstream_servo_revision = 'aac43a3f31a259f04a574f5ec4e959c943ad7cc7'
    diagnostic_sync = 'Existing (unchanged adapter default)'
    executable = $binaryPath
    executable_sha256 = (Get-FileHash -LiteralPath $binaryPath -Algorithm SHA256).Hash
    executable_file_version = $binaryVersion.FileVersion
    executable_product_version = $binaryVersion.ProductVersion
    process_id = $ownedProcess.Id
    exit_code = $ownedProcess.ExitCode
    elapsed_seconds = $timer.Elapsed.TotalSeconds
    timed_out = $timedOut
    requested_backend = 'dx12'
    requested_priority = 'BelowNormal'
    arguments = $ownedArguments
    qualification_scope = if ($RawPresentControl) { 'Diagnostic raw swap only; no imported-frame or pixel qualification.' } else { 'GPU-import initial pixel, click pixel, and resize smoke.' }
    loaded_angle = @($loadedModules.Values | Sort-Object module)
    shutdown_scope = 'Donor process::exit; full process-root teardown is not qualified.'
}
$result | ConvertTo-Json -Depth 5 | Set-Content -LiteralPath $provenancePath -Encoding utf8
if ($timedOut) { throw 'Owned donor exceeded its bounded 40-second runtime; provenance preserved.' }
if ($ownedProcess.ExitCode -ne 0) { exit $ownedProcess.ExitCode }
if ($loadedModules.Count -ne 2) { throw 'Smoke completed without observing both ANGLE modules.' }
foreach ($loadedModule in $loadedModules.Values) {
    if ($loadedModule.sha256 -ne $expectedHashes[$loadedModule.module]) {
        throw "Observed an unexpected ANGLE module: $($loadedModule.path)"
    }
}
$expectedMarker = if ($RawPresentControl) { 'GRAFT RAW PRESENT CONTROL: swap completed without importer; no pixel/import qualification' } else { 'GRAFT DEMO SMOKE PASS path=GPU-import' }
if (-not (Select-String -LiteralPath $stdoutPath -Pattern $expectedMarker -SimpleMatch -Quiet)) {
    throw 'Donor exited without the expected scope-specific completion marker.'
}
Write-Output "Native donor completed '$($result.qualification_scope)' with both exact ANGLE modules observed."
