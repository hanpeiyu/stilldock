[CmdletBinding()]
param(
    [switch]$SkipBuild,
    [string]$NsisPath,
    [string]$WebView2Setup,
    [string]$TargetDirectory = $env:CARGO_TARGET_DIR
)
$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
if ($env:OS -ne 'Windows_NT') { throw 'Packaging requires Windows x64.' }
$repoRoot = Split-Path -Parent $PSScriptRoot
Push-Location $repoRoot
try {
    $manifest = Get-Content -LiteralPath (Join-Path $repoRoot 'Cargo.toml') -Raw
    $versionMatch = [regex]::Match($manifest, '(?m)^version\s*=\s*"([0-9]+\.[0-9]+\.[0-9]+(?:-[A-Za-z0-9.-]+)?)"')
    if (-not $versionMatch.Success) { throw 'Cannot read package version.' }
    $version = $versionMatch.Groups[1].Value
    if (-not $TargetDirectory) { $TargetDirectory = Join-Path $repoRoot 'target' }
    $TargetDirectory = [System.IO.Path]::GetFullPath($TargetDirectory)
    if (-not $SkipBuild) {
        & cargo build --release --locked --target-dir $TargetDirectory
        if ($LASTEXITCODE -ne 0) { throw 'Cargo release build failed.' }
    }
    $exe = Join-Path $TargetDirectory 'release\xhs-browser.exe'
    if (-not (Test-Path -LiteralPath $exe)) { throw "Missing executable: $exe" }
    if (-not $NsisPath) {
        $nsisCommand = Get-Command makensis.exe -ErrorAction SilentlyContinue
        if ($nsisCommand) { $NsisPath = $nsisCommand.Source }
        else {
            $NsisPath = Join-Path ${env:ProgramFiles(x86)} 'NSIS\makensis.exe'
            if (-not (Test-Path -LiteralPath $NsisPath)) { throw 'Install NSIS 3 or pass -NsisPath.' }
        }
    }
    if (-not $WebView2Setup) {
        $vendor = Join-Path $repoRoot 'vendor'
        New-Item -ItemType Directory -Path $vendor -Force | Out-Null
        $WebView2Setup = Join-Path $vendor 'MicrosoftEdgeWebview2Setup.exe'
        if (-not (Test-Path -LiteralPath $WebView2Setup)) {
            Invoke-WebRequest -Uri 'https://go.microsoft.com/fwlink/p/?LinkId=2124703' -OutFile $WebView2Setup -UseBasicParsing
        }
    }
    $WebView2Setup = (Resolve-Path -LiteralPath $WebView2Setup).Path
    $signature = Get-AuthenticodeSignature -LiteralPath $WebView2Setup
    if ($signature.Status -ne 'Valid' -or $signature.SignerCertificate.Subject -notmatch 'O=Microsoft Corporation(?:,|$)') {
        throw 'WebView2 bootstrapper must have a valid Microsoft signature.'
    }
    $dist = Join-Path $repoRoot 'dist'
    New-Item -ItemType Directory -Path $dist -Force | Out-Null
    $portable = Join-Path $dist "StillDock-$version-windows-x64-portable.zip"
    Add-Type -AssemblyName System.IO.Compression
    Add-Type -AssemblyName System.IO.Compression.FileSystem
    $stream = [System.IO.File]::Open($portable, [System.IO.FileMode]::Create)
    try {
        $zip = [System.IO.Compression.ZipArchive]::new($stream, [System.IO.Compression.ZipArchiveMode]::Create)
        try {
            [System.IO.Compression.ZipFileExtensions]::CreateEntryFromFile($zip, $exe, 'xhs-browser.exe') | Out-Null
            foreach ($name in @('README.md', 'LICENSE', 'NOTICE.md')) {
                [System.IO.Compression.ZipFileExtensions]::CreateEntryFromFile($zip, (Join-Path $repoRoot $name), $name) | Out-Null
            }
        }
        finally { $zip.Dispose() }
    }
    finally { $stream.Dispose() }
    & $NsisPath /V2 "/DAPP_VERSION=$version" "/DAPP_EXE=$exe" "/DWEBVIEW2_SETUP=$WebView2Setup" (Join-Path $repoRoot 'installer.nsi')
    if ($LASTEXITCODE -ne 0) { throw 'NSIS packaging failed.' }
    $installer = Join-Path $dist "StillDock-$version-windows-x64-setup.exe"
    if (-not (Test-Path -LiteralPath $installer)) { throw 'Installer output was not created.' }
    $checksums = foreach ($artifact in @($portable, $installer)) {
        $hash = (Get-FileHash -LiteralPath $artifact -Algorithm SHA256).Hash.ToLowerInvariant()
        "$hash  $([System.IO.Path]::GetFileName($artifact))"
    }
    Set-Content -LiteralPath (Join-Path $dist 'SHA256SUMS.txt') -Value $checksums -Encoding Ascii
    Write-Host "Packages ready: $dist"
}
finally { Pop-Location }
