param(
  [string]$LocalDir = "release/updates",
  [string]$RemoteHost = "root@144.31.73.2",
  [string]$RemoteDir = "/var/www/tbw-updates"
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

Write-Host "Creating remote directory..."
ssh $RemoteHost "mkdir -p '$RemoteDir'"

Write-Host "Uploading update files..."
foreach ($file in $files) {
  $remotePath = "$RemoteHost`:$RemoteDir/$($file.Name)"
  scp $file.FullName $remotePath
}

Write-Host "Remote directory listing:"
ssh $RemoteHost "ls -lah '$RemoteDir'"

Write-Host "Done. Verify from VPS:"
Write-Host "curl -I https://vm863690.hosted-by.u1host.com/updates/latest.json"
