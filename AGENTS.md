# Agent operating instructions — NightKnight Explore

- Owner: ilievi187. Product: NightKnight Explore, independent of NKNOVA, Paperclip and NightKnight Command Center.
- This is a **FOSS public repository**. Avoid private infrastructure details, secrets, personal data and privileged host automation in commits.
- Development workflow: inspect current state first; use focused feature branches, tests, descriptive commits and pull requests; request independent review. **Never merge into main or deploy without separate founder authorization.**
- Do not edit unrelated projects, and do not use Desktop Commander, unattended GUI automation, unapproved LAN scans or production tasks. Follow approved GitHub/Hermes Bridge/pinned SSH routes when needed.
- Key design principle: real information with source links; explicit `Unavailable`/`Not checked` for anything not supplied by providers. Never manufacture ratings, review text, verified sentiment, open hours or reservation availability.
- Provider compliance: OpenStreetMap is ODbL-licensed data, OSMF tiles have separate usage requirements, Overpass has shared-service usage limits. Google Places/Reddit require explicit account authorization and licensing/terms review; no scraping of those services.
- On every PR report exact test commands, outcomes and unresolved platform gaps; old CI is not proof for a new SHA.
- The initial API is intended for localhost development and not hardened as a public service. Do not change binding, auth or external access without a focused security review.
