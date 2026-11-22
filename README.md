# trek

Minimal CLI for FiveM resource archives. Pack files from a `.pack` glob list, read or bump the version in `fxmanifest.lua`, and create release archives with a Markdown report.

## Install

Requires Rust 1.89 or newer.

```bash
git clone https://github.com/merzen-sh/trek.git
cd trek
cargo install --path .
```

## Quick Start

Run from your resource directory:

```bash
cd my-resource
```

Create `.pack` with the files to include (`*.lua` matches the root, `**/*.lua` recurses):

```text
*.lua
**/*.lua
```

Create an archive:

```bash
trek pack
```

This writes `<folder-name>.zip`. Add `--report` for a Markdown summary or `--sha256` for a checksum.

Read or bump the resource version:

```bash
trek version
trek version --patch
```

Run a complete release:

```bash
trek release --patch --sha256 -o ../dist
```

This updates `fxmanifest.lua` (`1.2.3` -> `1.2.4`), writes `../dist/<folder-name>.zip`, and prints a Markdown report.

## Optional: Upload to CFX

Build with the `asset-upload` feature:

```bash
cargo install --path . --features asset-upload
```

The upload feature uses a locally installed Chrome or Chromium on `PATH`; it does not download a browser.

From a resource with a ZIP in `dist/`:

```bash
export CFX_FORUM_COOKIE="..."
export ASSET_NAME="trek-core"
trek upload
```

`ZIP_FILE` and `VERSION` override the selected ZIP and inferred version. Validate the cookie without uploading:

```bash
trek upload --refresh-cookie
```
