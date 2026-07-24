/**
 * Rivora installer Worker — https://rivora.dev/install
 *
 * Serves the tracked scripts/install.sh (embedded at build time).
 * Does not intercept unrelated website paths.
 */

import { INSTALL_SCRIPT } from "./script.generated.js";

const INSTALL_HEADERS = {
  "Content-Type": "text/x-shellscript; charset=utf-8",
  "Cache-Control": "public, max-age=300",
  "X-Content-Type-Options": "nosniff",
  "Referrer-Policy": "no-referrer",
};

function isInstallPath(pathname) {
  return (
    pathname === "/install" ||
    pathname === "/install/" ||
    pathname === "/install.sh"
  );
}

export default {
  async fetch(request) {
    const url = new URL(request.url);

    if (!isInstallPath(url.pathname)) {
      return new Response("Not Found", { status: 404 });
    }

    const method = request.method.toUpperCase();

    if (method === "GET") {
      return new Response(INSTALL_SCRIPT, {
        status: 200,
        headers: INSTALL_HEADERS,
      });
    }

    if (method === "HEAD") {
      return new Response(null, {
        status: 200,
        headers: {
          ...INSTALL_HEADERS,
          "Content-Length": String(
            new TextEncoder().encode(INSTALL_SCRIPT).length,
          ),
        },
      });
    }

    return new Response("Method Not Allowed", {
      status: 405,
      headers: {
        Allow: "GET, HEAD",
        "Content-Type": "text/plain; charset=utf-8",
        "X-Content-Type-Options": "nosniff",
        "Referrer-Policy": "no-referrer",
      },
    });
  },
};
