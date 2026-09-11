# AGENTS.md

## Project purpose

An [OpenAction](https://openaction.amankhanna.me) plugin for [OpenDeck](https://github.com/nekename/OpenDeck) that shows [OpenRouter](https://openrouter.ai) remaining credits on a stream-deck-style key.

The action queries the OpenRouter credits API, computes `total_credits - total_usage`, and renders the balance as USD currency (for example `$74.75`).

## Architecture

- **Rust backend** (`src/main.rs`) using the official `openaction` crate (2.x). This is the recommended OpenAction language. Do not rewrite in another language unless asked.
- **Single action** with UUID `com.imdevinc.openroutercredits.credits`, defined in `assets/manifest.json`.
- **Property inspector** (`assets/pi.html`): a webview that stores the `api_key` in the action's settings via the OpenAction WebSocket (`setSettings` event). The backend reads it in `will_appear` / `did_receive_settings`.
- **Rendering**: the backend builds an SVG string and sends it to the key with `instance.set_image(...)` as a base64 data URI (`data:image/svg+xml;base64,...`). OpenDeck rasterises SVG at 144x144 in its frontend canvas, so SVG works on hardware. The manifest sets `ShowTitle: false` because the SVG carries all text.
- **Manifest naming**: the plugin's manifest `Name` is `OpenRouter Credits` (shown in OpenDeck's Plugin Manager and the marketplace catalogue; the catalogue `name` must match it exactly). The action's `Name` is `Credits` under the `OpenRouter` category. `CategoryIcon` (`assets/openrouter.svg`) is the category icon. Keep the stage/install copies in sync if assets change.
- **Data source**: `GET https://openrouter.ai/api/v1/credits` with header `Authorization: Bearer <api_key>`. A management key is required. Response has `data.{total_credits,total_usage}`.

## How it stays live

- A `tokio::spawn` background task ticks every 60s (`tick_instances`).
- It refetches from the API at most once per 5 minutes (per instance, tracked by `last_fetch: Instant`). Instances that are not stale are skipped.
- Pressing the key forces an immediate refetch + render.
- `set_credit_image` stores fresh fetch results back into the shared `INSTANCES` map so the ticker does not double-fetch.

## Key code conventions and gotchas

- **State tracking**: `INSTANCES` is a `static LazyLock<Mutex<HashMap<InstanceId, InstanceState>>>` keyed by `instance.instance_id`, updated in `will_appear` / `will_disappear` / `did_receive_settings`. The crate's own `Instance.settings_json` is `pub(crate)` and not readable from the plugin, so this map is how the ticker knows each instance's settings.
- **Do not hold the `INSTANCES` lock across `.await`**: `std::sync::MutexGuard` is not `Send`, so a guard held across an await fails to compile in spawned tasks and `async_trait` handlers. Snapshot (clone) state inside a scope, then await outside it.
- **SVG format strings**: use `r##"..."##` raw strings, never `r#"..."#`. The hex colors (`fill="#ffffff"`) contain the byte sequence `"#` which prematurely terminates `r#"..."#` and causes confusing `prefix ... is unknown` compile errors.
- **Truncate error text char-safely**: use `message.chars().take(16).collect()`, not `&message[..16]` (panics on multi-byte boundaries).
- **API JSON is snake_case**: the credits endpoint sends `total_credits` and `total_usage`, matching the Rust field names. No `rename_all` is needed here.
- **Pure helpers are testable**: `remaining_credits`, `format_currency`, `render_svg*`, `to_data_uri` are side-effect free. Add unit tests for any changes to them.
- Tabs for indentation (matches the `openaction` crate examples and this repo).

## Commands

```sh
cargo build --release     # build
cargo test --release      # unit tests (calculation/formatting/render logic)
cargo clippy --release    # lint; must be clean before finishing
make stage                # assemble com.imdevinc.openroutercredits.sdPlugin/
make package              # stage + zip com.imdevinc.openroutercredits.zip
make install              # stage + copy into OpenDeck plugins dir
make clean                # remove build artifacts, staged plugin dir, and package zip
```

OpenDeck plugins directory: Linux `~/.config/opendeck/plugins/` (or `$XDG_CONFIG_HOME/opendeck/plugins/`), macOS `~/Library/Application Support/OpenDeck/plugins/`.

## Distribution notes

- Manifest `CodePaths`/`CodePathWin`/`CodePathMac`/`CodePathLin` list the five standard platform triples (`x86_64`/`aarch64` × windows/mac/linux). The binary is named `oaopenrouter-credits-<target-triple>`.
- **Releases are PR-label driven** (modeled on `pd-slack`). Every PR must carry exactly one of the `major`/`minor`/`patch` labels; `.github/workflows/pr.yaml` validates this via the shared reusable `imdevinc/imdevinc/.github/workflows/shared-validate-semver-tags.yaml@v1`. Enforce it with branch protection requiring that check.
- On merge to `main`, `.github/workflows/build.yml` runs: `jefflinse/pr-semver-bump` (bump mode) derives the next version from the merged PR's label and tags it; the five triples are built natively; the bundle is assembled into `com.imdevinc.openroutercredits.zip`. CI then writes the new version back into `Cargo.toml` and `assets/manifest.json` (committed to `main` as `github-actions[bot]`), force-updates the `vX.Y.Z` tag, and creates the GitHub Release with the zip. A merge without a release label fails the workflow and produces no release.
- Release tags must match the manifest `Version` (OpenDeck strips a leading `v` and compares). `Cargo.toml`/`assets/manifest.json` hold the seed version (`0.0.1`); after the first release the tag is the source of truth and CI keeps the files in sync.
- The release asset is named `<bundle-id>.zip` (`com.imdevinc.openroutercredits.zip`), matching the convention used by other native plugins.
- The plugin is not yet listed on the OpenAction Marketplace. When submitting, the catalogue entry key is the bundle id `com.imdevinc.openroutercredits` in the "Native OpenAction plugins" section, with `name` = manifest `Name`, `author` = `ImDevinC`, and `repository` = `https://github.com/ImDevinC/opendeck-openrouter-usage`.

## Docs maintenance

Keep `AGENTS.md` and `README.md` accurate and up to date automatically. When you change behavior, commands, dependencies, settings, or architecture, update the relevant section of both files in the same change. Do not wait to be asked.
