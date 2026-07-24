# rivora-install Worker

Serves the canonical Rivora installer at:

```text
https://rivora.dev/install
```

## Source of truth

The shell script is tracked at `scripts/install.sh` in the Rivora repository.

`build.mjs` embeds that file into `src/script.generated.js` at deploy time.
The Worker never proxies an unpinned raw `main` branch URL.

## Deploy

From the repository root (requires Cloudflare auth via `wrangler login`):

```sh
# Build embeds scripts/install.sh
node distribution/install-worker/build.mjs

# Deploy (takes ownership of rivora.dev/install routes)
npx wrangler deploy -c distribution/install-worker/wrangler.toml
```

This Worker is the **only** owner of `rivora.dev/install`. The legacy commercial
Workers (`rivora-cli-installer` and the `cli.rivora.dev` proxy on
`rivora-mvp-workers`) were removed so they cannot reclaim this path.

## Headers

| Header | Value |
|--------|--------|
| `Content-Type` | `text/x-shellscript; charset=utf-8` |
| `Cache-Control` | `public, max-age=300` |
| `X-Content-Type-Options` | `nosniff` |
| `Referrer-Policy` | `no-referrer` |

- `GET` / `HEAD` → 200
- other methods → 405

## Local check

```sh
node distribution/install-worker/build.mjs
npx wrangler dev -c distribution/install-worker/wrangler.toml
curl -i http://127.0.0.1:8787/install
```
