# Isotropy Protocol Public Contract Package

Public reviewer bundle for the Isotropy Protocol CosmWasm contracts on Terra Classic.

This package is meant for:

- contract source review
- artifact reproduction
- `cargo test` verification
- SHA256 comparison against published wasm files

It is intentionally limited to contract-facing material and excludes operator-only infrastructure.

## Quick Start

Choose the package you want to inspect:

- `Mainnet/` - production contract package for `columbus-5`
- `Testnet/` - rehearsal package for `rebel-2`

Then:

1. Open the package README.
2. Run `cargo test`.
3. Rebuild artifacts with `scripts/build-compatible-wasm.ps1`.
4. Compare SHA256 hashes with the published values.

## What Is Included

- controller contract source
- CW20 token contract source
- wasm artifacts
- example instantiate and migrate payloads
- reproducible build helper
- package-specific notes for mainnet and testnet

## What Is Not Included

- operator secrets
- keeper mnemonics
- private infrastructure details
- server deployment files
- full rollout tooling that is not required for contract review

## Protocol Snapshot

Isotropy Protocol uses two CosmWasm contracts:

1. Controller contract
2. CW20 token contract

Core mechanics:

- users burn native LUNC in batches
- burns are tracked per cycle
- each completed cycle mints the next `isLUNC` emission
- earned allocation is auto-staked for burners
- allocation fees and protocol fees are accounted on-chain
- stakers earn native LUNC rewards

## Package Layout

### Mainnet

- path: `Mainnet/`
- docs: `Mainnet/README.md`
- review entrypoints: `Mainnet/src/contract.rs`, `Mainnet/src/msg.rs`, `Mainnet/token/src/lib.rs`

### Testnet

- path: `Testnet/`
- docs: `Testnet/README.md`
- review entrypoints: `Testnet/src/contract.rs`, `Testnet/src/msg.rs`, `Testnet/token/src/lib.rs`

## Network Snapshot

### Mainnet

- chain: `columbus-5`
- controller: `terra1ad5cva3hv82zg6p36n6vhszsd7ftznem85tgv94w2pmu332gc3jsqs9kdk`
- token: `terra1h9dg99v9nt22zsvd959cjtttmfp8x0paqtuegwecsjd3py2f9mqq5vrss3`
- controller SHA256: `0a0831ef37349bdb464b975fbe3655223ee7e5bfee1c66bc55b20a02c2faa9d0`
- token SHA256: `8dcbd90908e767984f76eb0df138301ac60cd6136c14e12814b4357f02522b97`

### Testnet

- chain: `rebel-2`
- controller: `terra1myezsr25f693td5jf40lvxhxlrqyphrgtsyqgkjsfsr7dmpr9jusxjjfz2`
- token: `terra1dkxyuqxfawdjaqh5pjdnxvxkfw6nwjtsl4rnqrf6xdlpkhmdf0qq9jprca`
- controller SHA256: `d82dd29cde957db6ef34303140aa8d7a439079f3c0e254a293b9adc9177faac6`
- token SHA256: `8dcbd90908e767984f76eb0df138301ac60cd6136c14e12814b4357f02522b97`

## Reviewer Flow

### Mainnet

From `for_Github_public\Mainnet`:

```powershell
cargo test
cargo run --bin schema
powershell -ExecutionPolicy Bypass -File .\scripts\build-compatible-wasm.ps1
Get-FileHash .\artifacts\isotropy_protocol.wasm -Algorithm SHA256
Get-FileHash .\artifacts\isotropy_token.wasm -Algorithm SHA256
```

### Testnet

From `for_Github_public\Testnet`:

```powershell
cargo test
cargo run --bin schema
powershell -ExecutionPolicy Bypass -File .\scripts\build-compatible-wasm.ps1
Get-FileHash .\artifacts\isotropy_protocol.wasm -Algorithm SHA256
Get-FileHash .\artifacts\isotropy_token.wasm -Algorithm SHA256
```

## Notes

- package-specific math notes live in `Mainnet/README.md` and `Testnet/README.md`
- testnet uses the same core per-cycle math but a much shorter cycle duration
- examples in `Mainnet/examples/` and `Testnet/examples/` help reconstruct payload shapes without private deployment tooling
