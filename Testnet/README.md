# Isotropy Protocol Testnet Public Package

This directory contains the public testnet contract package for Terra Classic `rebel-2`.

Included:

- controller source in `src/`
- token source in `token/`
- testnet wasm artifacts in `artifacts/`
- example payloads in `examples/`
- reproducible build helper in `scripts/build-compatible-wasm.ps1`

Not included:

- operator secrets
- keeper credentials
- private rollout details

## Testnet Deployment

- Chain ID: `rebel-2`
- Controller code ID: `2317`
- Previous controller code ID: `2316`
- Controller address: `terra1myezsr25f693td5jf40lvxhxlrqyphrgtsyqgkjsfsr7dmpr9jusxjjfz2`
- Controller wasm SHA256: `d82dd29cde957db6ef34303140aa8d7a439079f3c0e254a293b9adc9177faac6`
- Token code ID: `2315`
- Token address: `terra1dkxyuqxfawdjaqh5pjdnxvxkfw6nwjtsl4rnqrf6xdlpkhmdf0qq9jprca`
- Token wasm SHA256: `8dcbd90908e767984f76eb0df138301ac60cd6136c14e12814b4357f02522b97`
- Burn denom: `uluna`
- Cycle duration: `600` seconds
- Initial cycle start timestamp: `1780434276`

## Protocol Math

- `1 batch = 10,000 LUNC`, with burns accounted in `uluna`
- protocol fee default is `10%`, with a minimum allowed floor of `0.01%`
- allocation fee decreases linearly from `150%` at batch `1` to `20%` at batch `10,000`
- each completed cycle mints the next `isLUNC` emission and auto-stakes earned allocation for burners
- emission still runs for `4002` cycles with decay rate `0.0979396297851485%` per cycle
- unlike mainnet, one testnet cycle is only `600` seconds, so the same per-cycle math is compressed into a much faster wall-clock schedule for rehearsal and validation

## Build And Test

From `for_Github_public\Testnet`:

```powershell
cargo test
cargo run --bin schema
powershell -ExecutionPolicy Bypass -File .\scripts\build-compatible-wasm.ps1
```

## SHA Verification

```powershell
Get-FileHash .\artifacts\isotropy_protocol.wasm -Algorithm SHA256
Get-FileHash .\artifacts\isotropy_token.wasm -Algorithm SHA256
```

Expected:

- `isotropy_protocol.wasm`: `d82dd29cde957db6ef34303140aa8d7a439079f3c0e254a293b9adc9177faac6`
- `isotropy_token.wasm`: `8dcbd90908e767984f76eb0df138301ac60cd6136c14e12814b4357f02522b97`

## Review Notes

- The controller logic lives in `src/contract.rs`.
- Message and query types live in `src/msg.rs`.
- Persistent state layout lives in `src/state.rs`.
- The CW20 token implementation lives in `token/src/lib.rs`.
- This package is intentionally limited to contract review and artifact reproduction.
