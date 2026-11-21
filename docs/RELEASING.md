# Releasing Scarlet

`scripts/release.ps1` (PowerShell 7) builds the Windows app and the addon zip, signs the app, and publishes them
as a GitHub release on `Medalink/scarlet-app`. It has two lanes: a beta you can test, and a release that is the
same two commits promoted.

## Cut a beta

```
pwsh scripts/release.ps1 -Lane beta
```

- Takes the current `main` of `Medalink/scarlet-app` and `Medalink/scarlet`. Ship what you want in the beta first.
- Names it `<next>-beta.<N>`. `<next>` is the latest stable tag plus one minor (the first is `0.4.0`, then
  `0.5.0`); `N` is one more than the highest beta number already used for it. `-Version 0.4.1` sets `<next>` by
  hand.
- Publishes a prerelease tagged `v<version>` on the app commit. It never becomes the "latest" release.
- `-DryRun` builds and signs into `E:\Dev\scratch\scarlet-release\<version>` and publishes nothing.

## Test it

1. Download `Scarlet_<version>_x64-setup.exe` from the release page and run it. It installs for your user only and
   needs no administrator rights.
2. Unzip `Scarlet-<version>.zip` into `<WoW folder>\_forever_\Interface\AddOns`. The zip holds one `Scarlet`
   folder; its `.toc` carries `<version>`. `Digest.lua` is left out: the app writes it.
3. Open Settings in the app. It lists what it could not find in plain words (the WoW folder, the addon, a first
   recording).

## Promote it

```
pwsh scripts/release.ps1 -Lane release
```

Promotes the newest beta of the next version. To promote another one: `-Lane release -From v0.4.0-beta.2`.

It rebuilds the same two commits the beta was built from (read from the beta's notes, and checked against its
tag) at version `<x>` and publishes `v<x>` as the latest release. The binary is rebuilt, not copied, so it carries
`<x>` and a fresh signature. It stops if `v<x>` already exists.

## Versions

Versions come from tags only. No committed file holds one:

- The app version is passed to `cargo tauri build --config` as `{"version": "<version>"}` at build time. The
  `0.1.0` in `app/tauri.conf.json` and `app/Cargo.toml` is a placeholder.
- The addon version is written into the `## Version:` line of the TOC inside the zip. The TOC in the repo is not
  changed.

## Signing

The installer and `scarlet-app.exe` are signed with the MedaNet LLC Azure Artifact Signing profile that signs the
owner's other products.

- `-SignCommand` (default: `C:\Users\medal\.whisper-release-keys\rewrite\windows\sign-command.json`, or the
  `SCARLET_SIGN_COMMAND` environment variable) names the tool: `signtool.exe` with the Azure Artifact Signing
  dlib and its metadata file, and the timestamp server `http://timestamp.acs.microsoft.com`. The file holds paths
  only. The script passes it to Tauri as `bundle.windows.signCommand`.
- Authentication is the cached `az login` of the account that owns the signing profile. The script checks the
  token before it builds and stops with the reason when it is missing or about to expire. Run `az login` and
  rerun.
- Before upload, the script requires `Get-AuthenticodeSignature` to report `Valid` with a timestamp for the
  installer and the app exe. After publishing it downloads the installer from GitHub, checks the SHA-256 sums and
  the signature again, and turns the release back into a draft if anything differs.

## Assets

| Asset | What it is |
| --- | --- |
| `Scarlet_<version>_x64-setup.exe` | The signed NSIS installer for the app (per user). MSI is not built: it rejects text prerelease ids such as `-beta.1`. |
| `Scarlet-<version>.zip` | The addon: a `Scarlet` folder to unzip into `AddOns`, version stamped in the TOC, no `Digest.lua`. |
| `SHA256SUMS.txt` | SHA-256 of the two files above. |

The release notes list the app and addon commits (`App commit:` and `Addon commit:`; the release lane reads these
back, so do not edit them) and the commit titles since the previous release.

## Where it runs

Build output goes to `E:\Dev\caches\scarlet\target-release`, working copies to `E:\Dev\scratch\scarlet-release`.
Tauri downloads NSIS into `%LOCALAPPDATA%\tauri`, which on this PC is a junction to `E:\Dev\caches\tauri`
(`evict-c.ps1 apply -Only tauri` creates it); the script refuses to build when it is not, unless
`-AllowToolCacheOnC` is passed.

A release is created as a draft, filled with its assets, checked, and only then published, so a failed upload
leaves a draft and no half release.

## Settings on an installed app

The installed app keeps `settings.json` in `%APPDATA%\app.scarlet.desktop`. On first run it looks for the WoW
folder (Battle.net's registered install location, then the usual folders on every fixed drive) and uses
`Documents\Scarlet\out` for its data. A copy with a `settings.json` next to the exe keeps using that file and its
old rules (data off C:). Setting `SCARLET_HOME` to a folder keeps settings, the WebView2 profile, temp and the
default data folder inside it, and gives that copy its own single-window lock, so it runs beside the Scarlet you
already have open.
