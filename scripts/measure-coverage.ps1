param(
    [ValidateRange(1, 16)][int]$Jobs = 2,
    [ValidateRange(1, 16)][int]$TestThreads = 4,
    [switch]$ReadOnlyProbes
)

$ErrorActionPreference = 'Stop'
$repo = Split-Path -Parent $PSScriptRoot
$target = Join-Path $repo 'target/coverage/build'
$run = Join-Path $repo ('target/coverage/runs/' + [DateTime]::UtcNow.ToString('yyyyMMdd-HHmmss-fff'))
$utf8 = [System.Text.UTF8Encoding]::new($false)

function Get-SourceReceipt {
    $paths = @('src', 'vendor', 'assets', '.cargo') | ForEach-Object {
        Get-ChildItem -LiteralPath (Join-Path $repo $_) -File -Recurse
    }
    $paths += @('Cargo.toml', 'Cargo.lock', 'build.rs', 'LICENSE', 'NOTICE', 'THIRD_PARTY_NOTICES.txt') | ForEach-Object {
        Get-Item -LiteralPath (Join-Path $repo $_)
    }
    $files = @($paths | Sort-Object FullName | ForEach-Object {
        [ordered]@{
            path = $_.FullName.Substring($repo.Length).TrimStart([char[]]@('/', '\')).Replace('\', '/')
            sha256 = (Get-FileHash -LiteralPath $_.FullName -Algorithm SHA256).Hash
        }
    })
    $serialized = $files | ConvertTo-Json -Depth 3 -Compress
    $sha = [System.Security.Cryptography.SHA256]::Create()
    try { $fingerprint = [BitConverter]::ToString($sha.ComputeHash($utf8.GetBytes($serialized))).Replace('-', '') }
    finally { $sha.Dispose() }
    return [ordered]@{ fingerprint = $fingerprint; files = $files }
}

$toolchain = (& rustc --print sysroot).Trim()
if ($LASTEXITCODE -ne 0) { throw 'Could not locate the active Rust toolchain.' }
$hostLine = & rustc -vV | Where-Object { $_ -match '^host: ' }
$hostTriple = $hostLine.Substring(6)
$llvm = Join-Path $toolchain "lib/rustlib/$hostTriple/bin"
$profdata = Join-Path $llvm 'llvm-profdata.exe'
$cov = Join-Path $llvm 'llvm-cov.exe'
foreach ($tool in @($profdata, $cov)) {
    if (-not (Test-Path -LiteralPath $tool)) {
        throw "Missing matching LLVM tool: $tool. Install the active toolchain's llvm-tools component."
    }
}

New-Item -ItemType Directory -Path $run | Out-Null
$savedFlags = $env:RUSTFLAGS
$savedEncodedFlags = $env:CARGO_ENCODED_RUSTFLAGS
$savedProfile = $env:LLVM_PROFILE_FILE
$savedSkipLegacy = $env:TRONTOP_PROBE_SKIP_LEGACY
Push-Location $repo
try {
    $sourceBefore = Get-SourceReceipt
    # Keep the app's static CRT setting when environment flags replace config.
    # These flags affect only this process and the isolated coverage target.
    $env:LLVM_PROFILE_FILE = Join-Path $run 'profile-%p-%m.profraw'
    if ($savedEncodedFlags) {
        $env:CARGO_ENCODED_RUSTFLAGS = $savedEncodedFlags + [char]31 + '-C' + [char]31 + 'target-feature=+crt-static' + [char]31 + '-C' + [char]31 + 'instrument-coverage'
    } else {
        $env:RUSTFLAGS = "$savedFlags -C target-feature=+crt-static -C instrument-coverage".Trim()
    }

    Write-Host 'COVERAGE: building the ordinary test binary; no ignored desktop tests.'
    $messages = @(& cargo test --offline --no-run --message-format=json --target-dir $target --jobs $Jobs)
    if ($LASTEXITCODE -ne 0) { throw 'Coverage test compilation failed.' }
    $binaries = @($messages | ForEach-Object {
        $item = $_ | ConvertFrom-Json
        if ($item.reason -eq 'compiler-artifact' -and $item.target.name -eq 'trontop' -and $item.profile.test -and $item.executable) {
            $item.executable
        }
    })
    if ($binaries.Count -ne 1) { throw "Expected one Trontop test binary; found $($binaries.Count)." }
    # Preserve the exact instrumented object; later builds cannot overwrite it.
    $binary = Join-Path $run 'trontop-tests.exe'
    Copy-Item -LiteralPath $binaries[0] -Destination $binary
    if ((Get-FileHash -LiteralPath $binaries[0]).Hash -ne (Get-FileHash -LiteralPath $binary).Hash) {
        throw 'Instrumented binary copy hash mismatch.'
    }
    Write-Host "COVERAGE: running $binary"
    & $binary --quiet "--test-threads=$TestThreads" | Tee-Object -FilePath (Join-Path $run 'ordinary-tests.log')
    if ($LASTEXITCODE -ne 0) { throw 'Instrumented ordinary tests failed; no passing coverage report is claimed.' }

    $executedProbes = @()
    if ($ReadOnlyProbes) {
        # Explicitly reviewed read-only probes only. No native tray, service
        # commands, process control, desktop input or blanket ignored-test run.
        $allowed = @(
            'native_specs_board_read_only_probe',
            'native_specs_bridge_read_only_probe',
            'native_specs_cpu_read_only_probe',
            'native_specs_devices_read_only_probe',
            'native_specs_graphics_read_only_probe',
            'native_specs_memory_read_only_probe',
            'native_specs_network_read_only_probe',
            'native_specs_os_read_only_probe',
            'native_specs_storage_read_only_probe',
            'native_specs_registry_read_only_probe',
            'native_specs_setupapi_read_only_probe',
            'native_specs_smbios_read_only_probe',
            'native_specs_wmi_read_only_probe',
            'native_cpu_distribution_read_only_probe',
            'native_physical_disk_pdh_probe',
            'native_gpu_adapter_memory_read_only_probe',
            'native_nvml_read_only_probe',
            'native_memory_counters_read_only_probe',
            'native_filter_interfaces_hide_sysinfo_duplicates',
            'native_inventory_workers_publish_read_only_snapshots',
            'native_executable_icon_read_only_probe',
            'native_logical_cpu_read_only_probe',
            'native_gpu_refresh_preserves_warm_counters',
            'native_sampler_cadence_read_only_probe'
        )
        $listed = @(& $binary --list --ignored)
        if ($LASTEXITCODE -ne 0) { throw 'Could not list exact ignored probe names.' }
        $env:TRONTOP_PROBE_SKIP_LEGACY = '1'
        foreach ($name in $allowed) {
            $probeMatches = @($listed | Where-Object { $_ -match "::$([Regex]::Escape($name)): test$" })
            if ($probeMatches.Count -ne 1) { throw "Expected one exact read-only probe named $name; found $($probeMatches.Count)." }
            $test = $probeMatches[0].Substring(0, $probeMatches[0].Length - 6)
            Write-Host "COVERAGE: read-only $name"
            # Windows PowerShell treats native stderr (including ordinary
            # eprintln! diagnostics) as terminating errors under Stop. Capture
            # both streams directly and judge only the real process exit code.
            $probeStart = [System.Diagnostics.ProcessStartInfo]::new()
            $probeStart.FileName = $binary
            $probeStart.Arguments = "--exact $test --ignored --nocapture"
            $probeStart.WorkingDirectory = $repo
            $probeStart.UseShellExecute = $false
            $probeStart.CreateNoWindow = $true
            $probeStart.RedirectStandardOutput = $true
            $probeStart.RedirectStandardError = $true
            $probeProcess = [System.Diagnostics.Process]::new()
            $probeProcess.StartInfo = $probeStart
            try {
                if (-not $probeProcess.Start()) { throw "Could not start read-only probe $name." }
                $probeOut = $probeProcess.StandardOutput.ReadToEndAsync()
                $probeError = $probeProcess.StandardError.ReadToEndAsync()
                $probeProcess.WaitForExit()
                [System.IO.File]::WriteAllText((Join-Path $run "$name.log"), $probeOut.GetAwaiter().GetResult(), $utf8)
                [System.IO.File]::WriteAllText((Join-Path $run "$name.stderr.log"), $probeError.GetAwaiter().GetResult(), $utf8)
                if ($probeProcess.ExitCode -ne 0) { throw "Read-only probe failed: $name. See its run logs; no passing report is claimed." }
            } finally { $probeProcess.Dispose() }
            $executedProbes += $test
        }
    }

    $sourceAfter = Get-SourceReceipt
    if ($sourceBefore.fingerprint -ne $sourceAfter.fingerprint) {
        throw 'Source inputs changed during instrumentation; refusing a coverage claim against mixed source.'
    }

    $raw = @(Get-ChildItem -LiteralPath $run -Filter '*.profraw' | ForEach-Object FullName)
    if ($raw.Count -eq 0) { throw 'The tests produced no LLVM raw profiles.' }
    $inputs = Join-Path $run 'profile-inputs.txt'
    [System.IO.File]::WriteAllLines($inputs, [string[]]$raw, $utf8)
    $indexed = Join-Path $run 'coverage.profdata'
    & $profdata merge -sparse "--input-files=$inputs" -o $indexed
    if ($LASTEXITCODE -ne 0) { throw 'LLVM profile merge failed.' }

    # Explicit paths scope the report to all application source, including
    # inline/test modules. They do not exclude untested production files.
    $sources = @(Get-ChildItem -LiteralPath (Join-Path $repo 'src') -Filter '*.rs' -Recurse | ForEach-Object FullName)
    $summary = @(& $cov report $binary "-instr-profile=$indexed" @sources)
    if ($LASTEXITCODE -ne 0) { throw 'LLVM summary generation failed.' }
    [System.IO.File]::WriteAllLines((Join-Path $run 'summary.txt'), [string[]]$summary, $utf8)
    $export = & $cov export $binary "-instr-profile=$indexed" @sources
    if ($LASTEXITCODE -ne 0) { throw 'LLVM JSON export failed.' }
    [System.IO.File]::WriteAllText((Join-Path $run 'coverage.json'), ($export -join "`n"), $utf8)
    $lcov = & $cov export $binary "-instr-profile=$indexed" -format=lcov @sources
    if ($LASTEXITCODE -ne 0) { throw 'LLVM LCOV export failed.' }
    [System.IO.File]::WriteAllText((Join-Path $run 'coverage.lcov'), ($lcov -join "`n"), $utf8)
    & $cov show $binary "-instr-profile=$indexed" --format=html "-output-dir=$(Join-Path $run 'html')" @sources
    if ($LASTEXITCODE -ne 0) { throw 'LLVM HTML generation failed.' }

    # The scope analyzer is a separate tool, not part of the app or its profile.
    $env:RUSTFLAGS = $savedFlags
    $env:CARGO_ENCODED_RUSTFLAGS = $savedEncodedFlags
    $env:LLVM_PROFILE_FILE = $savedProfile
    $scopeManifest = Join-Path $repo 'scripts/coverage-scope/Cargo.toml'
    $scopeTarget = Join-Path $repo 'target/coverage/scope'
    & cargo test --offline --locked --manifest-path $scopeManifest --target-dir $scopeTarget --jobs $Jobs --quiet
    if ($LASTEXITCODE -ne 0) { throw 'Coverage scope analyzer checks failed.' }
    & cargo run --offline --locked --manifest-path $scopeManifest --target-dir $scopeTarget --jobs $Jobs --quiet -- $repo $run
    if ($LASTEXITCODE -ne 0) { throw 'Production line classification failed; refusing a passing report.' }
    if ($sourceBefore.fingerprint -ne (Get-SourceReceipt).fingerprint) {
        throw 'Source changed during report generation; refusing a passing receipt.'
    }
    [System.IO.File]::WriteAllText((Join-Path $run 'source-inputs.json'), ($sourceBefore | ConvertTo-Json -Depth 4), $utf8)
    $production = Get-Content -LiteralPath (Join-Path $run 'production-lines.json') -Raw | ConvertFrom-Json
    $receipt = [ordered]@{
        utc = [DateTime]::UtcNow.ToString('o')
        binary = $binary
        binarySha256 = (Get-FileHash -LiteralPath $binary -Algorithm SHA256).Hash
        sourceCommit = (& git rev-parse HEAD).Trim()
        sourceFingerprint = $sourceBefore.fingerprint
        scriptSha256 = (Get-FileHash -LiteralPath $PSCommandPath -Algorithm SHA256).Hash
        scopeAnalyzerSha256 = (Get-FileHash -LiteralPath (Join-Path $scopeTarget 'debug/trontop-coverage-scope.exe') -Algorithm SHA256).Hash
        scope = 'All compiled src/*.rs, including inline tests and test-only files; no production exclusions.'
        rawProfiles = $raw.Count
        ignoredTestsExecuted = $executedProbes.Count -gt 0
        readOnlyProbes = $executedProbes
        productionUniqueFileLines = $production.production
        mixedFileLines = $production.mixed
        rust = (& rustc -vV) -join "`n"
        summary = @($summary | Where-Object { $_ -match '^TOTAL\s' })
    }
    [System.IO.File]::WriteAllText((Join-Path $run 'receipt.json'), ($receipt | ConvertTo-Json -Depth 5), $utf8)
    [System.IO.File]::WriteAllText((Join-Path $repo 'target/coverage/latest-run.txt'), $run, $utf8)
    Write-Host 'COVERAGE: raw LLVM summary includes tests; separate production report uses unique LCOV file lines. No branch/feature acceptance claim.'
    $receipt.summary | ForEach-Object { Write-Host $_ }
    Write-Host "COVERAGE: report $run/html/index.html"
} finally {
    Pop-Location
    $env:RUSTFLAGS = $savedFlags
    $env:CARGO_ENCODED_RUSTFLAGS = $savedEncodedFlags
    $env:LLVM_PROFILE_FILE = $savedProfile
    $env:TRONTOP_PROBE_SKIP_LEGACY = $savedSkipLegacy
}
