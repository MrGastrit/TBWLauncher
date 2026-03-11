param(
  [string]$OutputDir = "release/updates",
  [string]$BaseUrl = "https://vm863690.hosted-by.u1host.com/updates",
  [string]$PlatformKey = "windows-x86_64",
  [string]$Notes = "TBW Launcher update",
  [string]$ArtifactPath = "",
  [string]$SignaturePath = ""
)

$ErrorActionPreference = "Stop"

if (-not ($BaseUrl -match "^https?://")) {
  throw "BaseUrl must start with http:// or https://. Got: $BaseUrl"
}

$repoRoot = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
$tauriDir = Join-Path $repoRoot "src-tauri"
$tauriConfigPath = Join-Path $tauriDir "tauri.conf.json"
$bundleDir = Join-Path $tauriDir "target\release\bundle"
$resolvedOutputDir = (Resolve-Path (New-Item -ItemType Directory -Force -Path $OutputDir)).Path

$tauriConfig = Get-Content -Path $tauriConfigPath -Raw | ConvertFrom-Json
$version = "$($tauriConfig.version)".Trim()
if ([string]::IsNullOrWhiteSpace($version)) {
  throw "Could not resolve version from $tauriConfigPath"
}

function Get-ArtifactExtension([string]$PathValue) {
  $lower = $PathValue.ToLowerInvariant()
  if ($lower.EndsWith(".tar.gz")) {
    return ".tar.gz"
  }
  return [System.IO.Path]::GetExtension($lower)
}

function Get-ExtensionPriority([string]$Extension) {
  switch ($Extension) {
    ".exe" { return 0 }
    ".msi" { return 1 }
    ".appimage" { return 2 }
    ".deb" { return 3 }
    ".rpm" { return 4 }
    ".dmg" { return 5 }
    ".zip" { return 6 }
    ".tar.gz" { return 7 }
    default { return 99 }
  }
}

function Get-InstallerSuffix([string]$Extension) {
  switch ($Extension) {
    ".exe" { return "nsis" }
    ".msi" { return "msi" }
    default { return $null }
  }
}

function Resolve-ArtifactPair {
  param(
    [string]$BundleDirectory,
    [string]$ForcedArtifactPath,
    [string]$ForcedSignaturePath
  )

  if (-not [string]::IsNullOrWhiteSpace($ForcedArtifactPath)) {
    $resolvedArtifactPath = (Resolve-Path $ForcedArtifactPath).Path
    $resolvedSignaturePath = if ([string]::IsNullOrWhiteSpace($ForcedSignaturePath)) {
      "$resolvedArtifactPath.sig"
    } else {
      (Resolve-Path $ForcedSignaturePath).Path
    }

    if (-not (Test-Path $resolvedSignaturePath)) {
      throw "Signature file not found: $resolvedSignaturePath"
    }

    return [pscustomobject]@{
      ArtifactPath   = $resolvedArtifactPath
      SignaturePath  = $resolvedSignaturePath
      Extension      = (Get-ArtifactExtension $resolvedArtifactPath)
      LastWriteTicks = (Get-Item $resolvedArtifactPath).LastWriteTimeUtc.Ticks
      Priority       = (Get-ExtensionPriority (Get-ArtifactExtension $resolvedArtifactPath))
    }
  }

  if (-not (Test-Path $BundleDirectory)) {
    throw "Bundle directory not found: $BundleDirectory. Build first: cargo tauri build"
  }

  $supportedExtensions = @(".exe", ".msi", ".appimage", ".deb", ".rpm", ".dmg", ".zip", ".tar.gz")
  $candidates = @()

  Get-ChildItem -Path $BundleDirectory -Recurse -File -Filter "*.sig" | ForEach-Object {
    $signatureFile = $_.FullName
    $artifactFile = $signatureFile.Substring(0, $signatureFile.Length - 4)
    if (-not (Test-Path $artifactFile)) {
      return
    }

    $extension = Get-ArtifactExtension $artifactFile
    if ($supportedExtensions -notcontains $extension) {
      return
    }

    $artifactItem = Get-Item $artifactFile
    $candidates += [pscustomobject]@{
      ArtifactPath   = $artifactFile
      SignaturePath  = $signatureFile
      Extension      = $extension
      LastWriteTicks = $artifactItem.LastWriteTimeUtc.Ticks
      Priority       = Get-ExtensionPriority $extension
    }
  }

  if ($candidates.Count -eq 0) {
    throw "Could not find signed updater artifact in $BundleDirectory"
  }

  return $candidates |
    Sort-Object -Property @{Expression = "Priority"; Ascending = $true }, @{Expression = "LastWriteTicks"; Descending = $true } |
    Select-Object -First 1
}

$artifactPair = Resolve-ArtifactPair -BundleDirectory $bundleDir -ForcedArtifactPath $ArtifactPath -ForcedSignaturePath $SignaturePath

$artifactName = [System.IO.Path]::GetFileName($artifactPair.ArtifactPath)
$signatureName = [System.IO.Path]::GetFileName($artifactPair.SignaturePath)
$signatureRaw = Get-Content -Path $artifactPair.SignaturePath -Raw
$signature = $signatureRaw.Trim()
if ([string]::IsNullOrWhiteSpace($signature)) {
  throw "Signature file is empty: $($artifactPair.SignaturePath)"
}

$artifactDestinationPath = Join-Path $resolvedOutputDir $artifactName
$signatureDestinationPath = Join-Path $resolvedOutputDir $signatureName

Copy-Item -Path $artifactPair.ArtifactPath -Destination $artifactDestinationPath -Force
Copy-Item -Path $artifactPair.SignaturePath -Destination $signatureDestinationPath -Force

$trimmedBaseUrl = $BaseUrl.TrimEnd("/")
$artifactUrl = "$trimmedBaseUrl/$([uri]::EscapeDataString($artifactName))"
$platformEntry = [ordered]@{
  signature = $signature
  url = $artifactUrl
}

$platforms = [ordered]@{}
$platforms[$PlatformKey] = $platformEntry

$installerSuffix = Get-InstallerSuffix $artifactPair.Extension
if ($installerSuffix -and -not $PlatformKey.EndsWith("-$installerSuffix")) {
  $platforms["$PlatformKey-$installerSuffix"] = $platformEntry
}

$latest = [ordered]@{
  version = $version
  notes = $Notes
  pub_date = [DateTime]::UtcNow.ToString("o")
  platforms = $platforms
}

$latestJsonPath = Join-Path $resolvedOutputDir "latest.json"
$latestJson = $latest | ConvertTo-Json -Depth 16
$utf8NoBom = New-Object System.Text.UTF8Encoding($false)
[System.IO.File]::WriteAllText($latestJsonPath, $latestJson, $utf8NoBom)

Write-Host "Updater release package is ready."
Write-Host "Version: $version"
Write-Host "Artifact: $($artifactPair.ArtifactPath)"
Write-Host "Signature: $($artifactPair.SignaturePath)"
Write-Host "Output directory: $resolvedOutputDir"
Write-Host "latest.json: $latestJsonPath"
Write-Host ""
Write-Host "Next step:"
Write-Host "npm run updater:publish-release"
