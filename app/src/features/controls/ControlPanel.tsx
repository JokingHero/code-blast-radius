import { createSignal, Show } from "solid-js";
import { useWorkspace } from "../../core/workspace.store";
import { useRecipe } from "../../core/recipe.store";

export const ControlPanel = () => {
  const { state: workspaceState } = useWorkspace();
  const { recipeState, setViewMode, saveCurrentRecipe } = useRecipe();
  const [showSaveModal, setShowSaveModal] = createSignal(false);
  const [saveName, setSaveName] = createSignal("");
  let inputRef: HTMLInputElement | undefined;

  const handleCopy = () => {
    const text = workspaceState.contextFiles.map(f => `// File: ${f.path}\n${f.content}`).join("\n\n");
    navigator.clipboard.writeText(text);
  };

  const openSaveModal = () => {
    setSaveName("");
    setShowSaveModal(true);
    // Focus input on next tick
    setTimeout(() => inputRef?.focus(), 50);
  };

  const handleSave = async () => {
    if (saveName().trim()) {
      await saveCurrentRecipe(saveName());
      setShowSaveModal(false);
    }
  };

  return (
    <div class="h-full flex flex-col p-2 gap-2 relative">
      
      {/* Header */}
      <div class="text-tiny uppercase tracking-widest opacity-40 mb-1 text-center border-b border-matrix-border/50 pb-1 select-none">
        COMMAND DECK
      </div>

      {/* --- VIEW MODES --- */}
      <div class="flex border border-matrix-border rounded overflow-hidden select-none">
        <button 
          onClick={() => setViewMode('full')}
          class={`flex-1 py-1 text-tiny font-bold transition-colors ${recipeState.viewMode === 'full' ? 'bg-matrix-primary text-matrix-bg' : 'hover:bg-matrix-primary/10'}`}
          title="Full Text: Show complete file content"
        >
          FULL
        </button>
        <button 
          onClick={() => setViewMode('skeleton')}
          class={`flex-1 py-1 text-tiny font-bold transition-colors ${recipeState.viewMode === 'skeleton' ? 'bg-matrix-primary text-matrix-bg' : 'hover:bg-matrix-primary/10'}`}
          title="X-Ray Mode: Hides function bodies to reduce token count"
        >
          X-RAY
        </button>
      </div>

      <button 
        onClick={openSaveModal}
        disabled={recipeState.steps.length === 0}
        class={`
            border border-matrix-border p-2 flex items-center justify-center gap-1 transition-all text-tiny font-bold uppercase
            ${recipeState.steps.length === 0 ? 'opacity-50 cursor-not-allowed' : 'hover:border-matrix-highlight hover:bg-matrix-border/50'}
        `}
      >
        [ Save Recipe ]
      </button>

      {/* Spacer to push actions to bottom */}
      <div class="flex-1 min-h-[10px]"></div>

      {/* Primary Action */}
      <button 
        onClick={handleCopy}
        disabled={workspaceState.contextFiles.length === 0}
        class={`
            bg-matrix-primary text-matrix-bg font-bold text-xs py-3 
            uppercase tracking-widest border border-transparent 
            transition active:scale-95
            ${workspaceState.contextFiles.length === 0 
                ? 'opacity-50 cursor-not-allowed bg-matrix-border text-matrix-primary' 
                : 'hover:shadow-glow hover:bg-matrix-highlight hover:border-white'}
        `}
      >
        Copy Output
      </button>

      {/* --- Save Modal Overlay --- */}
      <Show when={showSaveModal()}>
        <div class="absolute inset-0 bg-matrix-bg/95 z-50 flex flex-col items-center justify-center p-2 border border-matrix-primary animate-[fadeIn_0.1s_ease-out]">
          <div class="text-tiny font-bold mb-2 text-matrix-highlight">NAME RECIPE</div>
          
          <input 
            ref={inputRef}
            type="text" 
            value={saveName()}
            onInput={(e) => setSaveName(e.currentTarget.value)}
            class="bg-matrix-panel border border-matrix-border text-matrix-primary px-2 py-1 text-xs w-full mb-2 outline-none focus:border-matrix-primary focus:shadow-glow font-mono"
            placeholder="e.g. Auth Audit"
            onKeyDown={(e) => {
                if (e.key === 'Enter') handleSave();
                if (e.key === 'Escape') setShowSaveModal(false);
            }}
          />
          
          <div class="flex gap-2 w-full">
            <button 
                onClick={() => setShowSaveModal(false)} 
                class="flex-1 border border-matrix-border text-tiny py-1 hover:bg-matrix-error hover:text-matrix-bg transition-colors"
            >
                CANCEL
            </button>
            <button 
                onClick={handleSave} 
                class="flex-1 bg-matrix-primary text-matrix-bg text-tiny font-bold hover:bg-matrix-highlight transition-colors"
            >
                SAVE
            </button>
          </div>
        </div>
      </Show>
    </div>
  );
}