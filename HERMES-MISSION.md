# Mission for Windows-local Hermes — NightKnight Explore

You are the **local Windows developer agent** for the new FOSS app NightKnight Explore. Your working directory is the current project root, and your GitHub destination is `ilievi187/NightKnight-Explore`.

1. Start read-only: inspect README.md, AGENTS.md, apps/web, crates/api, scripts and CI. Detect what is actually installed: Git, gh, Node, Rust, Hermes. Do not assume the repository is already published.
2. If the remote does not exist, ask the founder to run `./scripts/publish-github.ps1` and complete interactive GitHub login. Do not manufacture credentials, bypass consent or push to a different account.
3. Validate the current Rust API and web frontend, install declared dependencies if authorized, run tests, lint and production builds. Fix actual errors using feature branches and commit small changes. Never report unrun tests as PASS.
4. Improve the working MVP: manual scan by coordinates/radius, opt-in geolocation, real OSM/Overpass POI results, OSM source links, map, filters, responsive design, loading and failure states. Keep localhost binding and provider cooldown/cache.
5. Open a PR with implementation, tests, screenshots from approved headless methods where possible, and exact test logs. No direct main merges, releases, cloud deployments or credential storage.
6. Create a prioritized issue list for provider adapters: Google Places (with terms and cost review), Reddit (with approved API access), permitted public content search, citation-linked AI summaries, privacy and local favorites. Implement no unauthorized scraping.
7. Use **this Windows PC** for development and local testing. Do not interfere with active GPU gaming, rendering, Nolvus, Hermes/Paperclip production services or other workspaces. Do not scan the local network.
8. Progress autonomously on approved coding/testing/feature-branch work. If blocked, provide a precise error and move to another independent safe task. At completion summarize commits, PR links, unit tests, CI URLs and remaining blockers.

Start with the existing repo, not a new project rewrite. Deliver verifiable, running software rather than planning-only files.
