# Signs DasIDM's Firefox extension as a private (unlisted) add-on on addons.mozilla.org, so it
# stays installed in regular Firefox. Nobody else can find or install it.
#
# Run it yourself in a terminal:
#   powershell -ExecutionPolicy Bypass -File C:\level-2\DasIDM\extension\sign.ps1
# It asks for your API key (https://addons.mozilla.org/developers/addon/api/key/) right here
# and keeps it in memory for this one command only: never saved, never put in a file.
$ErrorActionPreference = 'Stop'
Set-Location $PSScriptRoot

if (-not (Test-Path 'node_modules\.bin\web-ext.cmd')) {
    Write-Host 'Installing web-ext (Mozilla''s tool) ...'
    npm install --no-fund --no-audit
}

$version = (Get-Content 'firefox\manifest.json' -Raw | ConvertFrom-Json).version
Write-Host ''
Write-Host "Signing DasIDM for Firefox, version $version"
Write-Host '(Mozilla takes each version number once; a later change needs a higher one.)'
Write-Host ''

$issuer = (Read-Host 'JWT issuer (looks like user:12345678:123)').Trim()
$secure = Read-Host 'JWT secret (hidden as you type)' -AsSecureString
if (-not $issuer -or $secure.Length -eq 0) { throw 'Both the JWT issuer and the JWT secret are needed.' }

try {
    $env:WEB_EXT_API_KEY = $issuer
    $env:WEB_EXT_API_SECRET = [System.Net.NetworkCredential]::new('', $secure).Password.Trim()
    # Mozilla checks it automatically; this usually takes a few minutes.
    npx web-ext sign --channel=unlisted --source-dir firefox --artifacts-dir dist --ignore-files rules.test.mjs
    if ($LASTEXITCODE -ne 0) { throw "Signing failed (web-ext exit code $LASTEXITCODE). See the messages above." }
}
finally {
    Remove-Item Env:WEB_EXT_API_KEY, Env:WEB_EXT_API_SECRET -ErrorAction SilentlyContinue
    $secure.Dispose()
}

$xpi = Get-ChildItem 'dist' -Filter '*.xpi' | Sort-Object LastWriteTime -Descending | Select-Object -First 1
Write-Host ''
Write-Host "Signed: $($xpi.FullName)" -ForegroundColor Green
Write-Host 'Install it in Firefox: Add-ons and themes (Ctrl+Shift+A) -> gear icon -> Install Add-on From File... -> pick that file.'
Write-Host 'If the temporary copy is still loaded, remove it in about:debugging first.'
