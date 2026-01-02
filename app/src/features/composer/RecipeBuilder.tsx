import { For, createSignal } from "solid-js";
import { useWorkspace } from "../../core/workspace.store";

export const RecipeBuilder = () => {
  const { state, addToRecipe, removeFromRecipe } = useWorkspace();
  const [isDragging, setIsDragging] = createSignal(false);

  const handleDrop = (e: DragEvent) => {
    e.preventDefault();
    setIsDragging(false);
    
    if (e.dataTransfer) {
      const rawData = e.dataTransfer.getData("application/json");
      if (rawData) {
        try {
          const data = JSON.parse(rawData);
          if (data.type === 'add_file') {
            addToRecipe('add_file', data.value);
          }
        } catch (err) {
          console.error("Invalid drop data");
        }
      }
    }
  };

  const handleDragOver = (e: DragEvent) => {
    e.preventDefault(); // Necessary to allow dropping
    setIsDragging(true);
  };

  const handleDragLeave = () => {
    setIsDragging(false);
  };

  return (
    <div 
      class={`
        space-y-2 min-h-[100px] transition-colors rounded p-2
        ${isDragging() ? 'bg-matrix-primary/10 border-2 border-dashed border-matrix-primary' : ''}
      `}
      onDrop={handleDrop}
      onDragOver={handleDragOver}
      onDragLeave={handleDragLeave}
    >
      <For each={state.recipe}>
        {(step) => (
          <div class="flex items-center justify-between p-2 border border-matrix-border bg-matrix-bg rounded group">
             <div class="flex items-center overflow-hidden">
               <span class="mr-2 opacity-50 text-xs whitespace-nowrap">
                 {step.type === 'add_symbol' ? '[+] SYM' : '[+] FILE'}
               </span>
               <span 
                 class="text-matrix-highlight truncate text-xs" 
                 title={step.value}
               >
                 {step.value}
               </span>
             </div>
             <button 
               onClick={() => removeFromRecipe(step.id)}
               class="text-matrix-error opacity-0 group-hover:opacity-100 px-2 font-bold hover:scale-110 transition"
             >
               x
             </button>
          </div>
        )}
      </For>
      {state.recipe.length === 0 && !isDragging() && (
        <div class="text-center opacity-30 mt-8 text-xs">
          [ DROP FILES HERE OR SEARCH SYMBOLS ]
        </div>
      )}
      {isDragging() && (
        <div class="text-center text-matrix-primary mt-8 text-xs font-bold animate-pulse">
          [ RELEASE TO ADD FILE ]
        </div>
      )}
    </div>
  );
}