# Website maintenance

[Documentation index](README.md) · [Project overview](../README.md)

The official site is <https://miccamwatch.pages.dev/>, hosted on Cloudflare
Pages. GitHub Releases hosts downloads; the static site is not an executable mirror.
This guide describes maintenance, **not authorization to deploy**. A documentation
change must not silently publish an application candidate or change production.

## Source and release facts

`site/` contains shared templates and catalogs for English, French, German,
Spanish, Japanese, Simplified Chinese, and Russian. Node is build-time tooling,
not a visitor-side runtime. No application GUI framework is involved.

- `site/release.json` describes the **published** release, not the Cargo version
  of an unpublished development branch.
- `site/origin.json` records `https://miccamwatch.pages.dev`.
- Source screenshots are authentic historical Windows captures; provenance,
  version, and limits are recorded in [`captures.json`](../site/assets/captures.json).
  Localized captions do not turn them into current-version or hardware evidence.
- Privacy content and hosting/download processing are described in the
  [official policy](https://miccamwatch.pages.dev/privacy). Do not infer the
  Cloudflare account's live analytics settings from an old source comment.

## Build and check locally

Run from the repository root with Node.js. Node 24 is the previously qualified
major used in the documented build/deployment setup.

```sh
node --test site/localization.test.mjs
node site/build.mjs
```

Generated output is **`site-dist/`**, not `site/`. It contains the root English
home/404 pages, localized routes, privacy pages, sitemap/robots resources, static
assets, and the exported installer. The exporter validates release facts,
catalogs, technical tokens, and canonical installer integrity rather than silently
falling back to another language.

The exported installer must match the SHA-256 in `site/release.json`, even when
the application checkout is newer. A matching local `installer/install.sh` is
reused; otherwise the builder fetches that file from the exact published
40-character source commit on GitHub over HTTPS, without redirects, and verifies
the same digest before writing output. That build requires network access.
An unavailable source or mismatched digest stops the export without replacing
existing output. Never update the public release pin just to build a candidate.

The root remains English; locale choice is explicit, not an automatic redirect.
Keep the seven locales, canonical URLs, `hreflang`, sitemap, and root `WebSite`
metadata coherent. The root privacy URL is a permanent public entry point.

The exporter refuses unowned or symlinked output and preserves unrelated files.
If Windows returns EPERM or an ownership check fails, inspect the existing output;
do not force deletion, alter ACLs, or bypass antivirus to make a build pass.
The output directory is generated content and should not be committed.

## Cloudflare Git integration

Use the repository root, not `site`, as the build root:

| Setting | Value |
| --- | --- |
| Framework | None |
| Repository root | Empty/default |
| Build command | `node site/build.mjs` |
| Output directory | `site-dist` |
| Node major | `NODE_VERSION=24` reproduces the qualified setup |

Publishing raw `site/` can expose unresolved templates and omit the generated
installer. Keep production pointed at the deliberately approved branch; pushing a
GUI or documentation branch is not approval for a production deployment.

Canonical/hreflang/sitemap and localized social images use the stable production
origin, **not** an ephemeral `CF_PAGES_URL` preview hostname. `MCW_SITE_URL` can
explicitly select another real HTTPS root origin for a separate deployment.
A paid/custom domain is not required; keep the official `pages.dev` address.

## Direct upload

After an explicitly approved build, upload the **contents** of `site-dist/`, or a
ZIP with `index.html` at its root, using Cloudflare Workers & Pages → Create
application → Direct Upload. No Worker/Functions are required. See the
[official Direct Upload guide](https://developers.cloudflare.com/pages/get-started/direct-upload/).

Cloudflare Direct Upload projects cannot later switch to Git integration;
create a separate Git-integrated project if that workflow is needed. Do not
replace the existing production project merely to preview documentation.

## After an authorized deployment

Check the HTML actually served, not just the local build:

- Root English and all seven language routes, without JavaScript or authentication.
- Privacy routes, contact links, language selectors, and footer links.
- Release/download versions, checksum instructions, and the exact exported installer.
- Canonical, `hreflang`, sitemap/robots, structured data, favicon, and localized 404s.
- Desktop/mobile layout, keyboard platform tabs, and secure headers.

Do not relax CSP to permit an unexpected injected beacon. Confirm live hosting
settings separately. Earlier Cloudflare repairs, browser/clipboard limitations,
and deployment hashes remain version-scoped in the
[release history](RELEASE_HISTORY.md), not claims about today's production state.
