# Isotropy Protocol Mainnet Public Package

This directory contains the public mainnet contract package for Terra Classic `columbus-5`.

Included:

- controller source in `src/`
- token source in `token/`
- mainnet wasm artifacts in `artifacts/`
- example payloads in `examples/`
- reproducible build helper in `scripts/build-compatible-wasm.ps1`

Not included:

- operator secrets
- keeper credentials
- private rollout details

## Mainnet Deployment

- Chain ID: `columbus-5`
- Controller code ID: `11405`
- Previous controller code ID: `11401`
- Migration tx hash: `D751595E93A1E3C764CED5BDDDE04DBE836851140E75336D295A5FD83D2662EB`
- Controller address: `terra1ad5cva3hv82zg6p36n6vhszsd7ftznem85tgv94w2pmu332gc3jsqs9kdk`
- Controller wasm SHA256: `18b057e34e0069dc3f2703971ee668aac72b1ca4759114129294c7d67fdc39fd`
- Token code ID: `11400`
- Token address: `terra1h9dg99v9nt22zsvd959cjtttmfp8x0paqtuegwecsjd3py2f9mqq5vrss3`
- Token wasm SHA256: `8dcbd90908e767984f76eb0df138301ac60cd6136c14e12814b4357f02522b97`
- Burn denom: `uluna`
- Cycle duration: `86400` seconds
- Initial cycle start timestamp: `1781020800`
- Delayed start authority: `terra15gtmpmr4mwlyuku4ajjr6frshc0dznj34kwgsg`

The mainnet controller was migrated in place, so the contract address stayed the same while the active code changed from `11401` to `11405`.

## Protocol Math

- `1 batch = 10,000 LUNC`, with burns accounted in `uluna`
- protocol fee default is `10%`, with a minimum allowed floor of `0.01%`
- allocation fee decreases linearly from `150%` at batch `1` to `20%` at batch `10,000`
- each completed cycle mints the next `isLUNC` emission and auto-stakes earned allocation for burners
- emission runs for `4002` cycles with decay rate `0.0979396297851485%` per cycle
- on mainnet, one cycle is `86400` seconds, so the emission schedule is effectively daily

## Build And Test

From `for_Github_public\Mainnet`:

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

- `isotropy_protocol.wasm`: `18b057e34e0069dc3f2703971ee668aac72b1ca4759114129294c7d67fdc39fd`
- `isotropy_token.wasm`: `8dcbd90908e767984f76eb0df138301ac60cd6136c14e12814b4357f02522b97`

## Review Notes

- The controller logic lives in `src/contract.rs`.
- Message and query types live in `src/msg.rs`.
- Persistent state layout lives in `src/state.rs`.
- The CW20 token implementation lives in `token/src/lib.rs`.
- `examples/controller-migrate.json` matches the executed mainnet migration payload for code `11405`.
- This package is intentionally limited to contract review and artifact reproduction.
