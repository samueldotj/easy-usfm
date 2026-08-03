<script lang="ts">
  import { EditorState } from "@codemirror/state";
  import { EditorView, keymap, lineNumbers, highlightActiveLine, drawSelection } from "@codemirror/view";
  import { defaultKeymap, history, historyKeymap } from "@codemirror/commands";
  import { onMount } from "svelte";

  interface Props {
    value?: string;
    /** Document direction. UNICODE §8 — explicit, never inherited. */
    direction?: "ltr" | "rtl";
    onchange?: (value: string) => void;
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
            if (update.docChanged) onchange?.(update.state.doc.toString());
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
</script>

<div class="editor" bind:this={host}></div>

<style>
  .editor {
    block-size: 100%;
    overflow: hidden;
  }
</style>
