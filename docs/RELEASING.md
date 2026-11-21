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
2. The installed app puts the addon in place itself (see "Updates inside the app"). To do it by hand, unzip
   `Scarlet-<version>.zip` into `<WoW folder>\_forever_\Interface\AddOns`. The zip holds one `Scarlet`
   folder; its `.toc` carries `<version>`. The app writes the digest into its own `ScarletData` folder, which the zip does not hold.
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

## Publish the addon to CurseForge and Wago

```
pwsh scripts/publish-stores.ps1 -Version 1.0.0
pwsh scripts/publish-stores.ps1 -Version 1.0.0 -DryRun
```

`scripts/publish-stores.ps1` uploads the addon zip of an already published release. The release lane runs it right
after publishing (`-Stores auto`); a beta is uploaded only with `-Stores yes`, `-Stores no` skips it. A failed upload
leaves the GitHub release published and the lane ends with the command to rerun. The same command also works alone
for any published version.

- The zip is downloaded from the release and checked against `SHA256SUMS.txt`, so the stores get the file the app
  installs. Release type: `v<x>` is a release (Wago: stable), `v<x>-beta.N` a beta.
- Game version: the zip's `## Interface` (16001 is Forever 1.60.1). CurseForge's id for it is looked up with the
  token (`/api/game/wow/versions`, type 88568 = Forever) and Wago's patch is checked against
  `https://addons.wago.io/api/data/game`.
- Changelog: the "Addon changes" section of the release notes, without the commit hashes. `-Changelog <file>` uploads
  that file's text instead.
- Project IDs come from `addon/Scarlet/Scarlet.toc` on `main` of `Medalink/scarlet` (`## X-Curse-Project-ID:` and
  `## X-Wago-ID:`; both lines are commented out until the projects exist). `-CurseProjectId` and `-WagoId` override
  them. The zip itself never carries them.
- Tokens: `CF_API_KEY` and `WAGO_API_TOKEN`, or one-line files `E:\Dev\secrets\scarlet-addon-stores\curseforge-token`
  and `wago-token`. They are never printed or logged.
- A store without an ID or a token is skipped with one line. A store that already got the version from this PC is
  skipped too: neither store has an API that lists files, so the record is `<Root>\stores.json`. `-Force` uploads again.
- `-DryRun` prints the URL and the exact metadata JSON per store and uploads nothing. Without a CurseForge token,
  `gameVersions` stays empty in the output. `-Store curseforge` or `-Store wago` limits it to one store.

## Versions

Versions come from tags only. No committed file holds one:

- The app version is passed to `cargo tauri build --config` as `{"version": "<version>"}` at build time. The
  `0.0.0-dev` in `app/tauri.conf.json` is a placeholder: a build that keeps it shows `dev` as its version and never
  checks for updates. (`app/Cargo.toml` keeps `0.1.0`, which nothing reads.)
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
| `Scarlet-<version>.zip` | The addon: a `Scarlet` folder to unzip into `AddOns`, version stamped in the TOC. No `ScarletData` folder: the app writes it. |
| `SHA256SUMS.txt` | SHA-256 of the two files above. |
| `latest.json` | The update manifest: the version, the notes, the installer's updater signature and address, and the addon zip with its signature. The app reads it. |

The release notes list the app and addon commits (`App commit:` and `Addon commit:`; the release lane reads these
back, so do not edit them) and the commit titles since the previous release.

## Where it runs

Build output goes to `E:\Dev\caches\scarlet\target-release`, working copies to `E:\Dev\scratch\scarlet-release`.
Tauri downloads NSIS into `%LOCALAPPDATA%\tauri`, which on this PC is a junction to `E:\Dev\caches\tauri`
(`evict-c.ps1 apply -Only tauri` creates it); the script refuses to build when it is not, unless
`-AllowToolCacheOnC` is passed.

A release is created as a draft, filled with its assets, checked, and only then published, so a failed upload
leaves a draft and no half release.

## Updates inside the app

The app updates itself from the releases of `Medalink/scarlet-app` and keeps the addon current. Settings shows the
version (`dev` for a build nobody released), a Release or Beta choice, the update on offer with its notes, recent
releases, and the state of the addon.

### Feeds

Each track reads one `latest.json`:

| Track | Feed |
| --- | --- |
| Release | `https://github.com/Medalink/scarlet-app/releases/latest/download/latest.json`, the asset of the latest release. It is a 404 until the first release exists; the app reads that as "nothing published". |
| Beta | `https://github.com/Medalink/scarlet-app/releases/download/beta-channel/latest.json`, the asset of the fixed `beta-channel` prerelease. |

`release.ps1` attaches `latest.json` to every release it publishes. After publishing it also puts the manifest in the
`beta-channel` release (creating it the first time), unless that release already names a newer version. So the feed
of the Beta track names the newest of the betas and the releases, and a release outranks the betas before it.
Nobody edits `beta-channel` by hand. The app updates only to a newer version: a beta user who switches to Release
stays on the beta until the next release and is told so in Settings.

The notes in the manifest and in the app are the release body without the install and commit lines. The script stops
when the notes hold something shaped like a character name (Name-Surname) or a name from the data folder, so a name
in a commit title never reaches a release. `-AllowNotesText "Some-Words"` lets a harmless match through.

Betas before the updater (`v0.4.0-beta.1` and `-beta.2`) cannot be promoted: their `tauri.conf.json` has no public
key, so the script stops before building. Promote `v0.4.0-beta.3` or later.

If the script fails after publishing, at the `beta-channel` step, the release itself is complete. Finish the feed by
hand with `gh release upload beta-channel <Root>\<version>\latest.json --clobber -R Medalink/scarlet-app`
(`gh release create beta-channel` with the same file when the release does not exist yet; see `Update-BetaFeed`).

### The updater key

Installers and the addon zip are signed with a minisign key made by `cargo tauri signer generate`. The public key is
in `app/tauri.conf.json` (`plugins.updater.pubkey`); the app refuses anything the key did not sign.

- The private key is `E:\Dev\secrets\scarlet-updater\scarlet-updater.key`, outside every repository, with no
  password. `release.ps1` finds it there, or at `-UpdaterKey` / `SCARLET_UPDATER_KEY`. It never goes in a repository,
  a log or a chat.
- Back it up. A build signed with another key is rejected by every installed copy, so losing the key means every
  user installs the next version by hand.
- `release.ps1` signs after the Authenticode signature, because the updater signature covers the installer's exact
  bytes. It checks the key against the public key in the app before the long build, and stops when they differ.
- The first build with the updater in it cannot be reached by an update: install it by hand. After that, installs
  update themselves.

### Addon in the WoW folder

When the app is a released build and the switch "Keep the addon up to date" is on, it reads the addon zip named by
the feed (checking its signature against the same key, and that the signature is for that zip) and writes into
`<WoW folder>\_forever_\Interface\AddOns`:

- A folder that is missing or empty is installed. The app writes `.scarlet-app.json` in it, listing the files it wrote.
- A folder with that file is updated: the zip's files are written and only files the app wrote earlier and the zip no
  longer has are removed. `ScarletData`, which the companion writes (`ScarletData.toc`, `Digest.lua`, `Items.lua`), is never in either list, never offered by the feed and never touched here.
- Everything else is left alone and Settings says so: a folder without that file (installed by hand), a TOC with
  `X-Curse-Project-ID` (CurseForge) or `X-Wago-ID` (Wago), a folder holding `.git`, and a junction or symlink,
  the folder itself or anything in it.
- Settings > Addon shows each folder's installed version, the latest version on the chosen track and who keeps it
  current. A folder that is missing, installed by Scarlet or installed by hand has buttons: Install, `Update to x.y.z`
  (when the latest is newer) and Reinstall, which fetch the track's zip the same checked way and write it without the
  "Keep the addon up to date" switch. A hand-installed folder asks first, since Scarlet manages it from then on.
  The other folders have no buttons and are never written, whatever the command is asked.
- The zip `release.ps1` builds never carries `X-Curse-Project-ID` or `X-Wago-ID`, so a folder the app installed is
  never taken for CurseForge's or Wago's. When the listings exist, their apps and this one share the folder by that rule.

### What the installer does to other copies

Tauri's NSIS installer closes every process named `scarlet-app.exe` of the current user before it replaces the exe,
the background `--on-wow` watcher and a dev copy of the same name included. A running dev build is closed by an
update of an installed one.

### Trying the updater

`release.ps1 -DryRun -TestConfig <json> -TestUrlBase <url>` builds a test copy with its own name and identifier (so it
never touches a real install), signs it the real way and writes a `latest.json` that points at `<url>`. Serve the
output folder, set `SCARLET_UPDATE_BASE` (the app reads `<base>/<release|beta>/latest.json` and `<base>/releases.json`),
and install the copy with `/D=<folder>`. `--home <folder>` does what `SCARLET_HOME` does and survives the installer's
restart, which keeps the environment out. The test config must also set `plugins.updater.dangerousInsecureTransportProtocol`
for a plain-http server. Neither flag works without `-DryRun`.

## Settings on an installed app

The installed app keeps `settings.json` in `%APPDATA%\app.scarlet.desktop`. On first run it looks for the WoW
folder (Battle.net's registered install location, then the usual folders on every fixed drive) and uses
`Documents\Scarlet\out` for its data. A copy with a `settings.json` next to the exe keeps using that file and its
old rules (data off C:). Setting `SCARLET_HOME` to a folder keeps settings, the WebView2 profile, temp and the
default data folder inside it, and gives that copy its own single-window lock, so it runs beside the Scarlet you
already have open. `track` (`release` or `beta`; unset follows the installed build) and `keep_addon` are the
update settings.
