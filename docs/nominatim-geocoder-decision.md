# Place search geocoder decision

Reviewed: 2026-10-23 against the official [Nominatim Usage Policy](https://operations.osmfoundation.org/policies/nominatim/).

## Deliberate integration decision

NightKnight Explore adds **explicit, user-submitted place lookup** through the Nominatim Search API for this localhost-first prototype. It is submitted only when the user presses Search; the feature is not autocomplete, background lookup, bulk geocoding, reverse-grid scanning, or POI harvesting. Results are limited to five per request and supply a name, coordinates, canonical OSM object link and visible OSM attribution.

The public Nominatim service is a donated, low-capacity service. Its policy allows directly user-triggered searches for apps with moderate users, with an absolute maximum of one request per second **across the application**, a meaningful app User-Agent or Referer, visible attribution, caching where possible, and the ability to switch services without a software update. It prohibits client-side autocomplete and systematic/bulk queries. The public endpoint may change policy or withdraw access without notice.

## Safeguards in this implementation

- No autocomplete: browser performs a request only on explicit form submission.
- A server-side shared one-request-per-second cooldown; requests inside the cooldown fail locally with `provider_rate_limited` and a retry hint rather than reaching Nominatim.
- Five results maximum, 8-second client timeout, and no automatic retry of failed searches.
- Successful normalized queries and results are cached in process memory for 24 hours, bounded to 500 entries; expired entries are pruned and the oldest entry is evicted when the cap is reached. No disk persistence or search history.
- A descriptive `NightKnightExplore/0.1 (FOSS local development)` User-Agent is sent by the Rust API. The app does not log search terms or persist them to disk; submitted queries are sent to the selected external geocoder, whose own processing and privacy policy applies.
- The browser submits to the localhost API using POST form data instead of a query-string URL, keeping the entered term out of normal request URI trace spans.
- The UI discloses that a submitted query is sent to the configured provider and displays © OpenStreetMap contributors attribution beside search.
- `GEOCODER_ENDPOINT` selects an alternative HTTPS geocoder endpoint (or local loopback HTTP for development) without a software update; changing it requires restarting the local API. Adapter compatibility, licensing, attribution and rate limits for any alternative must be reviewed before use.
- The API remains bound to localhost and is not hardened for public service deployment. A public multi-user deployment must not share the public Nominatim default: use a suitably provisioned self-hosted instance or a reviewed provider, and enforce a true application-wide quota.

## Endpoint and errors

`POST /api/geocode` with form field `q=<explicit search>` returns up to five results with canonical OSM links and an attribution object. Queries shorter than 2 or longer than 200 characters are rejected. Provider outage, timeout, malformed data, and rate limiting are distinct from a successful zero-result search. A failed provider response is never presented as an empty success.

## Sources

- [Nominatim Usage Policy](https://operations.osmfoundation.org/policies/nominatim/) — official policy for `nominatim.openstreetmap.org`; consulted 2026-10-23.
- [OpenStreetMap copyright and license](https://www.openstreetmap.org/copyright) — ODbL and attribution.

This is an engineering compliance decision, not legal advice. Recheck the live policy and the selected endpoint's terms before release or broader deployment.
