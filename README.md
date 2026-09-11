# OpenRouter Credits OpenDeck plugin

An [OpenAction](https://openaction.amankhanna.me) plugin for [OpenDeck](https://github.com/nekename/OpenDeck) that shows your remaining [OpenRouter](https://openrouter.ai) credits. The plugin is listed as **OpenRouter Credits** in OpenDeck; its single action is **Credits**, under the `OpenRouter` category. It is published in the [OpenAction Marketplace](https://openaction.amankhanna.me) under the bundle id `com.imdevinc.openroutercredits`.

The action queries the OpenRouter credits API and shows the remaining balance as currency:

- Sends `GET https://openrouter.ai/api/v1/credits` with `Authorization: Bearer <api_key>`.
- Remaining balance is `total_credits - total_usage`.
- The key shows the result as USD, for example `$74.75`.

## Requirements

- Rust (stable) to build.
- An OpenRouter **management** API key. The regular inference key does not work for the credits endpoint.

## Build & install

```sh
make install
```

This builds the plugin, stages it into `com.imdevinc.openroutercredits.sdPlugin/`, and copies it into your OpenDeck plugins directory:

- Linux: `~/.config/opendeck/plugins/` (or `$XDG_CONFIG_HOME/opendeck/plugins/`)
- macOS: `~/Library/Application Support/OpenDeck/plugins/`

Restart OpenDeck (or reload plugins) and add the **Credits** action from the `OpenRouter` category.

### Manual install

Run `make stage` to produce `com.imdevinc.openroutercredits.sdPlugin/`, then copy that folder into your OpenDeck plugins directory (found via **Open config directory** in OpenDeck settings → `plugins/`).

## Packaging & releases

Run `make package` to assemble the plugin bundle and zip it into `com.imdevinc.openroutercredits.zip`. The archive contains `com.imdevinc.openroutercredits.sdPlugin/` and can be installed in OpenDeck via **Install from file**.

Releases are driven by PR labels. Every pull request to `main` must carry exactly one of the `major`, `minor`, or `patch` labels (validated by `.github/workflows/pr.yaml`). On merge, `.github/workflows/build.yml` bumps the version from that label, builds all five platform binaries (Windows, macOS, Linux; x86_64 + arm64), assembles the bundle, and publishes a GitHub Release with `com.imdevinc.openroutercredits.zip` attached. CI writes the new version back into `Cargo.toml` and `assets/manifest.json` so they stay in sync with the release tag.

The plugin is registered in the [OpenAction plugin catalogue](https://github.com/OpenActionAPI/plugins) (`com.imdevinc.openroutercredits` in the Native OpenAction plugins section). OpenDeck's marketplace resolves the plugin from this repo's GitHub Releases, so keep the catalogue entry in sync with any manifest changes.

## Configuration

Select the action on your deck, then in the property inspector paste your OpenRouter API key. The plugin adds the `Bearer ` prefix. The key is stored in the action's settings only.

The balance refreshes from `https://openrouter.ai/api/v1/credits` at most once every 5 minutes. Press the key to force an immediate refresh.

## Development

```sh
cargo test       # run the parsing/formatting unit tests
make stage       # build and assemble the .sdPlugin directory
cargo clippy     # lint
```
