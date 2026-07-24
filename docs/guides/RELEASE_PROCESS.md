# Release Process (contributors)

Rivora uses trunk-based development on `main`.

## Version patch (example: v0.9.1)

1. Bump workspace version in root `Cargo.toml` (`[workspace.package].version`).
2. Update `CHANGELOG.md`, README, install/distribution docs as needed.
3. Validate:

   ```bash
   cargo fmt --all -- --check
   cargo clippy --workspace --all-targets --all-features -- -D warnings
   cargo test --workspace --all-targets --all-features
   cargo build --workspace --release
   sh scripts/tests/install.test.sh
   ```

4. Confirm `rivora` / `rivora-workspace` report the new version.
5. Commit on `main` and push (no feature branches for release commits unless
   required by policy).
6. Create an annotated tag matching the workspace version:

   ```bash
   git tag -a v0.9.1 -m "Rivora v0.9.1 — Binary Distribution and Installer"
   git push origin v0.9.1
   ```

7. The `Release` GitHub Actions workflow (`.github/workflows/release.yml`) builds
   archives for all supported targets, generates `SHA256SUMS`, and uploads
   assets to the GitHub Release. Tag and workspace version must match.
8. Publish or complete the GitHub Release notes from `CHANGELOG.md`.
9. Deploy the install Worker if `scripts/install.sh` changed:

   ```bash
   node distribution/install-worker/build.mjs
   npx wrangler deploy -c distribution/install-worker/wrangler.toml
   ```

10. Verify production:

    ```bash
    curl -fsSL https://rivora.dev/install | RIVORA_INSTALL_DIR=/tmp/rivora-verify sh
    /tmp/rivora-verify/rivora --version
    ```

## Dry-run builds

Use workflow_dispatch with `dry_run=true` to package without uploading.

## Asset contract

See `docs/guides/DISTRIBUTION.md`.
