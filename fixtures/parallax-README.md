# Parallax wire fixtures

`parallax-readings.json` contains original synthetic readings encoded with the
documented protobuf field numbers and units. It contains no vehicle/account data,
credentials, captured private responses, copied implementation or generated code.

Schemas were reviewed on 2026-10-14 at
[rivian-python-client revision 7d6028233752cc226faeae35e9d12ce387abb2fe](https://github.com/bretterer/rivian-python-client/tree/7d6028233752cc226faeae35e9d12ce387abb2fe/src/rivian/parallax/proto).
The `body.proto`, `comfort.proto`, `dynamics.proto`, `energy.proto` and
`charging.proto` files provide the field-number references. They are community
reverse engineering; these tests prove decoder behavior, not account acceptance.

`rivian-core::parallax` tests apply the same fixture messages in order and compare
the normalized owner readings. The fixture's `timestamp` is an original message
observation, never local retrieval time. Inline tests additionally cover field
absence, explicit zero, source fix timestamps, partial locks, invalid tires,
malformed lengths/types/nesting, stale messages and bounded unknown-topic storage.

Physical measurements require their wire tags. In particular, an absent GNSS
coordinate does not become zero, though proto3 serialization may omit a legitimate
zero coordinate. Tire validity is a control enum whose documented proto3 default
zero means valid; the synthetic valid tire entries omit that control tag as a
normal serializer does. Pressure and status themselves still require wire tags.

The current [RivDocs energy](https://rivian-api.kaedenb.org/app/parallax/domains/energy/)
and [charging](https://rivian-api.kaedenb.org/app/parallax/domains/charging/) notes
disagree with the pinned client schema on battery range and charge-time fields.
Range is decoded from `dynamics.vehicle.range` in kilometres, and charge time from
the pinned validity/remaining-minutes fields. The published RivDocs battery sample
has an invalid nested message length and is a rejection case, not a valid reading
fixture. Odometer values are kilometres; tire pressures are bar; temperature is °C;
charging power is kW and is hidden after the charge state stops reporting charging.
