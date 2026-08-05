<script lang="ts">
  import { Annotation, EditorState } from "@codemirror/state";
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

  let host: HTMLDivElement;
  let view: EditorView | undefined;

  onMount(() => {
    view = new EditorView({
      parent: host,
      state: EditorState.create({
        doc: value,
        extensions: [
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

          // UNICODE §8. Set on the content rather than inherited, because a
          // document beginning `\v 1` auto-detects as left-to-right and would
          // be wrong for an entire right-to-left translation.
          EditorView.contentAttributes.of({
            dir: direction,
            "aria-label": "USFM source",
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
            if (update.selectionSet || update.docChanged) {
              oncursor?.(update.state.selection.main.head);
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

    return () => {
      dom.removeEventListener("compositionstart", started);
      dom.removeEventListener("compositionend", ended);
      view?.destroy();
    };
  });

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
  export function reveal(from: number, to: number): void {
    if (!view) return;
    const length = view.state.doc.length;
    view.dispatch({
      selection: { anchor: Math.min(from, length), head: Math.min(to, length) },
      scrollIntoView: true,
    });
    view.focus();
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
