# Module A ownership and publication — core v0.1.2

Issue: #40. This is an internal safety contract, not a Download Manager release.

## Ownership

- `StateStore::open` acquires a non-blocking exclusive OS lock **before** opening or migrating SQLite. The handle stays alive until the store is dropped, after the SQLite connection closes.
- The marker is `<canonical-database-path>.pipeline.lock`. Canonicalization joins directory/symlink aliases to the same identity. Use one database path; do not make hard-link copies/aliases of a live SQLite database.
- Each execution entry point acquires ownership of its actual output/log/temp roots before changing checkpoints or incrementing attempts. Acquisition and conversion also lock the parent of the persisted SRA accession directory. Finalization locks the FASTQ root used by conversion.
- Root markers are `.pipeline-owner.lock`. Multiple equal roots are canonicalized and deduplicated. Acquisition is non-blocking; contention returns typed `OwnershipError::Busy` with an action to wait/close the other job and Resume/Retry. It does not mark a healthy owner's run FAILED.
- Root ownership is conservative: independent databases sharing a root cannot write there concurrently. Separate owned roots may run independently. Future CLI/Tauri commands use these core entry points and retain one live store; they must not duplicate locking/state logic.
- OS locks release when the handle closes or its process terminates. Marker files remain. Their existence alone does not mean a job is active. **Never remove, rename, replace, truncate, or force-unlock a live marker.** There is no PID-based stale-file deletion.
- `open_in_memory` is an isolated test store; it does not coordinate a shared database.

## Publication

FASTQ directories, compressed directories, and `SHA256SUMS` all use one `publish_noreplace` adapter. An existence check is only an early diagnostic; the publication operation itself refuses replacement.

| Target | Primitive | Required behavior |
|---|---|---|
| Linux | `rustix::fs::renameat_with` with `RenameFlags::NOREPLACE` (`renameat2`) | Destination must be absent at the operation itself |
| macOS | Same rustix interface, mapped to `renameatx_np(RENAME_EXCL)` | Destination must be absent at the operation itself |
| Windows | `MoveFileW` | Existing destinations fail; no replacement or cross-volume copy flag |
| Other targets / unsupported filesystem | Error | No fallback to a replacing rename or partial copy |

Source and destination must be on the same filesystem. Publishing a directory makes the staged set visible together; no per-file partial final directory is used. Empty directories, files, non-empty directories, and dangling symlinks at the destination are retained on failure. Source staging remains available when publication fails.

Finalized data remain recovery evidence if SQLite persistence fails. Do not remove a published compressed directory or checksum manifest as if it were disposable staging. Existing gzip/SHA-256 recovery revalidates data before adoption or COMPLETE. SRA and uncompressed FASTQ remain preserved.

## Verification and limits

Synthetic tests cover separate-process SQLite/output contention, process termination/reacquisition with retained markers, alias/deduplication, two simultaneous uncoordinated publishers, an empty destination created after a prior check, existing manifest bytes, Unix dangling symlinks, and executor contention without checkpoint/attempt mutation. Existing tests cover interruption, checksum/gzip corruption, and persisted restart recovery. The Rust workflow runs on Linux, Windows, and macOS. Acceptance requires green checks for the exact candidate, not merely matrix configuration.

These are cooperating-engine locks on local filesystems. They do not stop an administrator or unrelated program from deliberately replacing parent directories, deleting lock markers, hard-link aliasing a database, or modifying source files during execution. Native no-replace publication protects against unrelated **destination creation**, even if that writer ignores the lock. Network/removable/virtual filesystems require independent validation and may reject locking/rename features; failures stay blocking and preserve evidence. Tests establish process-crash behavior, not hardware power-loss durability of every filesystem.

The CLI is still a scaffold, and Tauri execution controls are not yet wired. This patch does not authorize real GSE89223 downloads or scientific execution.

## Primary API contracts

- [Rust File locking](https://doc.rust-lang.org/std/fs/struct.File.html#method.try_lock), stable since Rust 1.89; [TryLockError](https://doc.rust-lang.org/std/fs/enum.TryLockError.html).
- [rustix renameat_with](https://docs.rs/rustix/1.1.5/rustix/fs/fn.renameat_with.html) and [platform flags](https://github.com/bytecodealliance/rustix/blob/v1.1.5/src/backend/libc/fs/types.rs).
- [Microsoft MoveFileW](https://learn.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-movefilew).
