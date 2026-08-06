<script lang="ts">
  import { Annotation, Compartment, EditorState } from "@codemirror/state";
  import { EditorView, keymap, lineNumbers, highlightActiveLine, drawSelection } from "@codemirror/view";
  import { defaultKeymap, history, historyKeymap } from "@codemirror/commands";
  import { autocompletion, completionKeymap } from "@codemirror/autocomplete";
  import { onMount } from "svelte";

  import {
    diagnostics,
    revealDiagnostic,
    setDiagnostics,
    stepDiagnostic,
  } from "../lib/diagnostics";
  import { markerCompletions, optionClass } from "../lib/complete";
  import { invisibles, setShowInvisibles } from "../lib/invisibles";
  import { changesOf, type Change } from "../lib/eol";
  import { highlighting, setTokens, tokenRequests } from "../lib/highlight";
  import type { Completion, Diagnostic, Token } from "../worker/protocol";

  interface Props {
    value?: string;
    /** Document direction. UNICODE §8 — explicit, never inherited. */
    direction?: "ltr" | "rtl";
    onchange?: (value: string, changes: Change[]) => void;
    /** An input method has begun assembling a word. */
    oncompositionstart?: () => void;
    /** It has committed. UNICODE §5 — everything held goes as one batch. */
    oncompositionend?: (value: string) => void;
    /** The visible range changed and wants highlighting. */
    ontokenrange?: (from: number, to: number) => void;
    /** The cursor moved. The status bar asks the engine where that is. */
    oncursor?: (at: number) => void;
    /** A backslash wants its marker list. */
    oncomplete?: (at: number) => Promise<Completion[]>;
    /** Show zero-width characters (UNICODE appendix, P3.12). */
    showInvisibles?: boolean;
    /** The editor was scrolled. The preview follows (P3.6). */
    onscroll?: () => void;
    /**
     * Refuse edits (FILE-FIDELITY §4).
     *
     * Another instance holds this file, so typing into it would produce work
     * that the other window is about to save over.
     */
    readOnly?: boolean;
  }

  let {
    value = "",
    direction = "ltr",
    onchange,
    oncompositionstart,
    oncompositionend,
    ontokenrange,
    oncursor,
    oncomplete,
    showInvisibles = false,
    onscroll,
    readOnly = false,
  }: Props = $props();

  /**
   * Marks a change as coming from outside the editor.
   *
   * Opening a file replaces the whole document, and CodeMirror reports that
   * as a transaction like any other. Without this the act of opening marks the
   * document unsaved, so every freshly opened file claims to have changes —
   * and the close warning fires on a document nobody touched.
   */
  const External = Annotation.define<boolean>();

  /**
   * The read-only flag, in a compartment so it can be changed later.
   *
   * `EditorState.readOnly` is a static facet; without a compartment it would be
   * fixed at construction, and taking over a file another instance held would
   * leave the editor refusing edits until the window was reopened.
   */
  const readOnlyState = new Compartment();

  let host: HTMLDivElement;
  let view: EditorView | undefined;

  /**
   * The page's CSP nonce, if the host set one.
   *
   * Read off an element the host itself stamped rather than from a meta tag we
   * write, because the nonce has to come from whatever generated the policy --
   * a nonce the page chose for itself is not a nonce, it is a constant.
   *
   * The `nonce` property rather than `getAttribute`: browsers hide the
   * attribute value from script to stop exactly the exfiltration this is
   * guarding, and expose it only through the IDL property.
   */
  function cspNonce(): string | undefined {
    const stamped = document.querySelector<HTMLElement>("script[nonce], style[nonce]");
    return stamped?.nonce || undefined;
  }

  onMount(() => {
    view = new EditorView({
      parent: host,
      state: EditorState.create({
        doc: value,
        extensions: [
          // CodeMirror injects its structural base theme as a style element,
          // which is the one thing keeping `'unsafe-inline'` in the CSP
          // (SECURITY 5). Given a nonce it stamps that element, and the policy
          // can name the nonce instead. Empty when the host supplies none,
          // which changes nothing -- so this is safe to have in place before
          // the policy moves.
          EditorView.cspNonce.of(cspNonce() ?? ""),

          // Before the line numbers, so the glyph column is on the outside and
          // the numbers stay next to the text they count.
          diagnostics,
          lineNumbers(),
          history(),
          drawSelection(),
          highlightActiveLine(),
          // Before the default keymap, so Enter and the arrow keys reach the
          // completion list while it is open rather than the document.
          keymap.of(completionKeymap),
          keymap.of([...defaultKeymap, ...historyKeymap]),

          autocompletion({
            override: [markerCompletions((at) => oncomplete?.(at) ?? Promise.resolve([]))],
            // Nothing else is completable, so a list with one entry left is
            // still worth showing rather than silently applying.
            activateOnCompletion: () => false,
            // Off deliberately: the icon sprite is a set of generic shapes for
            // programming-language symbols, and none of them means anything
            // for USFM's marker classes. The detail line says it in words.
            icons: false,
            optionClass,
          }),

          // Highlighting from the engine's own lexer (P2.6). The field holds
          // what has arrived; the listener asks for what is on screen.
          highlighting,
          tokenRequests((from, to) => ontokenrange?.(from, to)),

          invisibles,

          // Read-only rather than disabled: the text stays selectable and
          // copyable, which is most of what someone does with a file another
          // window has open.
          readOnlyState.of(EditorState.readOnly.of(readOnly)),

          // UNICODE §8. Set on the content rather than inherited, because a
          // document beginning `\v 1` auto-detects as left-to-right and would
          // be wrong for an entire right-to-left translation.
          EditorView.contentAttributes.of({
            dir: direction,
            "aria-label": "USFM source",
            // Where F6 puts focus when it reaches this pane: the element that
            // takes keys, not the region around it.
            "data-pane-focus": "",
          }),
          EditorView.perLineTextDirection.of(true),

          // No theme extension anywhere in this file. Appearance lives in
          // styles/codemirror.css, because EditorView.theme() injects a style
          // element at runtime and the real CSP blocks it (SECURITY §5).
          //
          // Writing that tag literally here would end this component's script
          // block -- Svelte's parser reads it even inside a comment.

          EditorView.updateListener.of((update) => {
            // The changes go with the text, because the per-line terminator
            // array can only be remapped by something that knows how the
            // transaction moved the lines (P1.4).
            const external = update.transactions.some((tr) => tr.annotation(External));
            if (update.docChanged && !external) {
              onchange?.(update.state.doc.toString(), changesOf(update.changes));
            }
            // Also on a document change: typing past a \v marker changes
            // which verse the cursor is in without moving the selection.
            //
            // The selection's *start*, not its head. Revealing a span selects
            // it and leaves the head at its end, which for a verse is the
            // first character of the next one — so clicking verse 2 in the
            // preview reported verse 3, and clicking a chapter's first verse
            // reported the chapter. For a plain cursor the two are the same,
            // so this costs nothing anywhere else.
            if (update.selectionSet || update.docChanged) {
              oncursor?.(update.state.selection.main.from);
            }
          }),
        ],
      }),
    });

    // Listened for on the DOM rather than inferred from transactions, because
    // the composition is a property of the input method and not of the
    // document. CodeMirror's own `composing` flag is derived from these same
    // events; using them directly keeps the suppression independent of the
    // editor's internals (UNICODE §5).
    const dom = view.contentDOM;
    const started = () => oncompositionstart?.();
    const ended = () => oncompositionend?.(view!.state.doc.toString());

    dom.addEventListener("compositionstart", started);
    dom.addEventListener("compositionend", ended);

    // Listened for on the scroller rather than through an update listener:
    // CodeMirror reports a viewport change only when it re-renders, which is
    // coarser than a scroll and would make the preview lag behind in steps.
    const scroller = view.scrollDOM;
    const scrolled = () => onscroll?.();
    scroller.addEventListener("scroll", scrolled, { passive: true });

    return () => {
      dom.removeEventListener("compositionstart", started);
      dom.removeEventListener("compositionend", ended);
      scroller.removeEventListener("scroll", scrolled);
      view?.destroy();
    };
  });

  // The setting lives in editor state, so a change has to be dispatched --
  // a view plugin is not asked to update when a component's variable moves.
  $effect(() => {
    view?.dispatch({ effects: setShowInvisibles.of(showInvisibles) });
  });

  // Same reason, and it matters more: a stale value here is the difference
  // between refusing edits and accepting them.
  $effect(() => {
    view?.dispatch({ effects: readOnlyState.reconfigure(EditorState.readOnly.of(readOnly)) });
  });

  /**
   * The source offset at the top of the viewport.
   *
   * `elementAtHeight` asks CodeMirror rather than measuring DOM nodes, which
   * matters because the editor wraps: a visual line is not a document line,
   * and counting elements gets a wrapped paragraph wrong by however many rows
   * it occupies.
   */
  export function topOffset(): number | null {
    if (!view) return null;
    const block = view.elementAtHeight(view.scrollDOM.scrollTop);
    return block ? block.from : null;
  }

  /** The scrolling element, for the scroll sync to see who the user is driving. */
  export function scroller(): HTMLElement | undefined {
    return view?.scrollDOM;
  }

  /**
   * Puts a source offset at the top of the viewport.
   *
   * By moving the scroller rather than through `EditorView.scrollIntoView`,
   * which is the obvious call and does not do this. That effect's job is to
   * make a position *visible*, and it declines to move when the position
   * already is — so following the preview downwards worked for one step and
   * then stopped, because the paragraph being asked for was still on screen at
   * the bottom. Aligning to the top is a different request, and this is the
   * same arithmetic the preview side uses for it.
   *
   * `documentTop` is where document coordinate zero sits on screen, so adding
   * the block's own top turns a height in the document into a position in the
   * viewport.
   */
  export function scrollToOffset(offset: number): void {
    if (!view) return;
    const at = Math.max(0, Math.min(offset, view.state.doc.length));
    const block = view.lineBlockAt(at);
    const delta = view.documentTop + block.top - view.scrollDOM.getBoundingClientRect().top;
    view.scrollDOM.scrollTop += delta;
  }

  /** Focus the editor. Used by F6 pane cycling. */
  export function focus(): void {
    view?.focus();
  }

  /** Applies highlighting that has come back from the engine. */
  export function applyTokens(from: number, to: number, tokens: Token[]): void {
    view?.dispatch({ effects: setTokens.of({ from, to, tokens }) });
  }

  /** Applies the diagnostics from a parse result. */
  export function applyDiagnostics(list: Diagnostic[]): void {
    view?.dispatch({ effects: setDiagnostics.of(list) });
  }

  /** F8 and Shift+F8 (PRODUCT §6.4). */
  export function step(forward: boolean): void {
    if (view) stepDiagnostic(forward)(view);
  }

  /** The panel asking to go to one of them. */
  export function goTo(index: number, focus: boolean): void {
    if (view) revealDiagnostic(view, index, focus);
  }

  /**
   * Puts the cursor on a resolved reference and scrolls it into view.
   *
   * Selects the range rather than collapsing to its start, so that arriving
   * somewhere is visible -- landing a bare cursor in the middle of a screen of
   * text says nothing about whether anything happened.
   */
  export function reveal(from: number, to: number, focus = true): void {
    if (!view) return;
    const length = view.state.doc.length;
    view.dispatch({
      selection: { anchor: Math.min(from, length), head: Math.min(to, length) },
      scrollIntoView: true,
    });
    // Not while the find bar is being used: stealing focus on every step would
    // mean the next keystroke went into the document rather than the query.
    if (focus) view.focus();
  }

  /**
   * Replaces one range with text, inserted exactly as typed.
   *
   * UNICODE §4: "Replace *insertion* text — exactly as typed, not normalized
   * to match surroundings." The temptation is to make the replacement match
   * the spelling of the text around it, and it is wrong: the user typed what
   * they typed, and silently respelling it is the editor deciding what the
   * file should contain.
   */
  export function replaceRange(from: number, to: number, text: string): void {
    view?.dispatch({
      changes: { from, to, insert: text },
      selection: { anchor: from + text.length },
      scrollIntoView: true,
    });
  }

  /**
   * Replaces every range, in one transaction.
   *
   * One transaction rather than a loop: CodeMirror maps a set of changes
   * against the *original* document, so the offsets stay the ones the engine
   * reported. Applying them one at a time would need every later position
   * shifted by hand, which is arithmetic with no reason to be correct.
   *
   * It is also one undo step, which is what "replace all" means.
   */
  export function replaceAll(ranges: { from: number; to: number }[], text: string): void {
    if (!view || ranges.length === 0) return;
    view.dispatch({
      changes: ranges.map((range) => ({ from: range.from, to: range.to, insert: text })),
    });
  }

  /**
   * Replaces the whole document, without recording it as an edit.
   *
   * Opening a file is not a change to the one that was open, so this bypasses
   * the update listener's dirty tracking by replacing the state outright.
   */
  export function load(text: string): void {
    if (!view) return;
    view.dispatch({
      changes: { from: 0, to: view.state.doc.length, insert: text },
      annotations: External.of(true),
    });
  }
</script>

<div class="editor" bind:this={host}></div>

<style>
  .editor {
    block-size: 100%;
    overflow: hidden;
  }
</style>
