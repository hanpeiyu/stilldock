# Development/CI prerequisite for the hidden WebView2 smoke fixture.
$ErrorActionPreference = 'Stop'
$client = '{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}'
foreach ($root in @('HKLM:\SOFTWARE\WOW6432Node', 'HKCU:\SOFTWARE', 'HKLM:\SOFTWARE')) {
    $version = Get-ItemPropertyValue -LiteralPath "$root\Microsoft\EdgeUpdate\Clients\$client" -Name pv -ErrorAction SilentlyContinue
    if ($version -and $version -ne '0.0.0.0') { Write-Host 'WebView2 Runtime is available'; return }
}
$cache = Join-Path (Split-Path -Parent $PSScriptRoot) 'target\test-tools'
New-Item -ItemType Directory -Path $cache -Force | Out-Null
$bootstrap = Join-Path $cache 'MicrosoftEdgeWebview2Setup.exe'
Invoke-WebRequest -Uri 'https://go.microsoft.com/fwlink/p/?LinkId=2124703' -OutFile $bootstrap -UseBasicParsing
$signature = Get-AuthenticodeSignature -LiteralPath $bootstrap
if ($signature.Status -ne 'Valid' -or $signature.SignerCertificate.Subject -notmatch 'O=Microsoft Corporation(?:,|$)') { throw 'Invalid Microsoft installer signature' }
$installer = Start-Process -FilePath $bootstrap -ArgumentList '/silent /install' -WindowStyle Hidden -Wait -PassThru
if ($installer.ExitCode -notin @(0, 3010)) { throw "WebView2 installation failed: $($installer.ExitCode)" }
