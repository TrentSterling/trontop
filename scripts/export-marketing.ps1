param([Parameter(Mandatory=$true)][string]$SitePath, [switch]$SkipRender)
$ErrorActionPreference = 'Stop'
$repo = Split-Path -Parent $PSScriptRoot
$site = [IO.Path]::GetFullPath($SitePath)
if (-not (Test-Path -LiteralPath (Join-Path $site 'index.html'))) { throw 'SitePath must be the Trontop product page directory.' }
Push-Location $repo
try {
    if (-not $SkipRender) {
        cargo test --locked render_marketing_gallery -- --ignored --nocapture
        if ($LASTEXITCODE -ne 0) { throw 'Screenshot renderer failed.' }
    }
    $source = Join-Path $repo 'target/marketing'
    $media = Join-Path $site 'media'
    New-Item -ItemType Directory -Force -Path $media | Out-Null
    $gallery = Get-Content -LiteralPath (Join-Path $source 'gallery.json') -Raw | ConvertFrom-Json
    foreach ($shot in $gallery) {
        Copy-Item -LiteralPath (Join-Path $source $shot.file) -Destination $media
        Copy-Item -LiteralPath (Join-Path $source $shot.theme_file) -Destination $media
    }
    Copy-Item -LiteralPath (Join-Path $source 'gallery.json') -Destination $media
    Copy-Item -LiteralPath (Join-Path $repo 'assets/branding/trontop-logo-v2.png') -Destination (Join-Path $media 'logo.png')
    Copy-Item -LiteralPath (Join-Path $repo 'assets/fonts/Rajdhani-SemiBold.ttf') -Destination $media
    Copy-Item -LiteralPath (Join-Path $repo 'assets/fonts/OFL.txt') -Destination $media
    $cards = foreach ($shot in $gallery | Where-Object view -eq 'overview') {
        $label = [Net.WebUtility]::HtmlEncode($shot.theme)
        '<figure class="theme-card"><a class="shot-link" href="media/{0}" data-zoom><img src="media/{0}" alt="Trontop Overview in the {1} theme, with demo data" width="1440" height="900" loading="lazy"></a><figcaption><span>{1}</span><a href="media/{2}" download>Get theme JSON</a></figcaption></figure>' -f $shot.file,$label,$shot.theme_file
    }
    $index = Join-Path $site 'index.html'
    $html = [IO.File]::ReadAllText($index)
    $markup = '<!-- THEME_GALLERY -->' + [Environment]::NewLine + ($cards -join [Environment]::NewLine) + [Environment]::NewLine + '<!-- /THEME_GALLERY -->'
    if ($html.Contains('<!-- /THEME_GALLERY -->')) {
        $html = [regex]::Replace($html, '(?s)<!-- THEME_GALLERY -->.*?<!-- /THEME_GALLERY -->', [System.Text.RegularExpressions.MatchEvaluator]{ param($match) $markup })
    } else { $html = $html.Replace('<!-- THEME_GALLERY -->', $markup) }
    [IO.File]::WriteAllText($index,$html,(New-Object Text.UTF8Encoding($false)))
    Write-Output ('Exported {0} screenshots and {1} themes to {2}' -f $gallery.Count,($gallery | Where-Object view -eq 'overview').Count,$media)
} finally { Pop-Location }
