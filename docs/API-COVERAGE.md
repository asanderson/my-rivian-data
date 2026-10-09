# Rivian API coverage and provenance

[Owner guide](OWNER-GUIDE.md) · [Developer guide](DEVELOPER-GUIDE.md)

Review snapshot: 2026-10-09. These are community-documented, unofficial contracts.
The implementation contains fixed operation documents and native transport; a
catalog status of `live` means the code admits that read, **not** that it has been
accepted by Rivian for a real account. No owner-account acceptance test has yet
been performed. Vehicle generation, region, account roles and upstream changes
may affect availability.

The initial goal of “all API requests” is tracked as a coverage goal. This release
does not claim complete coverage of an undocumented, changing service. A separate
developer view exposes the implemented operations without providing arbitrary
GraphQL, mutation signing or a generic network proxy.

## Authentication and session scope

The native transport implements the CSRF/app-session bootstrap, password login,
pending verification and OTP login documented by
[RivDocs authentication](https://rivian-api.kaedenb.org/app/authentication/).
It uses the `csrf-token`, `a-sess` and `u-sess` protocol values internally. Those
values, the upstream access token and refresh token are never a browser API result.
[RivDocs logout](https://rivian-api.kaedenb.org/app/account/logout/) documents the
remote logout operation.

There is no invented refresh-token grant: the reviewed Python client stores a
`refreshToken` but does not establish a refresh-token exchange contract. App/CSRF
session renewal can retain the active user session; rejection of that user session
requires sign-in again. Passwords are not retained for automatic re-login. Tokens
are session-only native state, without disk persistence or an OS-vault fallback.

The app rotates its application session hourly. An unauthenticated read can
rotate that session once and retry once; an invalid user session requires a new
login. Password sign-in is limited to five attempts per 15 minutes, with at least
three seconds between authentication attempts. Pending MFA expires after ten
minutes or five attempts. Upstream rate-limit responses pause requests for five
minutes. These are local bounds, not a promise to avoid every upstream restriction.

## Executable read inventory

The authoritative documents and operation-name mapping are in
[`crates/rivian-core/src/live.rs`](../crates/rivian-core/src/live.rs).
All rows below have a fixed document; extra browser-supplied fields are rejected.
A vehicle-scoped operation additionally requires native account ownership.

| Local operation | Upstream operation | Route | Result |
| --- | --- | --- | --- |
| `account-summary` | `getUserInfo` | Gateway | Normalized account summary |
| `list-vehicles` | `getUserInfo` | Gateway | Account vehicles; missing telemetry is null |
| `vehicle-state` | `GetVehicleState` | Gateway | Normalized owner readings with individual source times |
| `charging-status` | `GetVehicleState` | Gateway | Charge, limit and state; unsupported power remains null |
| `vehicle-location` | `GetVehicleState` | Gateway | Coordinates and source observation time |
| `charging-history` | `getCompletedSessionSummaries` | Charging | Up to 100 sessions explicitly assigned to the selected vehicle |
| `vehicle-telemetry` | `GetVehicleState` | Gateway | Fixed detailed raw readings, including closures and status fields |
| `live-charging-session` | `getLiveSessionData` | Charging | Available current-session measurements |
| `account-charging-history` | `getCompletedSessionSummaries` | Charging | Account-wide sessions, including sessions without vehicle IDs |
| `charging-schedules` | `getVehicleChargingSchedules` | Gateway | Existing schedules; no changes |
| `registered-wallboxes` | `getRegisteredWallboxes` | Charging | Registered charger details and reported status |
| `supported-features` | `SupportedFeatures` | Gateway | Feature flags for account vehicles |
| `vehicle-connection` | `GetVehicleLastConnection` | Gateway | Reported cloud synchronization time |
| `ota-updates` | `getOTAUpdateDetails` | Gateway | Current/available release-note metadata; no installation |
| `vehicle-images` | `getVehicleImages` | Gateway | Image metadata and URLs; app does not fetch the images |
| `drivers-and-keys` | `DriversAndKeys` | Gateway | Invited drivers and enrollment metadata; no keys enrolled or signing performed |
| `account-profile` | `MyRivianDataProfile` | Orders | Fixed profile fields from the documented `user` schema |
| `vehicle-orders` | `MyRivianDataOrders` | Orders | Fixed order summary fields from the documented `user` schema |

The last two operation names are this project's GraphQL document names. The
selected schema fields come from the reviewed upstream orders schema; these are
not presented as captured Rivian mobile operation names.

Fixed HTTPS routes:

- Gateway: `https://rivian.com/api/gql/gateway/graphql`
- Charging: `https://rivian.com/api/gql/chrg/user/graphql`
- Orders: `https://rivian.com/api/gql/orders/graphql`

The client does not accept a destination URL from the browser. Verified rustls
roots, disabled redirects and disabled automatic system proxies constrain the
transport. Each upstream request has a 5-second connect timeout and 10-second
overall timeout, with a 1 MiB streamed body limit. The local response envelope can
contain both normalized and raw data and is limited to 3 MiB in the browser.
Errors are sanitized. Personal data in successful responses,
including VINs, contact details, locations and driver metadata, should be reviewed
before sharing a developer response.

## Normalization rules

- Missing readings remain `null`; no demo values are substituted for a live error.
- An aggregate vehicle observation time remains unknown when readings have
  different ages. `field_observed_at` preserves the supplied per-reading timestamps.
- Vehicle mileage is converted from meters to kilometers. Reported range is
  already in kilometers; no speculative second conversion is applied.
- A locked reading means the four passenger doors report locked. An explicitly
  unlocked door yields false; incomplete lock readings yield null. This is not a
  claim that every vehicle opening is secured.
- Charging history is filtered by an exact returned vehicle ID. Sessions without
  that association are counted as excluded and remain available through the
  account-wide developer operation; they are not guessed onto a selected vehicle.
- A reported vehicle ID mismatch rejects the response. The legacy VehicleState
  schema does not provide an `id` field, so that request is bound to the natively
  authorized input ID; the implementation does not invent an unsupported echo.
- Raw `getVehicle` reads request the schema-supported `id` and require it to match
  the authorized vehicle.

## Unavailable capabilities

| Capability | Current boundary |
| --- | --- |
| Lock / unlock and charge-limit changes | Explicitly blocked; no reviewed command authority, enrollment or signing |
| Parallax/WebSocket subscriptions | Explicitly blocked; one-shot reads only |
| Arbitrary GraphQL documents or URLs | Not accepted by the adapter |
| Phone-key enrollment, BLE pairing, key export | Not implemented |
| Persistent history, background collection, export | Not implemented |
| Remembered account sessions | Not implemented; no plaintext credential fallback |
| Android/iOS application host | Phase 2 Tauri work |

Blocked catalog entries make known gaps visible. They are not executable simply
because a developer calls `/api/execute` directly.

## Primary sources

| Source | Version / use |
| --- | --- |
| [rivian-python-client](https://github.com/bretterer/rivian-python-client/tree/4d15dd88e74cf1a0be0bd23f46565fe89b48af44) | Commit `4d15dd88e74cf1a0be0bd23f46565fe89b48af44` (2026-10-05); comparison of authentication headers, fixed query documents, charging behavior and schemas |
| [Client implementation](https://github.com/bretterer/rivian-python-client/blob/4d15dd88e74cf1a0be0bd23f46565fe89b48af44/src/rivian/rivian.py) | Gateway, charging and vehicle read operation reference |
| [Orders schema](https://github.com/bretterer/rivian-python-client/blob/4d15dd88e74cf1a0be0bd23f46565fe89b48af44/src/rivian/schemas/orders.graphql) | Profile and order selection sets |
| [RivDocs authentication](https://rivian-api.kaedenb.org/app/authentication/) | Sign-in, pending OTP, Android/Apollo headers and application/user sessions; consulted 2026-10-09 |
| [RivDocs account](https://rivian-api.kaedenb.org/app/account/user-info/) | Account/vehicle relationship |
| [RivDocs vehicle state](https://rivian-api.kaedenb.org/app/legacy/vehicle-state/) | Legacy telemetry fields and timestamps |
| [RivDocs completed charging sessions](https://rivian-api.kaedenb.org/app/legacy/charging/get-completed-session-summaries/) | Completed-session response fields |
| [RivDocs supported features](https://rivian-api.kaedenb.org/app/legacy/vehicle-info/supported-features/) | Account vehicle feature queries |
| [RivDocs last connection](https://rivian-api.kaedenb.org/app/legacy/vehicle-info/vehicle-last-connection/) | Cloud connection read |
| [RivDocs Parallax](https://rivian-api.kaedenb.org/app/parallax/) | Subscription/signing context and unit interpretation; not an implemented stream |

RivDocs pages are community references, not a Rivian compatibility guarantee or a
versioned official SDK. The Rust implementation is original. The reviewed Python project declares MIT in its pinned `pyproject.toml`; that
source tree has no standalone license file. No Python implementation or test
fixture was copied. Its provenance and declared license are recorded in
[third-party notices](THIRD-PARTY-NOTICES.md).
