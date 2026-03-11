# Tauri Updater Guide (Windows)

This project is configured for Tauri v2 updater.  
Endpoint is set to:

- `https://vm863690.hosted-by.u1host.com/updates/latest.json`

## 1) Generate updater keys (one time)

Run in `src-tauri`:

```powershell
cd src-tauri
cargo tauri signer generate -w "$env:USERPROFILE\\.tauri\\tbw-updater.key"
```

This creates:

- Private key: `C:\Users\<you>\.tauri\tbw-updater.key`
- Public key: `C:\Users\<you>\.tauri\tbw-updater.key.pub`

Keep private key secret. Never upload it.

## 2) Apply public key to project config

Run in project root:

```powershell
npm run updater:apply-pubkey -- "$env:USERPROFILE\\.tauri\\tbw-updater.key.pub"
```

This updates `src-tauri/tauri.conf.json` (`plugins.updater.pubkey`).

## 3) Build signed release

Before build, set signing env vars in current PowerShell:

```powershell
$env:TAURI_SIGNING_PRIVATE_KEY_PATH="$env:USERPROFILE\\.tauri\\tbw-updater.key"
$env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD="<your-key-password>"
```

Then build:

```powershell
cd src-tauri
cargo tauri build
```

## 4) Prepare `latest.json` + updater files

From project root:

```powershell
npm run updater:prepare-release
```

Generated folder:

- `release/updates/latest.json`
- signed installer artifact (`.exe` or `.msi`)
- matching `.sig`

Optional env overrides:

- `UPDATES_BASE_URL`
- `UPDATES_OUTPUT_DIR`
- `UPDATES_PLATFORM_KEY`
- `UPDATES_NOTES`
- `UPDATES_ARTIFACT_PATH`
- `UPDATES_SIGNATURE_PATH`

## 5) Upload to VPS (`/updates`)

From project root:

```powershell
npm run updater:publish-release
```

Default target:

- Host: `root@144.31.73.2`
- Dir: `/var/www/tbw-updates`

One-command shortcut (prepare + upload):

```powershell
npm run updater:ship-release
```

## 6) Verify

On VPS:

```bash
curl -I https://vm863690.hosted-by.u1host.com/updates/latest.json
```

If you get `200 OK`, updater endpoint is reachable.
