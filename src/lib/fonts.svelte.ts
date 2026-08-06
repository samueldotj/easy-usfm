/**
 * Fonts — the size multiplier, and finding out what will not render.
 *
 * UNICODE §7. Two jobs that look separate and are the same question asked
 * twice: which face covers this script, and how big does it need to be.
 *
 * # Why the faces are built here and not in a stylesheet
 *
 * The multiplier has to apply *per run* — a Tamil word inside an English
 * sentence needs it and the English does not — and CSS already has exactly
 * that mechanism: `@font-face` with `unicode-range` and `size-adjust`. The
 * browser picks the face per character, so no markup, no script detection at
 * render time, and mixed-script lines come out right without anyone thinking
 * about them.
 *
 * Written as `FontFace` objects rather than as CSS text because the rules are
 * derived from {@link SCRIPTS}, and a stylesheet would be a second copy of
 * that table maintained by hand. `document.fonts.add` is a scripting API, not
 * a `<style>` element, so this does not reintroduce the injection that
 * SECURITY §5 and P1.12 are about removing.
 *
 * # Why loading is also the detection
 *
 * `FontFace.load()` on a `local()` source resolves when the system has that
 * font and rejects when it does not. That is the missing-font check, for free,
 * from the code that was going to run anyway.
 *
 * It answers a narrower question than "will this be tofu", though, so the two
 * are kept apart: a system may lack Noto Sans Tamil and still render Tamil in
 * something else. {@link willNotRender} asks the second question by measuring,
 * and the notice says which of the two happened, because "install a font or
 * you will see boxes" and "this is not the font we would have chosen" deserve
 * very different amounts of the reader's attention.
 */

import { SCRIPTS, lineHeightFor, scriptsIn, unicodeRange, type Script } from "./scripts";

/**
 * The family every content surface asks for first.
 *
 * One name, many faces, each claiming its own `unicode-range`.
 */
export const CONTENT_FAMILY = "Easy USFM Content";

/** How long typing must stop before the document is re-checked. */
const IDLE_MS = 900;

/**
 * `FontFaceDescriptors`, plus the descriptor this module exists to set.
 *
 * `size-adjust` has been in the CSS Fonts specification and shipping in every
 * engine this project targets for years; TypeScript's DOM library has simply
 * not caught up. Declared narrowly rather than cast away, so the compiler
 * still checks the other descriptors.
 */
type Descriptors = FontFaceDescriptors & { sizeAdjust?: string };

/** What a script's font situation is. */
export interface FontReport {
  readonly script: Script;
  /** The recommended face is not installed. */
  readonly missing: boolean;
  /** Nothing on this system can draw it: the reader will see boxes. */
  readonly tofu: boolean;
}

let installed: Promise<ReadonlySet<string>> | null = null;

/**
 * Registers a face per script, so the multiplier applies.
 *
 * Idempotent and cached: called on open, and opening a second file must not
 * add a second set of faces for the same ranges.
 *
 * A face that fails to load is simply not added, and the stack falls through
 * to whatever the system does have. That is the right failure — the document
 * still renders, it renders in a substitute, and the notice says so.
 */
export function installFontFaces(): Promise<ReadonlySet<string>> {
  if (installed) return installed;

  installed = (async () => {
    const present = new Set<string>();
    if (typeof document === "undefined" || !document.fonts) return present;

    await Promise.all(
      SCRIPTS.map(async (script) => {
        // `local()` only: UNICODE §7 rules out bundling — licensing, and
        // roughly 10 MB per script.
        const descriptors: Descriptors = {
          unicodeRange: unicodeRange(script),
          // The whole reason this module exists.
          sizeAdjust: `${Math.round(script.scale * 100)}%`,
        };
        const face = new FontFace(CONTENT_FAMILY, `local("${script.font}")`, descriptors);

        try {
          await face.load();
          document.fonts.add(face);
          present.add(script.name);
        } catch {
          // Not installed, so the stack falls through to whatever the system
          // has. Recorded by omission.
        }
      }),
    );

    return present;
  })();

  return installed;
}

/**
 * A character no font can contain.
 *
 * U+FFFF is a permanent noncharacter — the standard guarantees it will never
 * be assigned, so nothing has a glyph for it and every engine draws its
 * last-resort box. That makes it a reference for what "no font" looks like.
 */
const NO_GLYPH = "￿";

/**
 * Whether nothing on this system can draw the script.
 *
 * Measured rather than asked, because there is no API for it, and measured
 * against {@link NO_GLYPH} rather than against a missing font family — which
 * was the first attempt and does not work. Naming a family that does not exist
 * changes nothing: the browser still runs its whole fallback chain and finds
 * whatever the system has, so the two measurements agree both when the script
 * is unrenderable *and* when a system font renders it perfectly well. On
 * Windows that reported Tamil and Devanagari as unreadable while Nirmala UI
 * was drawing them correctly, which is the loud notice crying wolf.
 *
 * Comparing against a character that certainly has no glyph asks the right
 * question. Equal widths mean the sample resolved to the same last-resort box.
 *
 * A canvas rather than the DOM: no layout, no reflow, and nothing appears on
 * screen while the question is being asked.
 */
function willNotRender(script: Script, context: CanvasRenderingContext2D): boolean {
  // Both measured in the same stack, so the only variable is whether a font in
  // it covers the character.
  context.font = `72px "${CONTENT_FAMILY}", "Noto Sans", system-ui, sans-serif`;

  const box = context.measureText(NO_GLYPH).width;
  const actual = context.measureText(script.sample).width;

  // Canvas widths are fractional, so this is a comparison and not equality.
  return Math.abs(actual - box) < 0.01;
}

/**
 * What each script in `text` will look like on this machine.
 *
 * Only the scripts the document actually uses. A report on scripts nobody has
 * opened is a notice about a problem that does not exist.
 */
export async function report(text: string): Promise<FontReport[]> {
  const present = await installFontFaces();

  const used = scriptsIn(text);
  if (used.length === 0 || typeof document === "undefined") return [];

  const context = document.createElement("canvas").getContext("2d");
  if (!context) return [];

  return used.map((script) => ({
    script,
    // Whether the face actually loaded, not `FontFaceSet.check` -- which
    // answers "are the faces matching this family loaded", and a family that
    // no face claims has nothing outstanding, so it answers yes. It reported
    // every font present on a machine that had none of them.
    missing: !present.has(script.name),
    tofu: willNotRender(script, context),
  }));
}

/**
 * The document's font situation, as the interface shows it.
 *
 * Held as state rather than recomputed, because the notice is dismissible and
 * a dismissal has to survive the next keystroke — UNICODE §7 asks for a
 * one-time non-modal notice, and one that came back would be neither.
 */
class Fonts {
  reports = $state<FontReport[]>([]);
  lineHeight = $state(1.5);

  /** Scripts already reported on, so the notice does not return. */
  #told = new Set<string>();

  /** Everything worth telling the user about, minus what they have seen. */
  get notices(): FontReport[] {
    return this.reports.filter(
      (entry) => (entry.missing || entry.tofu) && !this.#told.has(entry.script.name),
    );
  }

  dismiss(): void {
    for (const entry of this.reports) this.#told.add(entry.script.name);
    // Reassigned so the getter's dependents re-run; mutating the set alone
    // changes nothing Svelte is watching.
    this.reports = [...this.reports];
  }

  /** Called when a document is opened. */
  async inspect(text: string): Promise<void> {
    this.reports = await report(text);
    this.lineHeight = lineHeightFor(this.reports.map((entry) => entry.script));
  }

  #timer: ReturnType<typeof setTimeout> | null = null;

  /**
   * Re-checks once typing stops.
   *
   * UNICODE §7 asks for detection on open, and on open alone would be wrong in
   * the one case this project is most for: a translator creating a new file
   * and typing their own script into it never opens anything, so they would
   * get neither the leading nor the notice.
   *
   * Off the keystroke path entirely -- a scan of the document is cheap but not
   * free, and it answers a question that changes at most a handful of times in
   * a session.
   */
  schedule(text: string): void {
    if (this.#timer !== null) clearTimeout(this.#timer);
    this.#timer = setTimeout(() => {
      this.#timer = null;
      void this.inspect(text);
    }, IDLE_MS);
  }
}

export const fonts = new Fonts();
