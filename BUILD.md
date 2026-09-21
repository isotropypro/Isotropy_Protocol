# Reproducing the published WASM

The published SHA256 values describe raw `.wasm` bytes. Hashing source text or the `.wasm.gz` transport file is a different check.

## Pinned build recipe

- Host: Windows x86_64 MSVC.
- Rust toolchain: `1.85.0-x86_64-pc-windows-msvc`.
- rustc: `1.85.0 (4d91de4e4 2025-02-17)`, LLVM 19.1.7.
- Target: `wasm32-unknown-unknown`.
- Dependencies: the committed controller and token `Cargo.lock`, with `cargo build --locked`.
- RUSTFLAGS: `-C target-cpu=mvp -C target-feature=+mutable-globals`.
- Release profile: the committed `Cargo.toml` in each crate.
- wasm-opt: **none** for the published artifacts; it is never detected automatically from PATH.
- Controller and token are built separately into separate target directories so the standalone token keeps its entrypoints.

Install prerequisites (Rust/rustup and the Windows C++ build tools must be available):

```powershell
rustup toolchain install 1.85.0-x86_64-pc-windows-msvc --profile minimal
rustup target add wasm32-unknown-unknown --toolchain 1.85.0-x86_64-pc-windows-msvc
```

From either `Mainnet/` or `Testnet/`:

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\build-compatible-wasm.ps1
foreach ($name in @('isotropy_protocol.wasm', 'isotropy_token.wasm')) {
    $published = (Get-FileHash -Algorithm SHA256 ".\artifacts\$name").Hash
    $rebuilt = (Get-FileHash -Algorithm SHA256 ".\artifacts-rebuilt\$name").Hash
    if ($published -ne $rebuilt) { throw "Reproduction mismatch: $name" }
    Write-Host "$name $rebuilt"
}
```

`artifacts-rebuilt/` and `target_compat_wasm/` are disposable local build outputs and are ignored by Git. The helper keeps the tracked `artifacts/` intact by default. Start from a fresh clone or an empty target directory for independent reproduction. Both Mainnet and Testnet contain their own locked dependency trees.

The script sets RUSTFLAGS and temporarily clears CARGO_ENCODED_RUSTFLAGS, restoring both afterward. Avoid external Cargo config or profile/target overrides when reproducing a release. A different OS/compiler, changed lockfiles, custom `-RustToolchain`, or explicit `-WasmOptBin` is a custom build and may produce a different hash. Do not replace published expectations simply to accept a mismatch. `-WasmOptBin` is an explicit experimental override, not part of this verified recipe; record its exact version when using it.

Gzip output can vary with the compression implementation; compare the uncompressed WASM only. This Windows recipe does not claim cross-platform byte reproducibility or provide a pinned container image.

For mainnet, complete the separate live contract check using [Mainnet/VERIFICATION.md](Mainnet/VERIFICATION.md). A successful source build and hash match do not freeze contract state, scheduling or migration permissions.

## Reproduction result (2026-09-21 UTC)

Fresh builds from the public contract source and committed lockfiles, using the recipe above, reproduced all four published raw artifacts byte-for-byte:

| Package | Artifact | SHA256 |
| --- | --- | --- |
| Mainnet | controller | 18b057e34e0069dc3f2703971ee668aac72b1ca4759114129294c7d67fdc39fd |
| Testnet | controller | d82dd29cde957db6ef34303140aa8d7a439079f3c0e254a293b9adc9177faac6 |
| Mainnet and Testnet | token | 8dcbd90908e767984f76eb0df138301ac60cd6136c14e12814b4357f02522b97 |

The contract sources, lockfiles, published WASM and historical payloads were not modified by this verification update.
