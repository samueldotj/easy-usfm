/**
 * Per-line terminators, carried through edits.
 *
 * FILE-FIDELITY §1 states the rule most designs leave undefined:
 *
 * > Unmodified lines keep their original terminator. A new line inherits the
 * > terminator of the line it was split from. Mixed files are never silently
 * > normalized.
 *
 * The editor buffer uses one separator so change mapping is unambiguous, which
 * means the buffer cannot carry this information — it lives here, in an array
 * remapped on every transaction, and is reapplied when the file is serialized.
 *
 * # Why this is worth an item of its own
 *
 * The naive version keeps a terminator per line and rebuilds the array
 * whenever the line count changes. That is wrong in the case that matters:
 * splitting a CRLF line in a file whose other lines are LF has to produce two
 * CRLF lines, not one CRLF and one of whatever the file mostly uses. Getting
 * it wrong rewrites terminators on lines the user never touched, which is the
 * exact damage the fidelity envelope exists to prevent — and it is invisible
 * until someone reads the diff.
 */

export type Eol = "lf" | "crlf" | "cr";

export const TERMINATOR: Record<Eol, string> = {
  lf: "\n",
  crlf: "\r\n",
  cr: "\r",
};

/** One change, in the coordinates CodeMirror reports. */
export interface Change {
  /** Start of the replaced range, in the document before the change. */
  fromA: number;
  /** End of the replaced range, in the document before the change. */
  toA: number;
  /** The text put in its place. */
  inserted: string;
}

/**
 * The terminators of a document, one per newline, in order.
 *
 * `terminators[i]` ends line `i`. A final line with no trailing newline has no
 * entry, so the array is exactly as long as the document has newlines.
 */
export class LineTerminators {
  #eols: Eol[];
  #fallback: Eol;

  constructor(eols: Eol[], fallback: Eol) {
    this.#eols = [...eols];
    this.#fallback = fallback;
  }

  /** A document whose lines all end the same way. */
  static uniform(count: number, eol: Eol): LineTerminators {
    return new LineTerminators(Array<Eol>(count).fill(eol), eol);
  }

  get length(): number {
    return this.#eols.length;
  }

  /** The terminator ending line `index`, or the fallback past the end. */
  at(index: number): Eol {
    return this.#eols[index] ?? this.#fallback;
  }

  /**
   * The terminator a recovery would write the file with.
   *
   * The first one, falling back to what the document was opened as -- the same
   * rule the shell applies when it rebuilds the envelope, so the two cannot
   * disagree about a file's line endings. A document with no newlines at all
   * has only the fallback to go on.
   */
  dominant(): Eol {
    return this.#eols[0] ?? this.#fallback;
  }

  toArray(): Eol[] {
    return [...this.#eols];
  }

  /** Whether the document disagrees with itself. */
  get mixed(): boolean {
    return this.#eols.some((eol) => eol !== this.#eols[0]);
  }

  /**
   * Applies a set of changes.
   *
   * Changes arrive in ascending order and in the coordinates of the document
   * *before* any of them, which is what `iterChanges` reports. They are
   * therefore applied from the end backwards, so that each one's indices are
   * still valid when it is its turn.
   */
  apply(doc: string, changes: Change[]): LineTerminators {
    const eols = [...this.#eols];

    for (const change of [...changes].reverse()) {
      // Which terminators the replaced range covered. A newline at offset o
      // ends the line containing o, so counting newlines before a position
      // gives that position's line index.
      const firstLine = countNewlines(doc, 0, change.fromA);
      const removed = countNewlines(doc, change.fromA, change.toA);
      const added = countNewlines(change.inserted, 0, change.inserted.length);

      // The line being split, and therefore the terminator a new line
      // inherits. Taken before the splice, from the line the change starts on,
      // so an insertion in the middle of a CRLF line yields CRLF lines.
      const inherited = eols[firstLine] ?? this.#fallback;

      eols.splice(firstLine, removed, ...Array<Eol>(added).fill(inherited));
    }

    return new LineTerminators(eols, this.#fallback);
  }

  /**
   * Puts the terminators back, turning the editor's text into the file's
   * bytes.
   *
   * The counterpart of `FileFidelity::serialize` in the engine; the two must
   * agree, which is what the round-trip tests check on both sides.
   */
  serialize(text: string): string {
    const endsWithNewline = text.endsWith("\n");
    const body = endsWithNewline ? text.slice(0, -1) : text;
    const lines = body.split("\n");

    let out = "";
    for (let index = 0; index < lines.length; index += 1) {
      if (index > 0) out += TERMINATOR[this.at(index - 1)];
      out += lines[index];
    }
    if (endsWithNewline) out += TERMINATOR[this.at(lines.length - 1)];

    return out;
  }
}

/**
 * The shape `ChangeSet.iterChanges` reports into.
 *
 * Declared structurally rather than importing CodeMirror's type, so this
 * module and its tests stay free of the editor. The delta protocol (P2.2)
 * consumes the same callback for a different purpose.
 */
interface ChangeSetLike {
  iterChanges(
    callback: (
      fromA: number,
      toA: number,
      fromB: number,
      toB: number,
      inserted: { toString(): string },
    ) => void,
  ): void;
}

/** Turns a CodeMirror change set into the changes {@link LineTerminators} takes. */
export function changesOf(changes: ChangeSetLike): Change[] {
  const out: Change[] = [];
  changes.iterChanges((fromA, toA, _fromB, _toB, inserted) => {
    out.push({ fromA, toA, inserted: inserted.toString() });
  });
  return out;
}

function countNewlines(text: string, from: number, to: number): number {
  let count = 0;
  for (let index = from; index < to; index += 1) {
    if (text.charCodeAt(index) === 10) count += 1;
  }
  return count;
}
