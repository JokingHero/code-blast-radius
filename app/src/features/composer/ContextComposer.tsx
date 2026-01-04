import { For, Show } from "solid-js";
import { useWorkspace } from "../../core/workspace.store";

export const ContextComposer = () => {
  const { state } = useWorkspace();

  return (
    <div class="h-full">
      <Show when={state.contextFiles.length === 0}>
        <div class="flex flex-col items-center justify-center h-full opacity-20 text-tiny font-mono tracking-widest text-center">
          <div class="mb-2 text-4xl opacity-50">∅</div>
          <div>AWAITING INPUT</div>
          <div>ADD SYMBOLS OR FILES TO GENERATE CONTEXT</div>
        </div>
      </Show>

      <div class="space-y-4 pb-4">
        <For each={state.contextFiles}>
          {(file) => (
            <div class="border border-matrix-border/50 rounded bg-matrix-bg text-sm overflow-hidden animate-[fadeIn_0.3s_ease-out]">
              <div class="bg-matrix-border/20 p-2 flex justify-between items-center border-b border-matrix-border/30">
                <div class="flex items-center gap-2">
                    <span class="text-tiny uppercase opacity-50 border border-matrix-border px-1 rounded">
                        {file.language}
                    </span>
                    <span class="text-matrix-primary font-bold text-xs truncate" title={file.path}>
                        {file.path.split(/[/\\]/).slice(-2).join('/')}
                    </span>
                </div>
                <div class="text-tiny opacity-50">
                    {file.relevant_lines?.length || 0} SECTIONS
                </div>
              </div>
              <pre class="p-2 overflow-x-auto text-tiny opacity-80 max-h-64 scrollbar-thin leading-relaxed">
                {file.content}
              </pre>
            </div>
          )}
        </For>
      </div>
    </div>
  )
}