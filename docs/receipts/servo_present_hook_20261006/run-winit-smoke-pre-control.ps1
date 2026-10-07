# Bounded native donor receipt. This is not a full Servo shutdown proof.
$ErrorActionPreference = 'Stop'
$nativeDllsQualified = $true
if (-not $nativeDllsQualified) {
    throw 'Native build is pending; bind this runner to its completed ANGLE OUT_DIR and hashes before execution.'
}
$receiptDir = $PSScriptRoot
$binaryDir = 'C:/t/cargo-targets/wgpu-graft/debug'
$binaryPath = Join-Path $binaryDir 'demo-servo-winit.exe'
$expectedBinaryHash = '5C20C3BC239DC1D3E6B7532A9FC04B0E1A8836238E16A53542227A772BEEEFCD'
if ((Get-FileHash -LiteralPath $binaryPath -Algorithm SHA256).Hash -ne $expectedBinaryHash) {
    throw 'Donor executable differs from the completed final native build.'
}
$angleDir = 'C:/t/cargo-targets/wgpu-graft/debug/build/mozangle-f2b6807973fe390c/out'
$expectedHashes = @{
    'libEGL.dll' = 'A4C0B14F0B6A1BDE6AE88F5512CF22214A14722209AFDAA339C903F255D04D49'
    'libGLESv2.dll' = 'CA963BE19F9D69F108C98784D083E5910651AD1B5941677B182A99463D4597B3'
}
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
$ownedProcess = Start-Process -FilePath $binaryPath -ArgumentList '--smoke' -WorkingDirectory $binaryDir -WindowStyle Hidden -PassThru -RedirectStandardOutput (Join-Path $receiptDir 'winit-wgpu30-smoke.stdout.log') -RedirectStandardError (Join-Path $receiptDir 'winit-wgpu30-smoke.stderr.log')
$ownedProcess.PriorityClass = 'BelowNormal'
$loadedModules = @{}
$timer = [System.Diagnostics.Stopwatch]::StartNew()
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
    throw 'Owned donor exceeded its bounded 40-second runtime.'
}
$ownedProcess.WaitForExit()
$binaryVersion = (Get-Item -LiteralPath $binaryPath).VersionInfo
$result = [ordered]@{
    executable = $binaryPath
    executable_sha256 = (Get-FileHash -LiteralPath $binaryPath -Algorithm SHA256).Hash
    executable_file_version = $binaryVersion.FileVersion
    executable_product_version = $binaryVersion.ProductVersion
    process_id = $ownedProcess.Id
    exit_code = $ownedProcess.ExitCode
    elapsed_seconds = $timer.Elapsed.TotalSeconds
    requested_backend = 'dx12'
    requested_priority = 'BelowNormal'
    loaded_angle = @($loadedModules.Values | Sort-Object module)
    shutdown_scope = 'Donor process::exit; full process-root teardown is not qualified.'
}
$result | ConvertTo-Json -Depth 5 | Set-Content -LiteralPath (Join-Path $receiptDir 'winit-wgpu30-smoke-provenance.json') -Encoding utf8
if ($ownedProcess.ExitCode -ne 0) { exit $ownedProcess.ExitCode }
if ($loadedModules.Count -ne 2) { throw 'Smoke completed without observing both ANGLE modules.' }
foreach ($loadedModule in $loadedModules.Values) {
    if ($loadedModule.sha256 -ne $expectedHashes[$loadedModule.module]) {
        throw "Observed an unexpected ANGLE module: $($loadedModule.path)"
    }
}
Write-Output 'Native donor exited successfully with both exact ANGLE modules observed.'
