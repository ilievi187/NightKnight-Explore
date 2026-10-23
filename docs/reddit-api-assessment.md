# Reddit Data API eligibility assessment

Assessment date: 2026-10-23
Status: NOT CONFIGURED / UNAVAILABLE — no Reddit API calls, credentials, account registration, scraping, or content ingestion were performed.

## Decision

Do not ship a Reddit adapter in NightKnight Explore now. Reddit's Responsible Builder Policy, updated 2026-10-05, requires explicit approval before accessing any Reddit data through an API. No NightKnight Explore access approval, OAuth client, or written use-case authorization is present in this repository or this assessment. The repository owner must first submit the product's actual use case to Reddit and obtain written approval; a public repository or a FOSS license is not evidence that the product is eligible for non-commercial access.

Reddit also announced a migration of public Data API apps and bots to its Developer Platform: new public API access requests stop being accepted on 2026-10-31, removal for apps/users that have not registered or responded to Reddit outreach begins 2027-01-12, and remaining public API access ends in March 2027. As of this assessment (2026-10-23), the new-request cutoff is eight days away and no NightKnight request has been submitted. This short window and uncertain fit for an external location-discovery site make a durable API adapter especially unsuitable; contact Reddit before doing any API work if the owner still wants to pursue it.

There is a second product-fit blocker: Data API Terms grant a limited license to copy and display user content only as necessary to operate the approved app, and prohibit modifying user content except for formatting it for display. The Terms reserve other uses—including model training—unless rightsholders expressly permit them. Treat paraphrasing, summarizing, sentiment extraction, and model input as outside the default authorization unless Reddit and the relevant rights holders explicitly approve the exact intended use; this is a conservative implementation interpretation, not a legal opinion.

## Current access and eligibility requirements

- Request and obtain Reddit's explicit approval before any API access. The Responsible Builder Policy applies to developers, apps, researchers, and other users of Reddit data. It directs developers toward Devvit and says unsupported use cases should be raised with Reddit rather than assumed eligible for a third-party API client.
- Use the authorized Data API and registered OAuth credentials, an honest unique User-Agent, and only the approved use case and minimum necessary scopes/actions. Do not use scraping, anonymous requests, alternate accounts, or access-limit workarounds.
- Reddit distinguishes commercial and non-commercial use broadly. It calls out use by or on behalf of a business, monetized services, advertising, subscriptions, sponsorships, upsells, and free product features available for upsell as commercial examples. Commercial use requires explicit permission and a separate contract. NightKnight Explore's future monetization and entity use have not been established here; do not self-classify it as eligible for free/non-commercial access.
- Reddit's current public guidance describes both free and paid access; broader/bulk access may incur fees, and Reddit reserves the right to set fees. It publishes no price applicable to this unapproved use case, so no cost estimate is available. Ask Reddit for written pricing and limits as part of any application.
- Public Data API access is on a published migration path to Reddit's Developer Platform; the current transition dates are listed above. Do not build around the assumption that a legacy public API client can operate indefinitely.

## Rate limits and reliability

Reddit's Data API Wiki (updated 2026-05-11) describes a 100 queries/minute limit per OAuth client ID for eligible free-access usage, averaged over a 10-minute window, and recommends monitoring `X-Ratelimit-Used`, `X-Ratelimit-Remaining`, and `X-Ratelimit-Reset`. The Wiki also warns that older API materials may be out of date and directs developers to the current Terms. Treat these figures as operational guidance, not an entitlement or guaranteed quota; an approval or contract can set different limits, and Reddit can throttle, block, change, or discontinue API access.

## Content, attribution, privacy, and removal

If Reddit later approves the use case, require a further implementation review before enabling the adapter:

- Render only content and fields explicitly allowed in the approval; link every item to its original Reddit post and implement Reddit's required attribution. Do not imply Reddit endorsement or use Reddit marks outside the permitted branding rules.
- Do not edit, paraphrase, translate, or summarize user content except for display formatting unless Reddit and the applicable rights holders expressly authorize it. Do not use Reddit content to train models without Reddit's express permission. Do not infer sensitive characteristics or deanonymize users.
- Minimize data. Do not put Reddit text, author identifiers, or post metadata in NightKnight's local favorites or persistent caches by default. The API Terms require immediate deletion of material no longer needed for the approved use and, on termination/discontinuation, immediate deletion of cached/stored content and any data or models derived from it. The Data API Wiki further says deleted posts/comments and related data must be removed, author-identifying data must be removed after account deletion, and strongly recommends routinely deleting stored content within 48 hours.
- Provide a documented removal-request path, periodic deletion reconciliation, and a privacy notice before collecting or displaying Reddit-derived material. If these controls cannot be implemented for the approved scope, keep the adapter unavailable.

## Utility for NightKnight Explore

A Reddit post could provide additional first-person context about a specific destination, but this would require exact place matching, transparent post dates, original-post links, moderation/deletion handling, and safe treatment of user-generated claims. The current MVP has no approved Reddit access, no geospatial Reddit dataset, and no user-content moderation/deletion pipeline. The policy and rights constraints also do not permit the planned paraphrased/AI-summary experience by default. Expected value does not currently justify these compliance and retention costs; keep Reddit disabled.

## Required gates before reconsidering

1. Owner confirms the legal entity and whether any present or planned NightKnight Explore functionality is commercial/monetized.
2. Submit one accurate app profile/use-case request to Reddit; receive explicit approval before testing against Reddit data.
3. Obtain written scope for search, place matching, display, attribution, storage/cache, refresh/deletion, and any model-assisted processing; obtain pricing and rate limits if commercial or broader access is requested.
4. Review current Developer Terms, Data API Terms, Responsible Builder Policy, API documentation, and rights-holder restrictions again immediately before implementation.
5. Implement an approved, minimal adapter with OAuth only, descriptive User-Agent, rate-limit/backoff handling, canonical post links, deletion reconciliation, privacy disclosures, and tests. Do not scrape Reddit pages as a fallback.

## Official sources reviewed (2026-10-23)

- [Reddit Responsible Builder Policy](https://support.reddithelp.com/hc/en-us/articles/42728983564564-Responsible-Builder-Policy) — updated 2026-10-05.
- [Reddit Data API Terms](https://redditinc.com/policies/data-api-terms) — effective 2023-06-19; last revised 2026-07-20.
- [Developer Platform & Accessing Reddit Data](https://support.reddithelp.com/hc/en-us/articles/14945211791892-Developer-Platform-Accessing-Reddit-Data) — updated 2026-05-28.
- [Reddit Data API Wiki](https://support.reddithelp.com/hc/en-us/articles/16160319875092-Reddit-Data-API-Wiki) — updated 2026-05-11; operational guidance only, subordinate to current terms.
- [Reddit Changelog — October 8, 2026](https://support.reddithelp.com/hc/en-us/articles/54353370049684-Changelog-October-8-2026) — public Data API app/bot migration dates.
