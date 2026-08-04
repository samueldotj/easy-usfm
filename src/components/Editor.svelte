<script lang="ts">
  import { EditorState } from "@codemirror/state";
  import { EditorView, keymap, lineNumbers, highlightActiveLine, drawSelection } from "@codemirror/view";
  import { defaultKeymap, history, historyKeymap } from "@codemirror/commands";
  import { onMount } from "svelte";

  import { changesOf, type Change } from "../lib/eol";

  interface Props {
    value?: string;
    /** Document direction. UNICODE §8 — explicit, never inherited. */
    direction?: "ltr" | "rtl";
    onchange?: (value: string, changes: Change[]) => void;
  }

  let { value = "", direction = "ltr", onchange }: Props = $props();

  let host: HTMLDivElement;
  let view: EditorView | undefined;

  onMount(() => {
    view = new EditorView({
      parent: host,
      state: EditorState.create({
        doc: value,
        extensions: [
          lineNumbers(),
          history(),
          drawSelection(),
          highlightActiveLine(),
          keymap.of([...defaultKeymap, ...historyKeymap]),

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
            if (update.docChanged) {
              onchange?.(update.state.doc.toString(), changesOf(update.changes));
            }
          }),
        ],
      }),
    });

    return () => view?.destroy();
  });

  /** Focus the editor. Used by F6 pane cycling. */
  export function focus(): void {
    view?.focus();
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
      annotations: [],
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
