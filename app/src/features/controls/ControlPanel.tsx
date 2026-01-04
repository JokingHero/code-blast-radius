import { createSignal, Show } from "solid-js";
import { useWorkspace } from "../../core/workspace.store";

export const ControlPanel = () => {
  const { state, toggleTests } = useWorkspace();
  const [showFormatModal, setShowFormatModal] = createSignal(false);

  const handleCopy = () => {
    const text = state.contextFiles.map(f => `// File: ${f.path}\n${f.content}`).join("\n\n");
    navigator.clipboard.writeText(text);
  };

  return (
    <div class="h-full flex flex-col p-2 gap-2 relative">
      
      {/* Header Label for the Panel */}
      <div class="text-tiny uppercase tracking-widest opacity-40 mb-1 text-center border-b border-matrix-border/50 pb-1">
        COMMAND DECK
      </div>

      {/* Settings Group - Auto Grid */}
      <div class="grid grid-cols-1 lg:grid-cols-2 gap-2">
        
        {/* Toggle Tests */}
        <button 
          onClick={toggleTests}
          class={`
            border border-matrix-border p-2 flex flex-col items-center justify-center gap-1 transition-all
            hover:border-matrix-primary hover:bg-matrix-primary/10
            ${!state.settings.noTests ? 'bg-matrix-primary/5 border-matrix-primary/50' : ''}
          `}
          title="Toggle Test Files"
        >
          <div class={`w-2 h-2 rounded-full ${!state.settings.noTests ? 'bg-matrix-primary shadow-glow' : 'bg-matrix-text/30'}`}></div>
          <span class="text-tiny font-bold">TESTS</span>
        </button>

        {/* Add Rule */}
        <button class="border border-matrix-border p-2 flex flex-col items-center justify-center gap-1 hover:border-matrix-highlight hover:bg-matrix-border/50 transition-all">
          <span class="text-xs font-bold">[+]</span>
          <span class="text-tiny font-bold">GREP</span>
        </button>

        {/* Format Options */}
        <button 
          onClick={() => setShowFormatModal(!showFormatModal())}
          class="col-span-1 lg:col-span-2 border border-matrix-border p-2 flex items-center justify-center gap-2 hover:bg-matrix-border/50 transition-all text-tiny"
        >
          <span>{`{}`} FORMAT OPTIONS</span>
        </button>
      </div>

      {/* Spacer to push actions to bottom */}
      <div class="flex-1 min-h-[10px]"></div>

      {/* Primary Actions */}
      <button 
        onClick={handleCopy}
        class="bg-matrix-primary text-matrix-bg font-bold text-xs py-3 hover:shadow-glow hover:bg-matrix-highlight transition active:scale-95 uppercase tracking-widest border border-transparent hover:border-white"
      >
        Copy Context
      </button>

      {/* Format Modal (Positioned absolutely relative to this panel) */}
      <Show when={showFormatModal()}>
        <div class="absolute top-10 right-full mr-2 w-40 bg-matrix-bg border border-matrix-primary p-3 shadow-glow z-50">
          <h3 class="text-tiny font-bold border-b border-matrix-border pb-1 mb-2 text-matrix-primary">OUTPUT MODE</h3>
          <div class="space-y-1 text-xs">
             <label class="flex items-center space-x-2 cursor-pointer hover:bg-matrix-primary/10 p-1">
               <input type="radio" name="fmt" checked class="accent-matrix-primary" />
               <span>Markdown</span>
             </label>
             <label class="flex items-center space-x-2 cursor-pointer hover:bg-matrix-primary/10 p-1">
               <input type="radio" name="fmt" class="accent-matrix-primary" />
               <span>JSON</span>
             </label>
             <label class="flex items-center space-x-2 cursor-pointer hover:bg-matrix-primary/10 p-1">
               <input type="radio" name="fmt" class="accent-matrix-primary" />
               <span>XML</span>
             </label>
          </div>
          <div class="mt-2 pt-2 border-t border-matrix-border text-center">
             <button onClick={() => setShowFormatModal(false)} class="text-tiny hover:text-matrix-error">[CLOSE]</button>
          </div>
        </div>
      </Show>
    </div>
  );
}