import { For, Show } from "solid-js";
import { useWorkspace } from "../../core/workspace.store";

export const ContextComposer = () => {
  const { state } = useWorkspace();

  const handleCopy = () => {
    const text = state.contextFiles.map(f => `// File: ${f.path}\n${f.content}`).join("\n\n");
    navigator.clipboard.writeText(text);
  };

  const canCopy = () => state.contextFiles.length > 0 && !state.isSyncing;

  return (
    <div class="h-full flex flex-col min-h-0 bg-matrix-panel/20">
      
      {/* Header with integrated Actions */}
      <div class="sticky top-0 bg-matrix-panel/90 backdrop-blur z-10 px-4 h-10 border-b border-matrix-border/30 flex justify-between items-center shrink-0">
        <span class="text-xs uppercase tracking-widest opacity-40 font-bold">
            Context Output Stream
        </span>
        
        <button 
            onClick={handleCopy}
            disabled={!canCopy()}
            class={`
                text-xs font-bold uppercase tracking-widest px-3 py-1 border transition-all
                ${canCopy() 
                    ? 'border-matrix-primary text-matrix-primary hover:bg-matrix-primary hover:text-matrix-bg hover:shadow-glow' 
                    : 'border-transparent text-matrix-primary/20 cursor-not-allowed'}
            `}
        >
            {state.isSyncing ? "SYNCING..." : "[ COPY OUTPUT ]"}
        </button>
      </div>

      <div class="flex-1 p-4 pt-2 overflow-y-auto custom-scrollbar">
        <Show when={state.contextFiles.length === 0}>
            <div class="flex flex-col items-center justify-center h-full opacity-20 text-xs font-mono tracking-widest text-center select-none">
            <div class="mb-2 text-4xl opacity-50">∅</div>
            <div>AWAITING INPUT</div>
            <div class="text-[10px] mt-2">ADD SYMBOLS TO GENERATE CONTEXT</div>
            </div>
        </Show>

        <div class="space-y-4 pb-4">
            <For each={state.contextFiles}>
            {(file) => (
                <div class="border border-matrix-border/50 rounded bg-matrix-bg overflow-hidden animate-[fadeIn_0.3s_ease-out]">
                <div class="bg-matrix-border/20 p-2 flex justify-between items-center border-b border-matrix-border/30">
                    <div class="flex items-center gap-2">
                        <span class="text-[10px] uppercase opacity-50 border border-matrix-border px-1 rounded">
                            {file.language}
                        </span>
                        <span class="text-matrix-primary font-bold text-sm truncate font-mono" title={file.path}>
                            {file.path.split(/[/\\]/).slice(-2).join('/')}
                        </span>
                    </div>
                    <div class="text-tiny opacity-50 font-mono">
                        {file.relevant_lines?.length || 0} SECTIONS
                    </div>
                </div>
                <pre class="p-3 overflow-x-auto text-xs opacity-80 max-h-[500px] scrollbar-thin leading-relaxed font-mono selection:bg-matrix-primary selection:text-matrix-bg">
                    {file.content}
                </pre>
                </div>
            )}
            </For>
        </div>
      </div>
    </div>
  )
}