# Decision-focused README and engineering interview

The user requested moving the existing README content into docs and replacing it with a simplified README that explains design decisions and tradeoffs. They want an interview about their decisions and reasoning so the README highlights the engineering behind the project.

## Audience and editorial direction

User-confirmed audience: other engineers and hiring managers who are also engineers. The main takeaway should be technical judgment, especially tradeoffs. Separate operational/setup detail from decision rationale. Explain constraints, alternatives, the chosen approach, its costs and when to revisit it. Do not present inferred or assistant-proposed rationale as the user's own thought process.

Moved the original README content to docs/project-guide.md, preserving its technical information and adjusting relative links. Commands still run from the repository root. The README began as a factual outline and has now been consolidated using the completed component interview, with architecture, confirmed motivations and observable tradeoffs. No code, infrastructure or product behavior changes are required.

User-confirmed motivation: this was an interview case study. The first useful version should be a working, enjoyable experience with room for extension. Do not present extensibility as implementing speculative features; explore which boundaries were worth adding now and which features were deliberately deferred.

User-confirmed constraints: maintainability and time were the driving factors. The approach was pragmatic: get something working while building a stable platform to extend. Treat this as the central thesis, then substantiate it with concrete examples and costs rather than attributing every implementation detail to an explicit personal decision.

## Interview rounds

- Purpose/scope: intended audience (answered), original motivation and first useful version (answered); constraints (answered: maintainability and time); specific deliberate exclusions remain to explore.
- Backend and persistence: Rust/Axum, PostgreSQL/explicit SQL, one crate and separate processes, alternatives and prior experience.
- Synchronization and media: local reads, freshness tolerance, simpler reqwest pagination, per-record commits, timeout diagnosis, image caching and variant URLs.
- Product algorithms and notifications: counter-team objectives, bounded heuristic tradeoffs, meaningful change detection, polling and retained history.
- Frontend and contract: React/TypeScript choices, URL state, explicit saves/revisions, generated types and their maintenance cost.
- Identity and security: anonymous-first scope, ownership/session separation, deferred providers/linking, exact-Origin and cookie boundary.
- Hosting and operations: avoiding a Paid account, standard CloudFront versus flat-rate plans, single-host failure domain, deployment process, backups and cost limits.
- Verification and evolution: fixture/clock seams, integration tests, live failures, what was consciously not built, and what would trigger a redesign.

Ask in small rounds, follow up on meaningful answers, and allow "no strong preference" or "proposed during implementation" rather than inventing a personal rationale for every dependency. Keep the final README selective and readable; put detailed decision notes in docs with links. The main component interview is complete; the consolidated draft is ready for review.

## Validation

Check relative Markdown links after moving the guide and keep references to features/deployment status accurate. Documentation-only work does not need application test reruns. Record interview answers and final editorial decisions as they arrive.

## Current interview state

The main rounds are complete. Confirmed reasoning and deliberately skipped topics are recorded below; the README has been consolidated from those answers. No personal rationale is invented for individual libraries or implementation details that were not discussed. Relative links in the README and moved project guide resolve, and git whitespace checks pass. No application tests were rerun for this documentation-only change.

## Backend interview: Rust

User-confirmed rationale has two parts:
1. Resource efficiency makes a small project practical to deploy on small hardware as a straightforward binary.
2. In AI-assisted development, the strict compiler and comprehensive type system increase confidence and speed validation.

The README now states these reasons without claiming that compilation proves runtime correctness. The existing fixture/database tests provide separate runtime evidence. Do not invent a rejected language/framework or a Rust-specific cost; ask about the tradeoff next. Axum's specific selection is not yet explained separately.

User-confirmed Rust tradeoff: explicitness and verbosity can increase the volume of code to review. For a straightforward, small application, the confidence gained from compiler validation outweighs that overhead. The README now states this cost directly. Do not substitute unmentioned costs such as unfamiliarity, compile times or ecosystem gaps. Next explore persistence and the reasons for PostgreSQL/explicit SQL.

## Persistence interview

User-confirmed reasoning:
- Most database systems would have worked for this project; PostgreSQL was a good option for simple setup and future scaling. Do not imply the workload demanded PostgreSQL or that other systems were ruled out by requirements.
- Explicit SQL was preferred for portability and simplicity. Engineers often know SQL and basic query builders across languages, while each ORM introduces specific concepts and a learning curve.
- Plain queries or portions of queries can be run and debugged independently, then placed in application code.

The README now captures that distinction. Describe familiarity/portability across application tooling without promising database-engine independence: the implementation uses PostgreSQL-specific features. No specific rejected database or ORM has been named, and no migration/ease-of-scaling benchmark was claimed. The next interview topic is the deliberate simplification of sync and the per-record commit boundary.

## Synchronization interview

User-confirmed reasoning:
- API errors and timeouts occurred during development.
- Keeping the fetch loop simple and saving each Pokémon as it arrived maximized useful availability despite those failures.
- The catalog is small and refreshes infrequently, so connection-count/I/O optimization was not a priority.
- A usable product with partial data mattered more than waiting for a complete refresh.

The README now describes availability and transaction granularity as the motivation. Be precise about the implementation: records commit individually, but the worker reuses a dedicated PostgreSQL connection; do not claim it opens a new connection per Pokémon. Readers can observe entries from different refresh points, while each individual record's data/event/revision commit atomically. Do not invent a throughput benchmark or claim parallel fetching would be inherently incorrect. Still explore the local-read/background-refresh boundary and whether the daily interval was a deliberate target or a pragmatic default.

## Refresh interval and upstream stewardship

User-confirmed reasoning: 24 hours was a practical starting point. The case-study prompt described infrequent data changes, and PokéAPI's documentation prefers caching and treats the API as a shared public resource. More frequent refreshes seemed unnecessary and contrary to the spirit of the shared service. Do not claim the daily interval is an upstream requirement or a measured optimum.

Verified https://pokeapi.co/docs/v2 (Information / Fair Use Policy): upstream encourages limiting request frequency to reduce hosting cost and asks consumers to cache requested resources locally. Its documentation says rate limiting was removed, so avoid inventing a hard rate limit as the rationale. README now links this guidance. Next explore counter-team objectives, which were requirements versus implementation choices, and the limits consciously accepted.

## Counter-team product judgment

User-confirmed reasoning: the prompt was very open-ended. The user chose features they would find cool and fun, first type advantage, then stat balancing inspired by an in-game rival with a team appropriate for the player's stage. These ideas were the user's product choices, not prescriptive counter-mode requirements in the original case-study prompt.

README now connects the type/stat model to that experience. Describe game progression as inspiration: the implementation balances against the selected team's base stats and does not track levels, badges, stages or actual game progression. Do not claim the game uses this exact algorithm or that stat balance proves a fair battle. The next follow-up is the intended accuracy/scope boundary and which battle mechanics were consciously deferred.

User-confirmed counter scope: moves, abilities and levels were deliberately left out to keep implementation simple. README now states this as an intentional boundary, with the resulting limit that recommendations are type/stat suggestions rather than battle predictions. Do not attribute an accuracy benchmark, exhaustive search comparison or competitive-play goal to the user. Next distinguish notification requirements from the user's judgments about relevance, noise and delivery timing.

## Skipped topic: notifications

The user requested skipping the notification interview. Removed the dedicated notification decision row from the new README draft; existing behavior remains documented in the moved project guide/API/operations documentation. Do not return to this topic or invent a personal rationale for its requirements. Continue with frontend choices.

## Frontend interview

User-confirmed reasoning: React and TypeScript are the technologies they have seen used most. TypeScript provides similar, though less strict, benefits to Rust for type safety and catching certain classes of issues before runtime. The README now captures familiarity/prevalence and early validation; do not claim measured popularity, equal guarantees to Rust or that types replace runtime tests. React-specific rendering/architectural advantages were not asserted. Next explore identity and which extension boundaries were worth adding before provider integrations.

## Identity and extension boundaries

User-confirmed reasoning: the first goal was a working application, with systems kept as simple as possible. Avoiding external dependencies outside the user's control was particularly valuable at the start. Preserving the possibility of later feature extension was also important.

The README now ties anonymous sessions to getting a working app without a provider dependency, and the owner/device-session split to an extension boundary. Do not imply that real provider login, account linking or cross-device sign-in is implemented, or that all external dependencies were removed. The design deliberately keeps the provider integration deferred. Next explore the single-host deployment and when its tradeoffs would warrant changing.

## Hosting interview

User-confirmed rationale: simplicity and cost, nothing more. Reflect that directly. No personal scaling threshold, availability target or broader cloud strategy was supplied; do not invent one or keep probing this topic. The current implementation's single failure domain and operational limitations can be stated as facts, but not as an elaborate strategy attributed to the user. Next explore what validation beyond the type systems was important for confidence in the case study.

## Verification interview

User-confirmed emphasis: testing matters most for integrations, smoke tests and the frontend/backend connection. Use this to explain what the compiler/type systems cannot establish alone. The README should emphasize connected runtime workflows rather than lead with raw test counts or attribute the fixture/clock strategy itself to a personal preference the user did not express. Those implementation details remain available in the project guide and verification records. Next cover the image-storage/on-demand boundary, then consolidate the draft rather than indefinitely expanding it.

## Sprite-loading interview

User-confirmed reasoning: efficiency was the main concern. Sprite variations are cosmetic, so having defaults ready and allowing later selection without preloading all image data was preferable, especially after early sync-performance issues. The README now states this rationale and the observed dependency/fallback tradeoff without attributing an unmentioned storage benchmark or image-hosting strategy.

## Consolidated README

Consolidated the expanding interview draft into a concise architecture overview and a nine-row decision/tradeoff table. Removed repeated narrative/table explanations. The README covers Rust, React/TypeScript, PostgreSQL/SQL, sync, counter recommendations, identity, media, hosting and validation using confirmed user reasoning. The notification rationale remains skipped at the user's request; its functionality stays in the project guide. Library-specific motivations and unrelated implementation details are not invented or presented as personal decisions.

The user's central story is technical judgment under maintainability/time constraints: early type feedback for AI-assisted development, directly debuggable SQL, availability through incremental persistence, deliberate feature exclusions, deferred provider dependencies, efficient cosmetic assets and simple hosting. The detailed implementation material from the original README remains in docs/project-guide.md with rebased links.

## Final validation

Verified the moved guide matches the original HEAD README exactly apart from its title, a repository-root command note and relative-link rebasing. Checked local Markdown link targets and anchors in both files, and git diff --check passes. The new README is 594 words and contains no setup command blocks; the full prior material remains in the guide. No application code, runtime configuration, deployments or tests were changed. Documentation changes are uncommitted.
