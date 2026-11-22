# Asset upload port

The original `asset-upload` scripts are now available in `trek` behind the optional
`asset-upload` Cargo feature. The TypeScript environment-variable contract is unchanged.

## API mapping

| Original | Rust |
| --- | --- |
| `index.ts` upload flow | `src/upload/mod.rs` (`upload`) |
| `login(browser, page)` | `src/upload/browser.rs` (`BrowserSession::authenticate`, `login`) |
| `refresh-cookie.ts` | `trek upload --refresh-cookie` |
| `api(cookie)` axios helpers | `src/upload/api.rs` (`Api`) |
| `CFX_FORUM_COOKIE` | `CFX_FORUM_COOKIE` (required) |
| `ASSET_NAME` | `ASSET_NAME` (required) |
| `ZIP_FILE` | `ZIP_FILE` (optional) |
| `VERSION` | `VERSION` (optional) |
| `dist/*.zip` selection | `SelectedZip` / `first_zip` |
| Version regex | `derive_version` |
| Chunk upload | `upload_chunks` / `Api::upload_chunk` |
| Pack polling and download | `wait_for_pack`, `Api::pack_download_url`, `Api::download` |

## Test mapping

| Behavior | Rust test |
| --- | --- |
| Version derivation from ZIP names | `upload::tests::derives_version_like_the_typescript_script` |
| Final `.zip` suffix removal | `upload::tests::strips_only_the_final_zip_suffix` |
| Basename handling | `upload::tests::reads_file_name_from_a_path` |
| First ZIP selection | `upload::tests::selects_the_first_zip_in_dist` |
| Missing ZIP error | `upload::tests::reports_when_dist_has_no_zip` |
| Requested ZIP selection | `upload::tests::selected_zip_uses_the_requested_name` |
| Duplicate version lookup | `upload::tests::finds_duplicate_versions_by_exact_name` |
| Oldest version lookup | `upload::tests::finds_the_oldest_version_by_id` |
| Re-upload payload | `upload::api::tests::reupload_request_matches_the_typescript_payload` |
| Nullable re-upload errors | `upload::api::tests::reupload_errors_are_nullable` |
| CLI flag parsing | `cli::tests::parses_upload_options` |
| Missing cookie / asset name / ZIP | `tests/cli.rs` upload tests |
| Help output | `tests/cli.rs::upload_help_lists_refresh_cookie` |

## Preserved behavior

- `dist/` is resolved from the current directory.
- Without `ZIP_FILE`, the first directory entry ending in `.zip` is used, matching
  `readdirSync` order.
- Version inference uses the original `^.*?[-](\d[\w.]+\d)` capture and then removes a
  trailing `.zip`.
- ZIPs are uploaded in 2 MiB chunks with `chunk_id`/`chunk` multipart fields.
- An existing version with the same string is deleted first; while five versions remain,
  the oldest by id is deleted.
- Pack generation polls 30 times at two-second intervals (60 seconds total).
- The downloaded pack is saved as `<original stem>.pack.zip` in `dist/`.
- Progress and error text match the original scripts.
- `--refresh-cookie` validates the login without uploading or printing on success.
- The browser stays open through the HTTP upload and is closed afterwards, matching the
  original `finally` block.

## Deliberate differences

- A missing `errors` field in the re-upload response is treated as success; the original
  checked `!== null` and would also fail for `undefined`.
- HTTP failures now include the request action and status from `reqwest`.
- The output directory is not created automatically; `dist/` must exist, as before.
- Puppeteer downloaded a matching browser; `chromiumoxide` uses the Chrome or Chromium
  already installed on the machine.

## Verification

```bash
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
cargo test --features asset-upload
RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --all-features
```
