# File Fidelity

The fidelity envelope, line endings and BOM, the atomic save ladder, external change detection, recovery, and the tests behind them.

Related: [ARCHITECTURE](ARCHITECTURE.md) · [PRODUCT](PRODUCT.md) · [UNICODE](UNICODE.md) · [ADR-003](adr/003-source-authoritative.md) · [ADR-005](adr/005-save-strategy.md)

---

File safety is the property users cannot verify for themselves and cannot forgive. Everything here follows from one commitment: **the source text is authoritative and is never regenerated from the parse tree** ([ADR-003](adr/003-source-authoritative.md)). Saving writes the buffer; the parser's opinion is not consulted.

---

## 1. The fidelity envelope

Byte-level properties are captured at load into a value held **outside** the editor buffer and reapplied at serialization. CodeMirror normalizes line endings on load whatever you do; capturing separately is the workable response.

```rust
pub struct FileFidelity {
    pub bom: bool,
    pub eol: LineEndings,
    pub final_newline: bool,
    pub original_hash: [u8; 32],      // blake3 of the exact bytes read
    pub canonical_path: PathBuf,      // symlinks resolved
    pub was_symlink: bool,
    pub permissions: Permissions,
    pub mtime: SystemTime,
    pub len: u64,
}

pub enum LineEndings {
    Uniform(Eol),                     // Lf | Crlf | Cr
    Mixed { per_line: Vec<Eol>, dominant: Eol },
}
```

The editor uses one separator so the mapping is unambiguous: `EditorState.lineSeparator.of("\n")`.

Normalization form is deliberately **not** in this struct — it is not reapplied, because the buffer holds the original bytes and never normalizes. Detected form is reported and diagnosed if mixed ([UNICODE §4](UNICODE.md#4-normalization-versus-byte-fidelity)).

**Mixed line endings** — the rule most designs leave undefined:

> Unmodified lines keep their original terminator. A new line inherits the terminator of the line it was split from. Mixed files are never silently normalized.

A parallel per-line terminator array is remapped on each transaction via CodeMirror's change mapping of line-start positions. A **Normalize line endings…** command exists, is never automatic, and marks the document dirty like any other edit.

**Serialization:**

```text
bytes = (bom ? EF BB BF : "")
      + join(lines, per-line terminators)
      + (final_newline ? terminator_of_last_line : "")
```

**A clean document's Save is a no-op** — the file is not touched. Correct behaviour, and it has a consequence for testing (§5.1).

## 2. Atomic save

Three rungs, chosen per platform and filesystem. **The original is never truncated before the replacement is durable.** Rationale and rejected alternatives in [ADR-005](adr/005-save-strategy.md).

### Rung 1 — durable rename (POSIX)

```text
1.  Resolve symlinks → canonical target T.
2.  Create temp in dirname(T), not $TMPDIR:  .{basename}.{pid}.{rand}.tmp
        permissions copied from T (mode, uid/gid where permitted)
3.  Write bytes. fsync(temp_fd).
4.  macOS: copyfile(T, temp, COPYFILE_XATTR | COPYFILE_ACL)
    Linux:  copy xattrs via listxattr/getxattr/setxattr
5.  rename(temp, T)                    — atomic on the same filesystem
6.  fsync(dirfd(dirname(T)))           — makes the rename itself durable
```

Two steps are commonly omitted and both matter. **Step 2's placement:** a temp in `$TMPDIR` is usually on another filesystem, where `rename` is not atomic and often fails outright. **Step 6:** without it, a power loss can leave the directory entry unpersisted — temp gone, rename lost, original unreferenced.

### Rung 1 — Windows

`ReplaceFileW`, **not** `MoveFileEx`. `ReplaceFileW` preserves the destination's ACLs, attributes, creation time, object ID, and alternate data streams; `MoveFileEx` discards them.

```text
1.  Write temp in the same directory. FlushFileBuffers.
2.  ReplaceFileW(T, temp, NULL, REPLACEFILE_WRITE_THROUGH, NULL, NULL)
3.  On ERROR_UNABLE_TO_MOVE_REPLACEMENT_2 or a sharing violation → rung 2.
```

### Rung 2 — copy-back

For cloud-sync roots and network shares, detected by rename failure or by the target sitting under a known sync root (`~/Dropbox`, OneDrive registry paths, `~/Library/CloudStorage`, `~/Google Drive`).

Sync clients hold open handles and watch inodes. Replacing the inode reads to them as delete-plus-create, which produces conflicted copies and on some clients loses data.

```text
1.  Write temp beside T, fsync.
2.  Copy T → T.bak-{timestamp} (sidecar, same directory).
3.  Open T with O_TRUNC, stream temp's bytes in, fsync.
4.  On success, remove temp and sidecar.
5.  On failure, retain the sidecar and surface its path.
```

This preserves the inode, which is what sync clients and hardlinks require. It is genuinely non-atomic — hence rung 2, and hence the sidecar, which is the answer to "what if we crash inside step 3."

### Rung 3 — read-only or permission denied

No write attempted. The dialog offers **Save As…** and, on POSIX where the user owns the parent directory, a retry with elevated permissions.

### Hardlinks

If `st_nlink > 1`, rung 1 breaks the link silently — other names keep pointing at the old inode. Detect and use rung 2; show "linked file" in the status bar so slower saves have a visible reason.

### Failure reporting

Every failure path names the file that still holds the previous content, gives the sidecar path where one exists, and leaves the document dirty. **No failure path leaves the editor believing it saved.**

## 3. External change detection

The watcher must ignore our own writes **by content hash, not timing** — timing alone is unreliable on network filesystems where events arrive seconds late, and on sync clients that rewrite files after upload.

On a watcher event for the document path:

```text
1. Debounce 250 ms (editors and sync clients emit event storms).
2. Read the file and hash it.
3. hash == last_known_hash             → ignore silently.
4. hash matches a SaveEpoch < 30 s old → ignore, drop the epoch.
5. otherwise                           → genuine external change.
```

Without this, every save triggers "this file changed on disk", which trains users to dismiss the prompt that matters.

- **Clean** — reload silently, preserving position **by verse reference, not offset** (offsets are meaningless after an external rewrite), with a transient status-bar notice.
- **Dirty** — non-modal bar: *"This file changed on disk."* with **Reload (discard my changes)** / **Keep my version** / **Compare**. Never a blocking modal; never an automatic overwrite.
- **Deleted or renamed away** — mark dirty, retain the buffer, show *"The file no longer exists — Save will recreate it."*

Watching uses `notify`, falling back to 2 s polling where the backend reports unsupported — common on network mounts and in containers.

## 4. Recovery and locking

**Snapshots.** On 4 s idle after a change, and unconditionally every 45 s during continuous typing. Written to `recovery/{blake3(canonical_path)[..16]}/` as `snapshot.usfm` plus `meta.json` (original path, fidelity envelope, cursor, dirty flag, versions, pid, wall clock), using the rung-1 procedure — a torn recovery file is worse than none. Last 3 retained; directories older than 30 days pruned at startup; cleared on clean save and clean close.

**Advisory lock.** `recovery/{hash}/owner.lock` holds `{pid, started_at, host, app_version}`.

- **PID alive and ours** → focus the existing window rather than opening a second.
- **PID alive and foreign** → open read-only with **Open a copy** or **Take over** (warning that the other instance may overwrite).
- **PID dead** → a crash occurred. Compare the snapshot against disk; if they differ, offer recovery with a summary: *"Unsaved changes from Tuesday 14:12 were found (37 lines differ)."*

Recovery is always a choice, never automatic.

**Web parity.** IndexedDB keyed by file name, size, and last-modified; same cadence and retention. `navigator.locks` for the cross-tab equivalent. `beforeunload` warns on dirty state; a final snapshot flushes on `visibilitychange → hidden`, the only reliably-fired teardown event.

---

## 5. Testing

### 5.1 Round-trip fidelity

The naive test — open, save, compare — is unfalsifiable once §1 makes a clean save a no-op; it would pass on an implementation with no serializer at all. Three tests replace it:

```text
T1  Idempotent edit
      open → insert "x" → undo → save → compare bytes
      Expected: byte-for-byte identical.

T2  Save As round-trip
      open → Save As to a new path, no edits → compare bytes
      Expected: byte-for-byte identical.

T3  Localized edit
      open → edit exactly one verse → save → diff
      Expected: the diff touches only that verse's line(s). BOM, line endings
      elsewhere, blank lines, normalization form, trailing newline unchanged.
```

**T3 is the important one.** It catches accidental whole-document normalization, which is the failure the preservation guarantee actually cares about. T1 and T2 can pass on a system that quietly rewrites everything identically; T3 cannot.

All three run in CI across the full corpus ([ARCHITECTURE §12.4](ARCHITECTURE.md#124-corpus)).

### 5.2 Fault injection

File I/O goes through a trait so §2's failure paths are testable rather than aspirational:

```rust
pub trait FileSystem {
    fn read(&self, p: &Path) -> io::Result<Vec<u8>>;
    fn write_atomic(&self, p: &Path, b: &[u8], f: &FileFidelity) -> io::Result<()>;
}
```

`FaultyFs` injects ENOSPC mid-write, EACCES on rename, EROFS, a vanished parent directory, a concurrent external write between read and rename, and a crash between temp-write and rename. **Each must leave the original intact and the document dirty.** Every rung is exercised, including the rung-1 → rung-2 fallback.

### 5.3 Also required

Recovery after SIGKILL with the prompt asserted; external-change detection with and without self-suppression (the second must prompt, the first must not); rung-2 selection under a simulated sync root; hardlink detection forcing rung 2; mixed-EOL preservation through an edit that splits a line.
