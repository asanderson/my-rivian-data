# Synthetic validation fixtures

`validation-cases.json` contains no account data, credentials, tokens, real vehicle IDs,
or captured Rivian responses. Its operation IDs are local prototype aliases. All
vehicle values and fixed timestamps in the core are synthetic.

Each case supplies a `name`, `operation_id`, exact `variables_json` string, and
`expected_valid`. Keep input as a string so malformed JSON can be tested without
first parsing it in JavaScript. Cases cover both demo vehicle IDs, integer range
boundaries, foreign IDs, missing fields, unknown fields, invalid types, malformed
JSON, excessive depth, and disabled operations.

The native fixture runner emits the full catalog and complete validation results:

```sh
cargo run --quiet -p rivian-core --example validation_fixture
```

Compare those parsed results with `catalog_json()` and
`validate_request_json(operation_id, variables_json)` from the compiled WASM module.
The Rust wrapper unit test is an additional check; it does not establish execution
parity in an actual WebAssembly runtime.

Core tests separately cover raw JSON size (16 KiB including whitespace), structural
node count (256), nesting depth (8), static error messages, synthetic-result labels,
and the inability to execute mutation or subscription operations. The native host
must validate again before execution regardless of browser validation.
