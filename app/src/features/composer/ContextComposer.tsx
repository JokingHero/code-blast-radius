import { For } from "solid-js";
import { useWorkspace } from "../../core/workspace.store";

export const ContextComposer = () => {
  const { state } = useWorkspace();

  return (
    <div class="space-y-4">
      <For each={state.contextFiles}>
        {(file) => (
          <div class="border border-matrix-border/50 rounded bg-matrix-bg text-sm">
            <div class="bg-matrix-border/20 p-2 flex justify-between items-center">
              <span class="text-matrix-primary font-bold">{file.path}</span>
              {/* Ability to remove specific file from context even if part of symbol */}
              <button class="text-xs hover:text-matrix-error">[EXCLUDE]</button>
            </div>
            <pre class="p-2 overflow-x-auto text-xs opacity-80 max-h-48 scrollbar-thin">
              {file.content.slice(0, 300)}...
            </pre>
          </div>
        )}
      </For>
    </div>
  )
}