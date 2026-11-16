# Stable users and per-device sessions

The user requested an extensible user/session foundation for future authentication providers, with provider implementations deferred. They explicitly chose separating stable users from device sessions now, preserving existing cookies and teams. This discussion satisfies the repository requirement to review identity-schema decisions before implementing them.

## Foundation implemented now

Keep the existing `owners.id` UUID as the stable application user/authorization identifier and retain the `owner_id` API/team contract. This avoids renaming every ownership relation or coupling saved data to a provider. Move hashed bearer credentials into `owner_sessions`, with a distinct session UUID, owner foreign key, creation/last-use timestamps and nullable revocation time. Multiple sessions may belong to one owner; expiration/revocation of one device does not terminate others. The anonymous one-year inactivity lifetime, cookie name and flags, explicit Get started, exact-Origin validation and session response remain compatible.

Migration 0004 backfills one session per existing owner using the same credential hash and timestamps before removing the old owner credential column. Existing raw cookies therefore continue to authenticate the same UUID and own the same teams. Owners remain durable records even when sessions expire. The migration must be paired with updated API code; older API versions query the removed column. Do not deploy/apply it to a running old API. Production rollout requires the normal backup/migration/API update sequence; this task prepares and tests code without deploying it.

Centralize session authentication, issuance for an existing owner and owner-scoped revocation in a small backend module. Anonymous registration creates its owner/session atomically. Future trusted provider callbacks can resolve a stable owner and issue a separate device credential through the same helpers. Issuance for an existing owner is a trusted server-side operation, never a public endpoint taking an owner UUID or a client-asserted provider identity. Raw credentials remain private cookie material, not JSON fields or stored database values. No provider trait/registry, SDK, token-verification implementation, login UI or AWS resource is needed at this stage.

## Deferred provider integration

A provider adapter must validate the real login proof (for OIDC: issuer, signatures, audience, expiry, state/nonce and the appropriate code/PKCE flow) before identifying a user. Resolve a verified provider identity to the application UUID through a unique provider identity mapping; for OIDC the stable key is issuer + subject, not email. Do not treat a caller-supplied UUID, email or unverified token as authentication. Provider choice and exact adapter interface stay open until a concrete integration exists.

Device A and device B each receive independent app session cookies after signing in to the same provider identity. They share the stable owner and saved teams; the bearer cookie itself is not copied across devices. Browser clients continue using same-origin HttpOnly cookies rather than storing provider tokens locally.

Account linking, anonymous-team attachment/merging, credential rotation on authenticated login, logout/device-management endpoints, revocation policies, recovery and authenticated-session lifetimes need a concrete provider/user-flow design later. In particular, never silently merge users because emails match, and never reassign anonymous teams to an existing account without an explicit reviewed flow. The current anonymous cookie-clearing limitation remains accurate until provider login/linking actually exists.

Standards informing the future boundary:
- https://openid.net/specs/openid-connect-core-1_0.html#ClaimStability
- https://cheatsheetseries.owasp.org/cheatsheets/Session_Management_Cheat_Sheet.html

## Validation

Cover migration preservation of legacy cookies/teams, atomic anonymous creation and session reuse, multiple distinct sessions seeing the same teams, per-device idle expiry/renewal, scoped revocation, invalid/revoked credentials and rejection of client-requested owner selection. Keep existing ownership, notifications, counter and frontend flows passing. No production migration or deployment is part of this task.

## Results

Implemented migration `0004_owner_sessions.sql`, the small `identity::sessions` helper module, atomic anonymous registration and session-based owner lookup. Provider adapters/bindings and browser sign-in remain deferred. Existing API responses/cookie names and all team ownership queries are unchanged. The new helpers return SQLx errors to callers; HTTP handlers retain the existing database-error mapping.

All 31 backend tests passed (11 unit, 20 integration). New coverage verifies legacy hash/timestamp/team preservation; independent credentials for one owner; per-device inactivity expiry and sliding renewal; owner-scoped/idempotent revocation; hashed storage; refusal to trust request-body owner/provider claims; and rollback of a newly inserted anonymous owner when session insertion fails. Existing ownership, notifications, sync, counters, sprite and filter tests remain green. Clippy, Rust formatting, OpenAPI drift, frontend TypeScript/generated-type drift and git whitespace checks passed. No public API contract or frontend behavior changes required regenerated artifacts.

No migration was applied to the local development catalog or public AWS database, and no application service was restarted/deployed. The existing running app remains on the prior session storage until an explicit rollout. Changes are uncommitted for review.
