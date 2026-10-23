# NightKnight Explore 🧭

**FOSS, local-first discovery of restaurants, hotels, attractions, museums, cafes and hidden gems.**

Status: **early MVP scaffold**, not a finished travel/reviews service. The initial scan uses OpenStreetMap data through an Overpass endpoint, on explicit user request. **No Google Maps or Reddit reviews are fetched in this version.** Ratings, review synthesis, booking, and cross-site sentiment analysis are roadmap features that require lawful provider access and verifiable citations.

## Tech stack

- React 19 + TypeScript + Vite frontend with responsive dark UI
- MapLibre GL JS map with OpenStreetMap raster tiles and attribution
- Rust + Axum API (localhost by default)
- OpenStreetMap/Overpass for discoverable POIs with clearly labelled source
- Windows-first scripts, Linux-compatible source, GitHub Actions CI
- AGPL-3.0-or-later license; provider integrations must respect source-specific terms

## Getting started on Windows

1. Extract this project to a normal development directory, e.g. `C:\Dev\NightKnight-Explore`.
2. Open PowerShell in the project folder and run `./scripts/check-windows.ps1` to check Git, GitHub CLI, Node.js, Cargo and Hermes.
3. Create the GitHub repository and push from this exact folder: `./scripts/publish-github.ps1` (the script asks GitHub CLI to authenticate when required). **This action publishes publicly by default.**
4. Terminal A: `./scripts/start-api.ps1` (Rust server on http://127.0.0.1:8787).
5. Terminal B: `./scripts/start-web.ps1` (React dev app on http://127.0.0.1:5173).
6. Terminal C: `./scripts/start-hermes.ps1` to open the existing local Hermes in the project folder; give it the instructions in `HERMES-MISSION.md`.

If PowerShell blocks downloaded scripts, inspect the script and use `Unblock-File` on the specific files, or `Set-ExecutionPolicy -Scope Process RemoteSigned` in the current session if needed. No permanent policy change is required.

On Linux/macOS: `cd crates/api && cargo run` and, separately, `cd apps/web && npm install && npm run dev`.

## How to scan

Set coordinates manually (Sofia is only a sample starting point) or explicitly allow browser location. Choose a radius from 500 m to 10 km and optional category filters. Click **Scan area**. The server fetches POIs near the chosen coordinates, deduplicates, and sorts by actual geographic distance. The UI does not invent popularity, reviews, open-hours or ratings. Click the OSM link to inspect the original place record.

Use **Save place** on a result to keep its OpenStreetMap record ID and canonical source URL in this browser's local storage. Saved links remain on this device and can be removed individually or cleared together. NightKnight Explore does not store scan history, names, coordinates, or search terms in favorites; clearing browser site data also removes saved links.

The public Overpass endpoint is a shared resource: this MVP enforces an 8-second cooldown, 15-minute in-memory cache and a limit of 10 km per query; it is **not** appropriate for high-volume production traffic. For public deployment provide your own properly provisioned Overpass instance or another licensed POI data provider. Tile loading must likewise follow the [OSM Tile Usage Policy](https://operations.osmfoundation.org/policies/tiles/).

## API

`GET /api/health` — server status

`GET /api/scan?lat=43.214&lon=27.915&radius_m=2500` — manual POI scan, results with canonical record URLs, actual distance, and per-place attribution; provider failures include machine-readable codes and retry hints when known. Requests may return 429 on cooldown or 502 if the public data provider is unavailable. No authentication is implemented; **do not bind this prototype to a public network interface**.

Options: `OVERPASS_ENDPOINT` defaults to `https://overpass.private.coffee/api/interpreter` (replace with trusted service); `BIND_ADDRESS` defaults to `127.0.0.1:8787`.

Provider response metadata, attribution, error states, cache/retention rules, and the disabled-provider review gate are documented in [`docs/provider-integration.md`](docs/provider-integration.md).

## Project plan

1. **MVP (now):** real OSM scanning, map, filters, source links, Windows developer setup and CI.
2. Persistence, privacy-first favorites, local cache, better discovery/ranking with transparent *non-review* signals.
3. Optional Google Places integration under an approved account and license. Respect attribution, caching, storage, display and API pricing rules; never use scraping to bypass access controls.
4. Optional Reddit integration only after explicit API approval and terms review; see the [current eligibility assessment](docs/reddit-api-assessment.md). Cite original public posts; never scrape Reddit. Public-web search only through a provider permitting the intended use. Respect copyright, privacy and removal requests.
5. Evidence-linked summaries from approved inputs, following the [summary design contract](docs/evidence-linked-summary-design.md), with no fabricated quotes or sentiment.
6. Desktop/PWA packaging, self-hosting and offline downloads only where licensed.

## Security and contributor rules

Read `AGENTS.md`. Never commit credentials, private travel/location history, API keys, or unlicensed review datasets. This MVP collects coordinates only when a user manually enters them or grants browser geolocation access; requests are sent to the configured POI provider through the local server, and map tiles require outbound network access. No analytics or ad SDK is bundled.

## CI and status

GitHub Actions checks TypeScript and frontend production build and runs Rust format, clippy and unit tests on Windows and Ubuntu. This ZIP was created as a **starter**, not a verified live deployment; check actual runs after first push.
