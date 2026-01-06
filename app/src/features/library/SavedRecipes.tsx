import { createEffect, createSignal, For, Show } from "solid-js";
import { useRecipe } from "../../core/recipe.store";
import { useWorkspace } from "../../core/workspace.store";

export const SavedRecipes = () => {
  const {
    recipeState,
    fetchSavedRecipes,
    loadSavedRecipe,
    deleteSavedRecipe,
    saveCurrentRecipe,
  } = useRecipe();
  
  const { state: workspaceState } = useWorkspace();
  const [filter, setFilter] = createSignal("");
  const [showSaveModal, setShowSaveModal] = createSignal(false);
  const [saveName, setSaveName] = createSignal("");
  const [errorMsg, setErrorMsg] = createSignal(""); // Collision error state
  
  let inputRef: HTMLInputElement | undefined;

  // Refresh list when workspace loads
  createEffect(() => {
    if (workspaceState.isLoaded) {
      fetchSavedRecipes();
    }
  });

  const filteredRecipes = () => {
    const term = filter().toLowerCase();
    if (!term) return recipeState.savedRecipes;
    return recipeState.savedRecipes.filter((r) =>
      r.name.toLowerCase().includes(term)
    );
  };

  // --- Actions ---

  const handleUpdate = async () => {
    if (recipeState.activeRecipeName) {
        await saveCurrentRecipe(recipeState.activeRecipeName);
    }
  };

  const openSaveModal = () => {
    setSaveName("");
    setErrorMsg("");
    setShowSaveModal(true);
    setTimeout(() => inputRef?.focus(), 50);
  };

  const checkNameCollision = (name: string) => {
    const exists = recipeState.savedRecipes.some(r => r.name.toLowerCase() === name.toLowerCase());
    if (exists) {
        setErrorMsg("NAME ALREADY EXISTS");
        return true;
    }
    setErrorMsg("");
    return false;
  };

  const confirmSaveAs = async () => {
    const name = saveName().trim();
    if (!name) return;

    if (checkNameCollision(name)) {
        return;
    }

    await saveCurrentRecipe(name);
    setShowSaveModal(false);
  };

  return (
    <div class="flex flex-col h-full bg-matrix-panel/50 border-l border-matrix-border relative">
      
      {/* --- HEADER: Actions --- */}
      <div class="shrink-0 flex divide-x divide-matrix-border border-b border-matrix-border h-10 bg-matrix-panel">
            <button
                onClick={handleUpdate}
                disabled={!recipeState.activeRecipeName || workspaceState.isSyncing}
                class={`
                    flex-1 font-bold text-xs uppercase tracking-wider transition-all flex items-center justify-center gap-2
                    ${!recipeState.activeRecipeName 
                        ? 'text-matrix-primary/30 cursor-not-allowed' 
                        : 'text-matrix-primary hover:bg-matrix-primary hover:text-matrix-bg'}
                `}
                title="Overwrite current recipe"
            >
                [ UPDATE ]
            </button>
            <button
                onClick={openSaveModal}
                disabled={recipeState.steps.length === 0 || workspaceState.isSyncing}
                class={`
                    flex-1 font-bold text-xs uppercase tracking-wider transition-all flex items-center justify-center gap-2
                    ${recipeState.steps.length === 0
                        ? 'text-matrix-primary/30 cursor-not-allowed' 
                        : 'text-matrix-primary hover:bg-matrix-primary hover:text-matrix-bg'}
                `}
                title="Save as new recipe"
            >
                [ SAVE ]
            </button>
      </div>

      {/* --- SEARCH BAR (File Explorer Style) --- */}
      <div class="p-2 border-b border-matrix-border/50 bg-matrix-bg/80 shrink-0 flex gap-2">
          <div class="relative flex-1 group">
            <input 
                type="text"
                placeholder="Filter recipes..."
                value={filter()}
                onInput={(e) => setFilter(e.currentTarget.value)}
                class="w-full bg-matrix-panel border border-matrix-border/50 text-matrix-highlight px-2 pl-7 py-1 text-xs outline-none focus:border-matrix-primary focus:shadow-glow font-mono transition-all"
            />
            {/* Search Icon */}
            <div class="absolute left-2 top-1/2 -translate-y-1/2 opacity-50 pointer-events-none group-focus-within:opacity-100 group-focus-within:text-matrix-primary">
                <svg width="10" height="10" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="3">
                    <circle cx="11" cy="11" r="8"></circle>
                    <line x1="21" y1="21" x2="16.65" y2="16.65"></line>
                </svg>
            </div>
            <Show when={filter()}>
                <button 
                    onClick={() => setFilter("")}
                    class="absolute right-2 top-1/2 -translate-y-1/2 text-matrix-primary hover:text-matrix-error"
                >
                    ✕
                </button>
            </Show>
          </div>
      </div>

      {/* --- LIST --- */}
      <div class="flex-1 overflow-y-auto custom-scrollbar p-0">
        <For
          each={filteredRecipes()}
          fallback={
            <div class="text-center opacity-30 text-xs font-mono pt-8 tracking-widest">
              {filter() ? "NO MATCHES" : "LIBRARY EMPTY"}
            </div>
          }
        >
          {(recipe) => (
            <div 
                class={`
                    group flex items-center justify-between px-3 py-2
                    border-b border-matrix-border/30 cursor-pointer select-none transition-colors
                    ${recipeState.activeRecipeName === recipe.name 
                        ? 'bg-matrix-primary/10 text-matrix-highlight' 
                        : 'hover:bg-matrix-primary/5 text-matrix-primary/80 hover:text-matrix-highlight'}
                `}
                onClick={() => loadSavedRecipe(recipe)}
            >
                {/* Left: Name */}
                <div class="font-bold text-sm font-mono truncate mr-2 flex-1" title={recipe.name}>
                    {recipe.name}
                </div>

                {/* Right: Meta & Actions */}
                <div class="flex items-center gap-3 shrink-0">
                    <div class="flex items-center gap-1 opacity-50 text-xs font-mono" title={`${recipe.operations.length} Operations`}>
                        <span class="font-bold">{recipe.operations.length}</span>
                        <span class="text-[10px]">⚡</span> 
                    </div>

                    <button
                        onClick={(e) => { e.stopPropagation(); deleteSavedRecipe(recipe.name); }}
                        class="opacity-0 group-hover:opacity-100 text-matrix-primary hover:text-matrix-error font-bold transition-opacity px-1"
                        title="Delete Recipe"
                    >
                        [x]
                    </button>
                </div>
            </div>
          )}
        </For>
      </div>

      {/* --- SAVE MODAL --- */}
      <Show when={showSaveModal()}>
        <div class="absolute inset-0 z-50 bg-matrix-bg/95 backdrop-blur flex items-center justify-center p-4 animate-[fadeIn_0.1s_ease-out]">
            <div class="w-full max-w-xs border border-matrix-primary p-1 bg-matrix-panel shadow-glow">
                <div class="text-xs font-bold text-matrix-highlight bg-matrix-primary/20 p-2 mb-2 text-center uppercase tracking-widest">
                    Name Recipe
                </div>
                
                <input 
                    ref={inputRef}
                    type="text"
                    value={saveName()}
                    onInput={(e) => {
                        setSaveName(e.currentTarget.value);
                        setErrorMsg(""); // clear error on type
                    }}
                    onKeyDown={(e) => {
                        if (e.key === 'Enter') confirmSaveAs();
                        if (e.key === 'Escape') setShowSaveModal(false);
                    }}
                    class={`
                        w-full bg-matrix-bg border p-2 text-base outline-none mb-1 font-mono
                        ${errorMsg() 
                            ? 'border-matrix-error text-matrix-error focus:border-matrix-error placeholder:text-matrix-error/50' 
                            : 'border-matrix-border text-matrix-primary focus:border-matrix-primary'}
                    `}
                    placeholder="Recipe Name"
                />
                
                {/* Error Message Display */}
                <div class="h-4 mb-1 text-[10px] text-matrix-error font-bold text-center uppercase tracking-wider">
                    {errorMsg()}
                </div>

                <div class="flex gap-2">
                    <button 
                        onClick={() => setShowSaveModal(false)}
                        class="flex-1 py-2 text-xs font-bold border border-matrix-border hover:bg-matrix-error hover:text-matrix-bg transition"
                    >
                        CANCEL
                    </button>
                    <button 
                        onClick={confirmSaveAs}
                        disabled={!!errorMsg()}
                        class={`
                            flex-1 py-2 text-xs font-bold transition
                            ${errorMsg() 
                                ? 'bg-matrix-border text-matrix-primary/30 cursor-not-allowed' 
                                : 'bg-matrix-primary text-matrix-bg hover:bg-matrix-highlight'}
                        `}
                    >
                        SAVE
                    </button>
                </div>
            </div>
        </div>
      </Show>

    </div>
  );
};