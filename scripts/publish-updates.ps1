param(
  [string]$LocalDir = "release/updates",
  [string]$RemoteHost = "root@144.31.73.2",
  [string]$RemoteDir = "/var/www/tbw-updates",
  [switch]$Interactive
)

$ErrorActionPreference = "Stop"

if (-not (Get-Command scp -ErrorAction SilentlyContinue)) {
  throw "scp is not available. Install OpenSSH Client on Windows first."
}

if (-not (Get-Command ssh -ErrorAction SilentlyContinue)) {
  throw "ssh is not available. Install OpenSSH Client on Windows first."
}

$resolvedLocalDir = Resolve-Path $LocalDir -ErrorAction Stop
$latestJsonPath = Join-Path $resolvedLocalDir "latest.json"

if (-not (Test-Path $latestJsonPath)) {
  throw "latest.json not found at: $latestJsonPath. Run npm run updater:prepare-release first."
}

$files = Get-ChildItem -Path $resolvedLocalDir -File
if ($files.Count -eq 0) {
  throw "No files found in $resolvedLocalDir"
}

$sshOptions = @("-o", "ConnectTimeout=10", "-o", "StrictHostKeyChecking=accept-new")
if (-not $Interactive) {
  $sshOptions = @("-o", "BatchMode=yes") + $sshOptions
}

Write-Host "Creating remote directory..."
& ssh @sshOptions $RemoteHost "mkdir -p '$RemoteDir'"
if ($LASTEXITCODE -ne 0) {
  throw "Failed to create remote directory $RemoteDir on $RemoteHost (ssh exit code $LASTEXITCODE)."
}

Write-Host "Uploading update files..."
foreach ($file in $files) {
  $remotePath = "$RemoteHost`:$RemoteDir/$($file.Name)"
  & scp @sshOptions $file.FullName $remotePath
  if ($LASTEXITCODE -ne 0) {
    throw "Failed to upload $($file.Name) to $remotePath (scp exit code $LASTEXITCODE)."
  }
}

Write-Host "Remote directory listing:"
& ssh @sshOptions $RemoteHost "ls -lah '$RemoteDir'"
if ($LASTEXITCODE -ne 0) {
  throw "Failed to read remote directory listing for $RemoteDir on $RemoteHost (ssh exit code $LASTEXITCODE)."
}

Write-Host "Done. Verify from VPS:"
Write-Host "curl -I http://144.31.73.2/updates/latest.json"
