# Evidence-linked summary design

Status: DESIGN ONLY. NightKnight Explore does not generate AI summaries, fetch Reddit content, or call an external model. Build a summarization feature only after every input source and processing step is approved for the intended use. This document defines constraints and a reviewable contract, not a provider authorization or legal opinion.

## Product rule

A displayed factual sentence must be traceable to one or more approved source records, or be explicitly marked as unverified and not stated as fact. Prefer deterministic formatting of provider facts over generative paraphrase. An empty or failed source response is never evidence that a place lacks a feature.

Never infer or invent ratings, review text, sentiment, popularity, open hours, prices, safety, reservation availability, accessibility, or service quality. Do not fabricate quotations. Do not turn a missing tag/field into a negative claim.

## Evidence model

A summary is a set of independently renderable claims, not one opaque paragraph. Each claim carries its evidence and assessment alongside it:

```json
{
  "status": "available",
  "generated_at_unix": 0,
  "claims": [
    {
      "id": "claim-1",
      "text": "This record is tagged as a museum.",
      "assessment": "supported",
      "kind": "provider_field",
      "evidence": [
        {
          "provider_id": "openstreetmap",
          "record_id": "node-42",
          "record_url": "https://www.openstreetmap.org/node/42",
          "field_path": "tags.tourism",
          "observed_value": "museum",
          "observed_at_unix": 0
        }
      ]
    }
  ]
}
```

The timestamps above are schema examples only, not observations. Each generated claim must have at least one valid, approved evidence reference. Derived claims must list all material inputs and a deterministic method/version; for example, distance uses both the scanned place coordinates and the user-selected origin. Preserve provider IDs, canonical record URLs, field paths, access times, and per-record attribution. Do not persist copied source prose in the summary object by default.

## Required statuses

- `available`: a successful scan returned source-backed claims; each claim carries its own evidence.
- `unavailable`: the provider/feature is not enabled, denied, or failed. State the provider failure separately from a successful scan with zero places.
- `not_checked`: the relevant source or field was not queried. It must not be rendered as “no”.
- `unverified`: claim-level marker for a statement without sufficient approved evidence. Do not present it as a factual conclusion; normally show “Not verified” or omit the claim.
- `stale`: evidence is older than that provider's approved freshness policy, or freshness cannot be established. Keep the original access time visible and do not silently present it as current.
- `conflicting`: approved records disagree. Show the competing, separately attributed claims and their timestamps; do not average them, choose a winner, or hide a source without a declared provider rule.

The summary-level status may be `conflicting` or `stale` even when some individual claims remain supported. Every claim's evidence assessment must remain visible independently.

## Source and licensing gates

- OpenStreetMap is the only currently enabled POI source. Display the canonical record link and the returned attribution on each sourced place; respect ODbL and provider-specific attribution requirements.
- Google Places remains disabled until account authorization, license, content/display/storage/caching, attribution, privacy, and cost requirements are approved for this product.
- Reddit remains disabled. See the [Reddit API eligibility assessment](reddit-api-assessment.md): public API access requires approval, is on a dated migration path, and Reddit user content cannot be paraphrased or summarized under the default display-only grant.
- A web page being public does not itself authorize copying, storing, summarizing, or model processing. Record the allowed fields, uses, attribution text/link, cache TTL, deletion procedure, and rights/terms reviewer for every source before adding it.
- Official OSM attribution/licensing reference: [OpenStreetMap copyright and license](https://www.openstreetmap.org/copyright).

## Privacy, retention, and removal

- Keep scan inputs and any summary state in memory only; the existing OSM result cache may retain provider responses in memory for at most 15 minutes as documented in the [provider contract](provider-integration.md). Do not persist scan history or summaries by default. Local favorites store only selected OSM record IDs and source URLs.
- Do not send precise coordinates, search history, user identifiers, or unrelated source records to an external model. If model processing is later approved, disclose the recipient, fields sent, purpose, retention, and training policy before enabling it; obtain any required consent and keep the feature off by default.
- Do not train or fine-tune on source content without explicit authorization covering that use.
- If a provider removes a record or withdraws access, stop showing affected claims, remove derived content as required, and preserve only the minimum provenance needed for an auditable deletion record. Provider-specific obligations override generic application defaults.
- Never copy source text into logs or analytics. No analytics or model service is currently configured.

## UI and review requirements

1. Put a source link/attribution control beside each claim, not only in a global footer.
2. Show claim freshness and whether it is directly observed or calculated.
3. Render `Unavailable`, `Not checked`, `Unverified`, `Stale`, and `Conflicting` distinctly; never collapse them into an empty result.
4. Let a user open the original record and inspect the exact provider fields supporting a claim.
5. Test missing evidence, malformed source links, deleted sources, stale timestamps, conflicting assertions, provider outage, and successful empty scans.
6. Before enabling generation, review the exact input/output behavior against each provider's current written terms and verify deletion/retention behavior end to end.

## Current implementation boundary

Issue #5 is a design contract only. No generated summary code, prompts, model dependency, Reddit data, or third-party inference request is included. Reopen implementation work only after the relevant source approvals are documented.
