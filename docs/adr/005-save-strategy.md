# ADR-005 — Save strategy: a ladder, not a single algorithm

**Status:** Accepted

## Context

The textbook safe-save is four steps: write a temp file, flush, rename over the original, delete the temp. It is correct on a local POSIX filesystem and loses data everywhere else.

Translators do not work on clean local filesystems. They work in Dropbox and OneDrive folders, on institutional network shares, in symlinked directories, on Windows volumes where ACLs matter, and on macOS where extended attributes carry metadata other tools depend on.

What the naive rename destroys:

| Situation | What breaks |
|---|---|
| Target is a symlink | The link is replaced by a regular file; edits go somewhere unexpected |
| `st_nlink > 1` | The hardlink is silently broken; other names see the old content |
| macOS | Extended attributes, resource forks, ACLs are lost |
| Windows with `MoveFileEx` | ACLs, alternate data streams, creation time, object ID are lost |
| Cloud-sync folder | The inode changes; the client reads delete-plus-create and produces conflicted copies |
| Network share | `rename` may fail outright, or succeed non-atomically |
| Temp in `$TMPDIR` | Different filesystem, so `rename` is not atomic and usually fails |
| Power loss after rename | Without a directory `fsync`, the entry may not persist — temp gone, rename lost, original unreferenced |

No single algorithm handles all of these. **Atomicity and metadata preservation are in direct tension:** `rename` is atomic but replaces the inode; rewriting in place preserves the inode but is not atomic.

## Decision

**Three rungs, selected per platform and filesystem, with an explicit fallback ladder. The original is never truncated before the replacement is durable.** Procedures in [FILE-FIDELITY §2](../FILE-FIDELITY.md#2-atomic-save).

| Rung | Method | Used when |
|---|---|---|
| **1** | Durable rename (POSIX) / `ReplaceFileW` (Windows) | Default |
| **2** | Copy-back with sidecar | Cloud-sync roots, network shares, hardlinks, rung-1 failure |
| **3** | Refuse and offer Save As | Read-only target, permission denied |

**Rung 2 exists because atomicity is not the only value.** Cloud-sync clients hold open handles and watch inodes; an atomic rename is, from their perspective, a delete followed by a create — producing conflicted copies and on some clients losing data. Rung 2 gives up atomicity to preserve the inode. **The sidecar is what makes that acceptable**: the truncate-and-rewrite step has a window, and the sidecar is the answer to "what if we crash inside it." That window is the price of inode preservation, paid only where rung 1 would do worse. Rung 2 is also correct for hardlinks, where rung 1 breaks the link silently.

## Rationale

**Why not always rung 2.** It is never atomic. On a normal local filesystem that is a real regression for no benefit — rung 1 should be the common path.

**Why not always rung 1.** It is wrong in ways invisible until they cost someone a day's work: a conflicted copy in Dropbox two hours later, a broken hardlink discovered next week, a lost ACL noticed by an administrator. None of these produce an error message.

**Why detection rather than configuration.** Asking "is this a cloud folder?" is asking the user to debug filesystem semantics. Detection uses two signals — known sync-root paths and rung-1 failure. Neither is perfect, together they cover the realistic cases, and the fallback is automatic.

**Why failures must name the surviving file.** A failed save is frightening; the user does not know whether their work is gone, and the application does. Every failure path states which file holds the previous content, gives the sidecar path where one exists, and leaves the document dirty. A clean dirty-flag after a failed write is worse than the failure.

## Consequences

**The watcher must be taught to ignore us.** Any save fires the file watcher, and timing-based suppression is unreliable — network filesystems deliver events seconds late, sync clients rewrite after upload. Suppression is therefore by content hash with a 30-second epoch window ([FILE-FIDELITY §3](../FILE-FIDELITY.md#3-external-change-detection)). Without it, every save prompts "this file changed on disk", which trains users to dismiss the prompt that matters.

**Recovery snapshots use rung 1.** A torn recovery file is worse than none, so snapshots get the same durability treatment as saves.

**Failure paths must be testable.** Three rungs plus a fallback ladder is enough branching that "it should work" is not good enough, which is why file I/O goes through a trait and a fault-injection harness exercises every rung ([FILE-FIDELITY §5.2](../FILE-FIDELITY.md#52-fault-injection)).

**Saves are slower than a naive implementation** — two `fsync` calls plus metadata copying on rung 1, a full file copy for the sidecar on rung 2. Budgeted under 300 ms p95 for a typical file, well inside what a user perceives as immediate. Rung 2's extra cost is surfaced honestly: the status bar shows "linked file" or a sync-folder indicator, so slower saves have a visible reason.

**It only works because saves write the buffer.** This ladder is tractable because saving is "write these exact bytes" ([ADR-003](003-source-authoritative.md)). If saving meant serializing a tree, every rung would additionally have to be correct about *content*, and the failure modes would compound.

## Alternatives considered

**Write in place always.** Simplest, preserves everything, and truncates the user's file before the new content is durable. Rejected outright — this is the failure the design exists to prevent.

**Rename always, accept the metadata loss.** What most editors do. Rejected because the cloud-sync case is not an edge case for this audience: institutional translation work lives in shared folders.

**Copy-on-write / reflink where available (`FICLONE`, APFS).** Attractive for the sidecar step, but platform coverage is too uneven to depend on. Worth revisiting as an optimization within rung 2, not as a rung of its own.

**Journal or write-ahead log.** Over-engineered for single-file editing, and it introduces a second source of truth — precisely what [ADR-003](003-source-authoritative.md) rejects. The recovery snapshot system already covers the crash case at far lower complexity.
