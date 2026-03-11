param(
  [Parameter(Mandatory = $true)]
  [string]$PublicKeyPath
)

$ErrorActionPreference = "Stop"

$resolvedPublicKeyPath = (Resolve-Path $PublicKeyPath).Path
$publicKeyRaw = Get-Content -Path $resolvedPublicKeyPath -Raw
$publicKey = $publicKeyRaw.Trim()

if ([string]::IsNullOrWhiteSpace($publicKey)) {
  throw "Public key file is empty: $resolvedPublicKeyPath"
}

$repoRoot = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
$tauriConfigPath = Join-Path $repoRoot "src-tauri\tauri.conf.json"

$config = Get-Content -Path $tauriConfigPath -Raw | ConvertFrom-Json

if (-not $config.plugins) {
  $config | Add-Member -MemberType NoteProperty -Name plugins -Value ([pscustomobject]@{})
}

if (-not $config.plugins.updater) {
  $config.plugins | Add-Member -MemberType NoteProperty -Name updater -Value ([pscustomobject]@{})
}

$config.plugins.updater.pubkey = $publicKey

$configJson = $config | ConvertTo-Json -Depth 32
$utf8NoBom = New-Object System.Text.UTF8Encoding($false)
[System.IO.File]::WriteAllText($tauriConfigPath, $configJson, $utf8NoBom)

Write-Host "Updater public key applied."
Write-Host "Public key file: $resolvedPublicKeyPath"
Write-Host "Updated: $tauriConfigPath"
