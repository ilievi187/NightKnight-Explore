# Transparent Hidden Gems method (osm-tag-signals-v1)

Status: implemented (API field `gem` on each place). This document defines the signal so scores stay auditable.

## Principle

A "hidden gem" score here is **not** a rating, popularity index, or review synthesis. It is a deterministic, fully explained summary of the *interest-bearing characteristics mappers recorded about the place itself* in OpenStreetMap. Every point in the score comes from one concrete tag on one canonical record, and every serialised reason carries the exact `field_path` and `observed_value` that produced it.

What this signal deliberately does **not** do:

- No popularity or "trending" inference (neither OSM nor anything else provides it honestly here).
- No ratings, sentiment, or review text.
- No negative claims from missing tags. A place without any gem-relevant tag simply has no `gem` field at all — the absence is reported honestly, not as a zero-stamped verdict.
- Never definitive condition claims: dereliction signals are phrased as "mapped as derelict; verify on site" because tag data can be stale.

## Scoring table (v1)

Each contribution fires at most once per place; raw points are capped to 100.

| Points | Trigger (tags) | Reason text fragment |
|---|---|---|
| 25 | `historic` ∈ {monument, memorial} | monument or memorial — a historical landmark |
| 25 | `historic` ∈ {castle, fort, monastery, tower} | historical structure |
| 25 | `historic` ∈ {ruins, ruin} | ruins — condition varies; verify before visiting |
| 25 | `historic` = archaeological_site | archaeological site |
| 20 | `abandoned` = yes/true/1 | mapped as derelict; verify on site |
| 20 | `disused` = yes/true/1 (only when `abandoned` absent) | no longer in operation; verify on site |
| 15 | `tourism` = viewpoint | a vantage point noted by mappers |
| 15 | `tourism` = artwork | public artwork |
| 10 | `natural` ∈ {beach, spring, cave_entrance} | natural feature |
| 10 | `waterway` = waterfall | waterfall |
| 10 | `leisure` = garden | garden |
| 10 | `wikidata` / `wikipedia` / `image` non-empty | record links documentation (image, Wikipedia or Wikidata) |
| 5 | `description` non-empty | carries an OSM description field |

The serialised `gem.method` is `"osm-tag-signals-v1"` so clients and future versions can distinguish scoring generations. Any change to this table must bump the method id and update this document in the same PR.

## Data scope

Signals are computed only from the tags of records fetched by an explicit user-triggered scan through the configured Overpass services. They are computed fresh from scan responses, held only in the same 15-minute memory cache as the scan itself (see [provider contract](provider-integration.md)) and never persisted or combined with user history.

## Downstream obligations

- The UI renders `gem.reasons` verbatim alongside the record link; it must not paraphrase reasons into claims of quality or popularity.
- UI tier labels ("Notable / Hidden gem / Rare find") derive only from the published score; they summarize the same tag evidence, nothing else.
- `gem: null` renders nothing and never becomes "not a gem" prose.
