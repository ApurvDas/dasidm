# Packs the Firefox extension into extension\dist\dasidm-firefox.xpi (a zip), for signing on
# addons.mozilla.org (unlisted) or for "Install Add-on From File…". Tests are left out.
$ErrorActionPreference = 'Stop'
$src = Join-Path $PSScriptRoot 'firefox'
$out = Join-Path $PSScriptRoot 'dist'
New-Item -ItemType Directory -Force $out | Out-Null
$xpi = Join-Path $out 'dasidm-firefox.xpi'
if (Test-Path $xpi) { Remove-Item $xpi }
Add-Type -AssemblyName System.IO.Compression.FileSystem
$zip = [System.IO.Compression.ZipFile]::Open($xpi, 'Create')
try {
    Get-ChildItem $src -Recurse -File | Where-Object { $_.Name -notlike '*.test.*' -and -not $_.Name.StartsWith('.') } | ForEach-Object {
        # Zip entries use forward slashes, or Firefox can't find icons/…
        $rel = $_.FullName.Substring($src.Length + 1).Replace('\', '/')
        [void][System.IO.Compression.ZipFileExtensions]::CreateEntryFromFile($zip, $_.FullName, $rel)
    }
} finally {
    $zip.Dispose()
}
"Wrote $xpi"
