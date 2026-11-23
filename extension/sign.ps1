# Signs DasIDM's Firefox extension on addons.mozilla.org (AMO).
#
#   powershell -ExecutionPolicy Bypass -File C:\level-2\DasIDM\extension\sign.ps1
#       Private (unlisted): signed in minutes, only you have it; the .xpi lands in dist\.
#   powershell -ExecutionPolicy Bypass -File C:\level-2\DasIDM\extension\sign.ps1 -Listed
#       Public (the Firefox Add-ons store): submitted with the listing text and reviewer notes
#       from amo\metadata.json. Mozilla reviews it first; it goes live when approved.
#
# Run it yourself in a terminal. It asks for your API key
# (https://addons.mozilla.org/developers/addon/api/key/) right here and keeps it in memory for
# this one command only: never saved, never put in a file.
param([switch]$Listed)
$ErrorActionPreference = 'Stop'
Set-Location $PSScriptRoot

if (-not (Test-Path 'node_modules\.bin\web-ext.cmd')) {
    Write-Host 'Installing web-ext (Mozilla''s tool) ...'
    npm install --no-fund --no-audit
}

$version = (Get-Content 'firefox\manifest.json' -Raw | ConvertFrom-Json).version
$channel = if ($Listed) { 'listed' } else { 'unlisted' }
Write-Host ''
Write-Host "Signing DasIDM for Firefox, version $version ($channel)"
Write-Host '(Mozilla takes each version number once; a later change needs a higher one.)'
Write-Host ''

$issuer = (Read-Host 'JWT issuer (looks like user:12345678:123)').Trim()
$secure = Read-Host 'JWT secret (hidden as you type)' -AsSecureString
if (-not $issuer -or $secure.Length -eq 0) { throw 'Both the JWT issuer and the JWT secret are needed.' }

$webExt = @('web-ext', 'sign', "--channel=$channel", '--source-dir', 'firefox', '--artifacts-dir', 'dist', '--ignore-files', 'rules.test.mjs')
# A listed version waits for Mozilla's reviewers (often days): stop once it is uploaded and
# passed the automatic checks, instead of waiting and timing out.
if ($Listed) { $webExt += @('--amo-metadata', 'amo\metadata.json', '--approval-timeout', '0') }
$before = @(Get-ChildItem 'dist' -Filter '*.xpi' -ErrorAction SilentlyContinue | ForEach-Object FullName)
try {
    $env:WEB_EXT_API_KEY = $issuer
    $env:WEB_EXT_API_SECRET = [System.Net.NetworkCredential]::new('', $secure).Password.Trim()
    # Mozilla checks it automatically first; this usually takes a few minutes.
    npx @webExt
    if ($LASTEXITCODE -ne 0) { throw "Signing failed (web-ext exit code $LASTEXITCODE). See the messages above." }
}
finally {
    Remove-Item Env:WEB_EXT_API_KEY, Env:WEB_EXT_API_SECRET -ErrorAction SilentlyContinue
    $secure.Dispose()
}

Write-Host ''
$new = Get-ChildItem 'dist' -Filter '*.xpi' -ErrorAction SilentlyContinue | Where-Object { $before -notcontains $_.FullName } | Sort-Object LastWriteTime -Descending | Select-Object -First 1
if ($new) {
    Write-Host "Signed: $($new.FullName)" -ForegroundColor Green
    Write-Host 'Install it in Firefox: Add-ons and themes (Ctrl+Shift+A) -> gear icon -> Install Add-on From File... -> pick that file.'
} elseif ($Listed) {
    Write-Host "Submitted version $version to the Firefox Add-ons store." -ForegroundColor Green
    Write-Host 'Mozilla reviews it now (often a few days). Follow it at https://addons.mozilla.org/developers/addons'
    Write-Host 'If the store asks for missing listing details, the text is in extension\amo\listing.md.'
} else {
    Write-Host 'Signed, but the file was not downloaded. Get it from https://addons.mozilla.org/developers/addons' -ForegroundColor Yellow
}
