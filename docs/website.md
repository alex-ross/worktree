# Website

The website at https://worktree.aross.se is plain HTML, CSS, JavaScript, and product
images in `/web`. It has no build dependencies, server code, or Pages Functions.

The GitHub mark is downloaded from [logo.dev](https://www.logo.dev/search/brands/github.com)
and served locally as `web/assets/github.svg`.

The stylesheet URL is versioned to replace older cached theme rules. Its `no-cache`
header lets browsers store it but requires revalidation before reuse.

## Preview and check

```sh
python3 -m http.server 4173 --directory web
node --test tests/web.mjs
```

Open http://localhost:4173. Check both themes and a narrow viewport. Theme selection
uses the system setting until the visitor chooses a theme; that choice is saved
locally. The content and installation commands also work without JavaScript.

## Deployment

Cloudflare Pages project: `worktree` in the account that owns `aross.se`.

| Setting | Value |
| --- | --- |
| Git repository | `alex-ross/worktree` |
| Production branch | `main` |
| Root directory | Repository root |
| Build command | `node --test tests/web.mjs` |
| Output directory | `web` |
| Production deployments | Every push to `main`, including merges |
| Preview deployments | Other branches |
| Custom domain | `worktree.aross.se` |
| Pages hostname | `worktree-auv.pages.dev` |

The native [Pages Git integration](https://developers.cloudflare.com/pages/configuration/git-integration/)
handles deployment, so no GitHub deployment secrets or additional workflow are needed.
Preview deployments are separate from production and Cloudflare marks them `noindex`.
The production Pages hostname also has a `noindex` header; the canonical URL points
to the custom domain, which remains indexable.

`index.html` contains the searchable content, canonical URL, social metadata, and
SoftwareApplication structured data. `robots.txt` allows crawling and advertises
the sitemap. These make the site eligible for indexing; Google chooses when to index it.

Update the Homebrew commands alongside README changes. They currently install HEAD
because the formula has no tagged release. See [product image provenance](product-images.md)
before replacing the screenshot. Use only isolated, fictional demo data.

## Security headers

`web/_headers` applies a restrictive Content Security Policy to every static response,
including the 404 page. Only local scripts, styles, and images are allowed; embedding,
forms, plugins, and other resource types are blocked. The inline JSON-LD is a non-executable
data block and needs no script exception. The website test rejects inline executable
scripts and styles. No `unsafe-inline` or `unsafe-eval` is allowed.

HSTS requires HTTPS for one year on the responding hostname. It intentionally omits
`includeSubDomains` and `preload`, so it does not commit other hostnames to this policy.
The existing `nosniff` and `strict-origin-when-cross-origin` headers remain enabled.
Headers are served through [Cloudflare Pages](https://developers.cloudflare.com/pages/configuration/headers/).

The PTK report also included findings outside this site's scope: its cookie list belonged
to Google and GitHub, and `root.dataset.theme` is only the color-theme switch, not an admin
gate. The live response has no credentialed CORS header or legacy `X-XSS-Protection`.
`Server: cloudflare` identifies the CDN without exposing an application version.
Do not commit raw scanner exports containing session cookies or authorization headers.
