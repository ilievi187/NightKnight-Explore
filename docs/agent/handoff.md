# Agent handoff

Updated: 2026-10-29. `main` includes pagination fix `c4a496a` (#33).

## Landed

Live Overpass scans no longer hide matches past the first 120. The memory cache stores the full nearest-first list for one origin and radius. `total_matches` is the full count. `offset` reads the next page from that cache.

## This cycle

Map clusters were built once, in meters, and never rebuilt when the user zoomed, so a cluster click zoomed the camera but left the same bubble in place. Markers are now grouped in screen pixels and redrawn on `zoomend`. Grouping turns off at zoom 15 and closer. A cluster click zooms toward that cutoff.

## Do not repeat

- `feat/resilient-overpass-failover` and `feat/transparent-hidden-gems` are behind `main` and diverged. Their work already landed through later PRs. Do not revive them.
- Issue #16 is a historical session log. The backlog in its older comments is stale relative to `main`.

## Next objective

Estimated walking and driving time from a reviewed routing service. Do not invent ratings, reviews, or places. Do not call a public routing demo until its usage policy is recorded the same way Nominatim was.

## Verification still required

Read GitHub Actions on the open pull request. Local test success is not CI success.
