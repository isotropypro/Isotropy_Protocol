# Mainnet verification: code and live state

## What a checksum proves

SHA256 identifies the **uncompressed WASM bytes**, not the contract storage, start date, admin permissions, Rust source text or gzip container. Check the chain and contract address first, then resolve its active code ID. A matching artifact alone does not identify the code currently running at an address.

The published controller (code 11405) and token (code 11400) were downloaded from columbus-5 and matched byte-for-byte against this repository on 2026-09-21 around 22:28 UTC. Controller code ID and the scheduled start were also confirmed with a second LCD. Expected addresses and hashes are in [verification-manifest.json](verification-manifest.json).

## Read-only verification

From `Mainnet/` on Windows PowerShell 5.1 or newer:

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\verify-mainnet.ps1
# Repeat against an independent node:
powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\verify-mainnet.ps1 -Lcd https://lcd.terra-classic.hexxagon.io
```

No wallet, key or signing is used. The script verifies:

1. LCD chain ID is columbus-5.
2. Both contract addresses still resolve to the published code IDs.
3. SHA256 of downloaded WASM equals LCD data_hash, the manifest and the local artifact.
4. The controller references the expected token address.

It then reports live `config`, `current_cycle`, `global_state` and both `contract_info.admin` values. Queries use latest state and span the reported block-height range; this is not an atomic snapshot or a light-client proof. For high-assurance review, use your own trusted node and height-pinned queries. An LCD failure is not a checksum mismatch.

The script intentionally fails on a changed code ID: review the migration, source and new artifact before updating the manifest. It does **not** reject a legitimate new start date. Compare live configuration and administrative permissions with your expectations separately. A checksum match is not a security audit and does not imply that the contracts are immutable.

## Start schedule and history

Snapshot checked on 2026-09-21 (UTC):

| Item | Value |
| --- | --- |
| Current cycle | 1 |
| Scheduled cycle 1 start | 1793808000 / 2026-11-04 16:00:00 UTC |
| Cycle duration | 86400 seconds |
| Owner and admin of both contracts | terra10m9z6t2vu7aptvqtglt7wr8hz94rtt57gsmdvs |
| Delayed-start authority | terra15gtmpmr4mwlyuku4ajjr6frshc0dznj34kwgsg |

These are dated observations, not permanent expectations. Always query live state again.

- Migration from 11401 to 11405 set the start to 1781020800 (2026-06-09 16:00 UTC). [Migration transaction](https://finder.terraclassic.community/classic/tx/D751595E93A1E3C764CED5BDDDE04DBE836851140E75336D295A5FD83D2662EB).
- The later confirmed `update_delayed_start` transaction set it to 1793808000 on 2026-07-31 at 13:01:44 UTC, block 29740282, result code 0. [Reschedule transaction](https://finder.terraclassic.community/classic/tx/6CB692BB3BB0EC8D0219160CDCCBAFC721A2929FE32EA655EF518A1653FA8395).
- This is not an exhaustive log of all intermediate reschedules. The queried code history contained the initial deployment and the migration to 11405, with no later migrations.

Historical execute payload (reference only):

```json
{"update_delayed_start":{"start_timestamp":1793808000}}
```

`execute_update_delayed_start` changes `GLOBAL_STATE.current_cycle_start`, not WASM. The current implementation permits owner or delayed-start authority to move the date in either direction only while cycle ID is 1 and the block time is before the currently scheduled start. The new timestamp cannot be earlier than the block time. After launch this operation is locked under the current implementation; existing migration-admin powers must still be considered separately.

Keep [examples/controller-migrate.json](examples/controller-migrate.json) unchanged as the historical migration payload. Record later reschedules as execute transactions. A reschedule needs a refreshed dated schedule/transaction record, **not a new code checksum**. A different resulting WASM binary requires a new checksum; a deployment or migration also requires updated address/code-ID/release records, even if bytes happen to be identical.

## Source-to-WASM reproduction

See [BUILD.md](../BUILD.md) for the pinned build and limitations. This is a separate step from verifying already published binaries against the chain.
