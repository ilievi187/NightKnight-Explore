# NightKnight Explore 🧭

**FOSS, local-first discovery of restaurants, hotels, attractions, museums, cafes, historical landmarks and hidden gems.**

Status: **early MVP scaffold**, not a finished travel/reviews service. The initial scan uses OpenStreetMap data through a failover chain of Overpass endpoints, on explicit user request. **No Google Maps or Reddit reviews are fetched in this version.** Ratings, review synthesis, booking, and cross-site sentiment analysis are roadmap features that require lawful provider access and verifiable citations.

Each result can carry a transparent **Hidden Gems signal**: a deterministic score plus per-point explanations derived only from that record's own OSM tags (historical structures, viewpoints, waterfalls, dereliction, documentation links…). The method is documented in [`docs/hidden-gems-method.md`](docs/hidden-gems-method.md); no popularity or rating is invented anywhere.

## Tech stack

- React 19 + TypeScript + Vite frontend with responsive dark UI
- MapLibre GL JS map with OpenStreetMap raster tiles and attribution (map engine is lazy-loaded after the app shell)
- Rust + Axum API (localhost by default)
- OpenStreetMap/Overpass for discoverable POIs and explicit Nominatim place lookup, with clearly labelled sources
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

On Linux/macOS: run `./scripts/start-api.sh` (Rust API at `127.0.0.1:8792`, using `data/osm-bulgaria.db` when available) and then `./scripts/start-web.sh` (web app at `127.0.0.1:5199`). Port `8787` is reserved for the separate OmO Board service on NightKnightPC. The Vite proxy points to port `8792` by default; if you override the backend `BIND_ADDRESS`, set the matching `EXPLORE_API_TARGET` for Vite as well. For example: `BIND_ADDRESS=127.0.0.1:8800 ./scripts/start-api.sh` and `EXPLORE_API_TARGET=http://127.0.0.1:8800 ./scripts/start-web.sh`. After updating the repository, verify `http://127.0.0.1:5199/api/health` and run a small `/api/scan` request to catch proxy mismatches.

## How to scan

Search for a city or landmark and submit the form (there is deliberately no autocomplete), choose one of at most five OpenStreetMap Nominatim results, or set coordinates manually/allow browser location. Choose a radius from 500 m to 10 km and filter across 5 categories (attractions, historical landmarks, nature, food, stays). Click **Scan area**. The server fetches POIs near the chosen coordinates, deduplicates, sorts them (by distance or by transparent gem signal), and shows each place's source and gem reasoning. The UI does not invent popularity, reviews, open-hours or ratings. Click the OSM link to inspect the original place record.

Use **Save place** on a result to keep its OpenStreetMap record ID, canonical source URL, public place name and category key in this browser's local storage — the name and category are the record's own public labels, shown on the Saved places screen. NightKnight Explore still does not store scan history, coordinates, search terms, gem scores or any derived personal data in favorites; clearing browser site data also removes saved links.

The public Overpass endpoints are shared resources: this MVP enforces an 8-second cooldown, 15-minute in-memory cache, bounded failover with per-endpoint retry limits, and a hard radius cap of 10 km per query; it is **not** appropriate for high-volume production traffic. For public deployment provide your own properly provisioned Overpass instance or another licensed POI data provider. Tile loading must likewise follow the [OSM Tile Usage Policy](https://operations.osmfoundation.org/policies/tiles/).

Explicit place searches use the public Nominatim service by default, with a one-request-per-second local API cooldown and a 24-hour process-memory cache. Nominatim is donated low-capacity infrastructure; its one-request/second maximum applies across the *whole application*. Do not deploy this public endpoint as a shared multi-user geocoder: configure a reviewed, appropriately provisioned provider with `GEOCODER_ENDPOINT`. See the [policy decision and safeguards](docs/nominatim-geocoder-decision.md).

## API

`GET /api/health` — server status

`GET /api/scan?lat=43.214&lon=27.915&radius_m=2500` — manual POI scan, results with canonical record URLs, actual distance, per-place attribution, and an optional explained `gem` signal; provider failures include machine-readable codes and retry hints when known. Requests may return 429 on cooldown or 502/`provider_timeout` if the data providers are unavailable or stalled. No authentication is implemented; **do not bind this prototype to a public network interface**.

`POST /api/geocode` (form field `q`) — explicit user-submitted place search; returns up to five results and OSM attribution. Search terms are not placed in the local API URL/log span. No autocomplete or automatic retry. Searches are cached in process memory for 24 hours; see [`docs/nominatim-geocoder-decision.md`](docs/nominatim-geocoder-decision.md).

Options: `OVERPASS_ENDPOINT` (optional) pins a single trusted Overpass service; by default the API fails over across a small list of public Overpass instances and marks unhealthy ones offline for 30 seconds. `GEOCODER_ENDPOINT` switches Nominatim to a reviewed alternative without a software update (API restart required). See [`docs/provider-integration.md`](docs/provider-integration.md). Real upstream queries take 5–40 seconds under load; the API reports honest `provider_timeout`/`provider_unavailable` errors rather than stalling forever. `BIND_ADDRESS` defaults to `127.0.0.1:8792` on the Rust API; Vite defaults to proxying `/api` to this address.

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
