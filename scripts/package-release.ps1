param([string]$Executable = 'target/release/trontop.exe', [string]$OutputDirectory = 'target/release-package', [switch]$AllowDirtyForCheck)
$ErrorActionPreference = 'Stop'
$repo = Split-Path -Parent $PSScriptRoot
Push-Location $repo
try {
    $version = [regex]::Match([IO.File]::ReadAllText((Join-Path $repo 'Cargo.toml')), '(?m)^version = "([^"]+)"').Groups[1].Value
    $exe = Get-Item -LiteralPath $Executable
    if ($exe.VersionInfo.ProductVersion -ne $version) { throw 'EXE version does not match Cargo.toml.' }
    $commit = (git rev-parse HEAD).Trim()
    $dirty = [bool](git status --porcelain --untracked-files=normal)
    if ($dirty -and -not $AllowDirtyForCheck) { throw 'Release packaging requires a clean source checkout.' }
    $binaryBuild = $exe.VersionInfo.PrivateBuild
    $expectedBuild = if ($AllowDirtyForCheck -and $binaryBuild -eq ($commit + '+modified')) { $commit + '+modified' } else { $commit }
    if ($binaryBuild -ne $expectedBuild) { throw 'EXE build identity does not match this commit. Rebuild before packaging.' }
    $out = [IO.Path]::GetFullPath((Join-Path $repo $OutputDirectory))
    $stage = Join-Path $out ('trontop-' + $version + '-windows-x64')
    if (Test-Path -LiteralPath $stage) { throw 'Package staging folder already exists; choose a fresh output directory.' }
    New-Item -ItemType Directory -Force -Path $stage | Out-Null
    Copy-Item -LiteralPath $exe.FullName -Destination (Join-Path $stage 'trontop.exe')
    foreach ($name in @('LICENSE','NOTICE','THIRD_PARTY_NOTICES.txt','PRIVACY.md')) { Copy-Item -LiteralPath (Join-Path $repo $name) -Destination $stage }
    Copy-Item -LiteralPath (Join-Path $repo 'docs/RELEASE_ALPHA41.md') -Destination (Join-Path $stage 'README.md')
    $hash = (Get-FileHash -LiteralPath $exe.FullName -Algorithm SHA256).Hash.ToLowerInvariant()
    $signature = (Get-AuthenticodeSignature -LiteralPath $exe.FullName).Status.ToString()
    @{ version=$version; commit=$commit; binaryBuild=$binaryBuild; sourceDirty=$dirty; executableSha256=$hash; bytes=$exe.Length; authenticode=$signature; target='x86_64-pc-windows-msvc' } | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $stage 'build-info.json') -Encoding UTF8
    $zip = $stage + '.zip'
    Compress-Archive -LiteralPath (Get-ChildItem -LiteralPath $stage -File).FullName -DestinationPath $zip -CompressionLevel Optimal
    Copy-Item -LiteralPath $exe.FullName -Destination (Join-Path $out 'trontop.exe')
    Copy-Item -LiteralPath (Join-Path $stage 'build-info.json') -Destination $out
    $zipHash = (Get-FileHash -LiteralPath $zip -Algorithm SHA256).Hash.ToLowerInvariant()
    [IO.File]::WriteAllText((Join-Path $out 'SHA256SUMS.txt'), "$hash  trontop.exe`n$zipHash  $([IO.Path]::GetFileName($zip))`n", (New-Object Text.UTF8Encoding($false)))
    Write-Output "Packaged $version at commit $commit; Authenticode: $signature"
    Get-Content -LiteralPath (Join-Path $out 'SHA256SUMS.txt')
} finally { Pop-Location }
