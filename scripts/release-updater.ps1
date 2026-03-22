param(
  [string]$PrivateKeyPath = "",
  [string]$PrivateKeyPassword = "",
  [string]$BaseUrl = "http://144.31.73.2/updates",
  [string]$PlatformKey = "windows-x86_64",
  [string]$Notes = "TBW Launcher update",
  [string]$RemoteHost = "root@144.31.73.2",
  [string]$RemoteDir = "/var/www/tbw-updates",
  [switch]$SkipBuild,
  [switch]$Interactive
)

$ErrorActionPreference = "Stop"

$repoRoot = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
$defaultPrivateKeyPath = Join-Path $env:USERPROFILE ".tauri\tbw-updater.key"

if ([string]::IsNullOrWhiteSpace($PrivateKeyPath)) {
  if (-not [string]::IsNullOrWhiteSpace($env:TAURI_PRIVATE_KEY_PATH)) {
    $PrivateKeyPath = $env:TAURI_PRIVATE_KEY_PATH
  } else {
    $PrivateKeyPath = $defaultPrivateKeyPath
  }
}

if ([string]::IsNullOrWhiteSpace($PrivateKeyPassword)) {
  if (-not [string]::IsNullOrWhiteSpace($env:TAURI_PRIVATE_KEY_PASSWORD)) {
    $PrivateKeyPassword = $env:TAURI_PRIVATE_KEY_PASSWORD
  } elseif (-not [string]::IsNullOrWhiteSpace($env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD)) {
    $PrivateKeyPassword = $env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD
  }
}

if (-not $SkipBuild) {
  $resolvedPrivateKeyPath = (Resolve-Path $PrivateKeyPath -ErrorAction Stop).Path
  if ([string]::IsNullOrWhiteSpace($PrivateKeyPassword)) {
    throw "Private key password is required. Pass -PrivateKeyPassword or set TAURI_PRIVATE_KEY_PASSWORD."
  }

  $privateKey = (Get-Content -LiteralPath $resolvedPrivateKeyPath -Raw).Trim()
  if ([string]::IsNullOrWhiteSpace($privateKey)) {
    throw "Private key file is empty: $resolvedPrivateKeyPath"
  }

  $env:TAURI_SIGNING_PRIVATE_KEY = $privateKey
  $env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD = $PrivateKeyPassword

  Write-Host "Building signed NSIS installer..."
  Push-Location (Join-Path $repoRoot "src-tauri")
  try {
    & cargo tauri build --bundles nsis
  } finally {
    Pop-Location
  }
}

Write-Host "Preparing updater release files..."
& (Join-Path $PSScriptRoot "prepare-updater-release.ps1") `
  -BaseUrl $BaseUrl `
  -PlatformKey $PlatformKey `
  -Notes $Notes

Write-Host "Publishing updater files to VPS..."
if ($Interactive) {
  & (Join-Path $PSScriptRoot "publish-updates.ps1") `
    -RemoteHost $RemoteHost `
    -RemoteDir $RemoteDir `
    -Interactive
} else {
  & (Join-Path $PSScriptRoot "publish-updates.ps1") `
    -RemoteHost $RemoteHost `
    -RemoteDir $RemoteDir
}

Write-Host ""
Write-Host "Done. The launcher can now fetch this update from:"
Write-Host "$BaseUrl/latest.json"
