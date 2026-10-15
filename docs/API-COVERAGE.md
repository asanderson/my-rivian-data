# Rivian API coverage and provenance

[Owner guide](OWNER-GUIDE.md) · [Developer guide](DEVELOPER-GUIDE.md)

Review snapshot: 2026-10-14 (`2026-10-14.1`). These are community-documented, unofficial contracts.
The implementation contains fixed operation documents and native transport; a
catalog status of `live` means the code admits that fixed operation, **not** that it has been
accepted by Rivian for a real account. No owner-account acceptance test has yet
been performed. Vehicle generation, region, account roles and upstream changes
may affect availability.

The versioned [known-operation snapshot](../crates/rivian-core/data/operations-2026-10-14.json)
records 234 source surfaces: 69 named GraphQL documents, 44 schema root fields,
75 concrete Parallax RVMs and 46 legacy command names. It covers the union of
the pinned RivDocs App API pages and Python client contracts, rather than every
undocumented surface of a changing service. Each record includes source commits,
endpoint, document or schema references, implementation and verification status,
prerequisites, retry policy and an explicitly unknown last-tested firmware.

The developer view exposes implemented, documented and blocked entries. Inventory
membership grants no execution. Imported documents must match the exact endpoint,
operation name and document bytes of a reviewed local operation; their variables
are then validated separately and authorized natively. Authentication documents
that return session secrets are excluded from raw browser execution and their
document selections are redacted from the catalog.

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
requires sign-in again. Passwords are not retained for automatic re-login. Sessions
are memory-only by default. Opt-in remembered sign-in uses the native OS credential
store; an unavailable or locked store has no plaintext fallback. The browser never
receives the stored secret bundle. OS/device acceptance is tracked separately in
[status](STATUS.md).

The app rotates its application session hourly. An unauthenticated read can
rotate that session once and retry once; an invalid user session requires a new
login. Password sign-in is limited to five attempts per 15 minutes, with at least
three seconds between authentication attempts. Pending MFA expires after ten
minutes or five attempts. Upstream rate-limit responses pause requests for five
minutes. These are local bounds, not a promise to avoid every upstream restriction.

## Executable fixed-operation inventory

The authoritative documents and operation-name mapping are in
[`crates/rivian-core/src/live.rs`](../crates/rivian-core/src/live.rs).
All rows below have a fixed document; extra browser-supplied fields are rejected.
A vehicle-scoped operation additionally requires native account ownership.
Order and wallbox reads require fresh native account-resource checks; a valid
browser-supplied ID never establishes ownership. There are 33 fixed reads and one
confirmed nickname write. The separately controlled native stream admits eleven
reviewed Parallax read topics; it does not run through the HTTP executor.

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
| `current-user` | `CurrentUser` | Gateway | Account contact details, units and sharing preferences |
| `payment-methods` | `paymentMethods` | Orders | Type, card brand and last four digits; no full card number |
| `camp-speakers` | `GetProvisionedCampSpeakers` | Gateway | Connected camp speaker records for account vehicles |
| `vehicle-names` | `VehicleNames` | Gateway | Account vehicle nickname settings |
| `user-pin-settings` | `GetUserPinAuth` | Gateway | Driver profile lock settings; no PIN value |
| `user-pin-exists` | `GetUserPinExists` | Gateway | Whether a driver profile PIN exists; no PIN value |
| `linked-charging-account` | `CheckByRivianId` | Charging | Linked ChargePoint account status |
| `linked-charging-email` | `getLinkedEmailForRivianId` | Charging | Email for the account's linked ChargePoint service |
| `non-rivian-charging-session` | `getNonRivianUserSession` | Charging | Current account session; no inferred vehicle association |
| `delivery-status` | `delivery` | Gateway | Delivery details for a natively authorized account order |
| `finance-summary` | `financeSummary` | Payments | Financing information for a natively authorized account order |
| `transaction-status` | `transactionStatus` | Payments | Transaction steps for a natively authorized account order |
| `order-details` | `MyRivianDataOrderDetails` | Orders | Fixed order totals and vehicle fields; native order authority |
| `wallbox-status` | `getWallboxStatus` | Charging | Status for a natively authorized registered wallbox |
| `charging-sites` | `chargingSites` | Gateway | Bounded map search, explicit coordinates and CCS/NACS vehicle port |
| `rename-vehicle` | `SetVehicleName` | Gateway | Vehicle nickname write with native ownership and one-use confirmation |

The `MyRivianData*` operation names are this project's GraphQL document names. The
selected schema fields come from the reviewed upstream orders schema; these are
not presented as captured Rivian mobile operation names.

Fixed HTTPS routes:

- Gateway: `https://rivian.com/api/gql/gateway/graphql`
- Charging: `https://rivian.com/api/gql/chrg/user/graphql`
- Orders: `https://rivian.com/api/gql/orders/graphql`
- Payments: `https://rivian.com/api/gql/t2d/graphql`

The nickname write requires a native confirmation bound to the current account,
local operation and exact variables. It expires after two minutes and is consumed
once, including an ambiguous upstream failure. Direct execution without that
confirmation is rejected; no mutation is automatically retried. The portable core
can construct a reviewed document, but cannot grant account authority or mint a
confirmation.

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
| Parallax/WebSocket subscriptions | Eleven fixed read topics in the protected native stream; other topics remain inventoried and unavailable for execution |
| Arbitrary GraphQL documents or URLs | Rejected; exact reviewed-document imports resolve to local allowlisted operations |
| Phone-key enrollment, BLE pairing, key export | Not implemented |
| Persistent history, background collection, export | Not implemented |
| Remembered account sessions | Opt-in native OS credential storage; no plaintext fallback and OS/device acceptance remains separate |
| Android/iOS application host | Phase 2 Tauri work |

Documented and blocked catalog entries make known gaps visible. They are not executable simply
because a developer calls `/api/execute` directly.

## Primary sources

| Source | Version / use |
| --- | --- |
| [Current Python client](https://github.com/bretterer/rivian-python-client/tree/7d6028233752cc226faeae35e9d12ce387abb2fe) | Commit `7d6028233752cc226faeae35e9d12ce387abb2fe` (2026-10-14); complete named-operation, schema-root, command and registered-RVM inventory |
| [Pinned RivDocs source](https://github.com/kaedenbrinkman/rivian-api/tree/2eb98b4b50b26d9f80a190f65107d4bd9d828cd9/app) | Commit `2eb98b4b50b26d9f80a190f65107d4bd9d828cd9` (2026-10-10); all App API operation documents and Parallax domain topics; reviewed 2026-10-14 |
| [rivian-python-client](https://github.com/bretterer/rivian-python-client/tree/4d15dd88e74cf1a0be0bd23f46565fe89b48af44) | Commit `4d15dd88e74cf1a0be0bd23f46565fe89b48af44` (2026-10-05); comparison of authentication headers, fixed query documents, charging behavior and schemas |
| [Client implementation](https://github.com/bretterer/rivian-python-client/blob/4d15dd88e74cf1a0be0bd23f46565fe89b48af44/src/rivian/rivian.py) | Gateway, charging and vehicle read operation reference |
| [Orders schema](https://github.com/bretterer/rivian-python-client/blob/4d15dd88e74cf1a0be0bd23f46565fe89b48af44/src/rivian/schemas/orders.graphql) | Profile and order selection sets |
| [RivDocs authentication](https://rivian-api.kaedenb.org/app/authentication/) | Sign-in, pending OTP, Android/Apollo headers and application/user sessions; consulted 2026-10-09 |
| [RivDocs account](https://rivian-api.kaedenb.org/app/account/user-info/) | Account/vehicle relationship |
| [RivDocs vehicle state](https://rivian-api.kaedenb.org/app/legacy/vehicle-state/) | Legacy telemetry fields and timestamps |
| [RivDocs completed charging sessions](https://rivian-api.kaedenb.org/app/legacy/charging/get-completed-session-summaries/) | Completed-session response fields |
| [RivDocs supported features](https://rivian-api.kaedenb.org/app/legacy/vehicle-info/supported-features/) | Account vehicle feature queries |
| [RivDocs last connection](https://rivian-api.kaedenb.org/app/legacy/vehicle-info/vehicle-last-connection/) | Cloud connection read |
| [RivDocs Parallax](https://rivian-api.kaedenb.org/app/parallax/) | Subscription/signing context and unit interpretation; native streaming is restricted to eleven independently reviewed read topics |

RivDocs pages are community references, not a Rivian compatibility guarantee or a
versioned official SDK. The Rust implementation is original. The reviewed Python project declares MIT in its pinned `pyproject.toml`; that
source tree has no standalone license file. No Python implementation or test
fixture was copied. Its provenance and declared license are recorded in
[third-party notices](THIRD-PARTY-NOTICES.md).
