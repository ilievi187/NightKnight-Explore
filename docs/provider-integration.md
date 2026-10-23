# Provider and attribution contract

This document describes the provider boundary for the localhost MVP. A provider is not enabled merely because its fields appear in this contract. Integration requires a provider-specific terms, licensing, privacy, attribution, and operations review first.

## Place record contract

Each returned place carries:

- `id`: stable provider-scoped record identifier (currently `node-42`, `way-42`, or `relation-42` for OSM).
- `source`: human-readable source label retained for compatibility.
- `source_url`: canonical provider record URL, not a search result or an inferred URL.
- `attribution`: `{ provider_id, label, text, url, license }` metadata that applies to this record.
- `name`, category, coordinates, and distance: values supplied or calculated from the provider record; absent facts remain absent.

Clients must show the returned source attribution and keep its link adjacent to the place. Do not use provider names, logos, or attribution text to imply endorsement.

## Failure response contract

Errors retain the existing `error` message for compatibility and include:

- `code`: stable machine-readable state, such as `invalid_scan_parameters`, `provider_rate_limited`, `provider_unavailable`, or `provider_invalid_response`.
- `provider_id`: the provider that failed, or `null` when the request did not reach one.
- `retry_after_seconds`: a retry hint only when the server knows a bounded cooldown; otherwise `null`.

HTTP status mapping in this MVP: invalid scan input is 400; local Overpass cooldown is 429 with an 8-second retry hint; upstream failure or invalid upstream data is 502. A failed provider response is not a successful empty result. Clients must distinguish it from a successful scan with zero places and must not fabricate substitutes.

## Provider-specific caching and retention

| Provider/data | Status | Cache/retention behavior |
|---|---|---|
| OpenStreetMap POIs via configured Overpass | Enabled for explicit user-triggered scans | API response cache is memory-only for 15 minutes, keyed by rounded scan coordinates and radius. Expired entries are pruned during the next scan request. The shared endpoint has an 8-second request cooldown. No server-side persistence or user scan history. ODbL attribution remains on each place. |
| OpenStreetMap raster tiles | Displayed by MapLibre | Tiles are requested from `tile.openstreetmap.org`; the app does not add its own tile cache. Browser/network caching and display must follow the OSM Tile Usage Policy. |
| Google Places | Not configured / unavailable | No requests, caching, storage, or display until the owner approves a provider-specific license, account, attribution, caching, retention, display, privacy, and cost review. No scraping. |
| Reddit | Not configured / unavailable | No requests, caching, storage, or display until official API authorization and current terms permit the intended use and retention. No HTML scraping or access-control bypass. |
| Search and summary providers | Not configured / unavailable | Do not retain source material or generated output until source permissions, attribution, copyright, privacy, deletion, and retention rules are reviewed and recorded. |

Provider adapters must declare their own cache TTL, storage rules, attribution renderer, and deletion behavior. Do not inherit the Overpass policy by default: a provider's terms may prohibit or constrain storage. If a policy is unknown, disable the adapter and report it as unavailable.

## Compatibility and versioning

Attribution and structured error metadata are additive to the existing response. Keep the top-level scan `source`, `cached`, and `places` fields while clients migrate; nullable provider/retry fields represent cases where those values do not apply. A breaking change to these shapes requires an explicit API versioning decision and tests for both successful and unavailable provider states.
