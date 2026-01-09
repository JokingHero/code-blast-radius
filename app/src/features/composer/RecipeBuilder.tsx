import { For, createSignal, Show, Switch, Match } from "solid-js";
import { useRecipe } from "../../core/recipe.store";
import { UiRecipeStep } from "../../core/types";
import { invoke } from "@tauri-apps/api/core";

interface ResolvedPathDTO {
  original: string;
  relative_path: string | null;
  root_id: string | null;
  is_indexed: boolean;
}

const StepItem = (props: { step: UiRecipeStep; index: number; onMove: (from: number, to: number) => void }) => {
  const { removeStep, updateStepParams, toggleStepType } = useRecipe();
  const [isHovering, setIsHovering] = createSignal(false);

  // DnD Handlers
  const handleDragStart = (e: DragEvent) => {
    e.dataTransfer?.setData("text/plain", props.index.toString());
    e.dataTransfer?.setData("type", "reorder_step");
    e.dataTransfer!.effectAllowed = "move";
  };

  const handleDragOver = (e: DragEvent) => {
    e.preventDefault(); 
    e.dataTransfer!.dropEffect = "move";
  };

  const handleDrop = (e: DragEvent) => {
    e.preventDefault();
    e.stopPropagation();
    const fromIndexStr = e.dataTransfer?.getData("text/plain");
    const type = e.dataTransfer?.getData("type");
    if (type === "reorder_step" && fromIndexStr !== undefined) {
        const fromIndex = parseInt(fromIndexStr);
        if (!isNaN(fromIndex)) props.onMove(fromIndex, props.index);
    }
  };

  const isInfinite = () => (props.step.op.params.max_depth || 0) > 20;
  const depth = () => isInfinite() ? 5 : (props.step.op.params.max_depth || 5);

  return (
    <div 
        draggable={true}
        onDragStart={handleDragStart}
        onDragOver={handleDragOver}
        onDrop={handleDrop}
        onMouseEnter={() => setIsHovering(true)}
        onMouseLeave={() => setIsHovering(false)}
        class="
            border border-matrix-border bg-matrix-bg p-2 text-sm 
            transition-colors hover:border-matrix-primary/50 cursor-grab active:cursor-grabbing
            relative group/item
        "
    >
      {/* Header Row */}
      <div class="flex items-center justify-between gap-2">
        
        {/* Left: Indicator & Name */}
        <div class="flex items-center gap-3 overflow-hidden min-w-0 flex-1">
          <Switch>
            <Match when={props.step.op.type === 'AddFiles'}>
              <button 
                onClick={() => toggleStepType(props.step.id)}
                class="
                    w-5 h-5 flex items-center justify-center 
                    text-matrix-bg bg-matrix-primary font-bold 
                    hover:bg-matrix-highlight transition-colors
                "
                title="Include Mode (Click to Exclude)"
              >
                +
              </button>
            </Match>
            <Match when={props.step.op.type === 'RemoveFiles'}>
              <button 
                onClick={() => toggleStepType(props.step.id)}
                class="
                    w-5 h-5 flex items-center justify-center 
                    text-matrix-bg bg-matrix-primary font-bold 
                    hover:bg-matrix-highlight transition-colors
                "
                title="Exclude Mode (Click to Include)"
              >
                -
              </button>
            </Match>
            <Match when={props.step.op.type === 'BlastRadius'}>
<div class="w-5 h-5 flex items-center justify-center text-matrix-primary border border-matrix-primary">
                   <span class="text-sm font-bold">?</span>
               </div>
            </Match>
          </Switch>
          
          <span 
            class="truncate font-mono font-bold text-matrix-highlight/90 text-base flex-1" 
            title={props.step.op.params.pattern || props.step.op.params.symbol}
          >
            {props.step.op.params.pattern || props.step.op.params.symbol}
          </span>
        </div>
        
        {/* Right: Delete Button */}
        <button 
          onClick={() => removeStep(props.step.id)}
          class={`
            text-matrix-primary hover:text-matrix-error font-bold px-1 shrink-0 transition-opacity
            ${isHovering() ? 'opacity-100' : 'opacity-0'}
          `}
        >
          [x]
        </button>
      </div>

      {/* Controls Row */}
      <Show when={props.step.op.type === 'BlastRadius'}>
        <div class="mt-2 pt-2 border-t border-matrix-border/30 flex items-center gap-3 animate-[fadeIn_0.2s_ease-out]">
          
            {/* Radius */}
            <div class="flex items-center gap-2">
<button 
                     onClick={() => updateStepParams(props.step.id, { max_depth: isInfinite() ? 5 : 100 })}
                     class={`
                         text-lg uppercase font-bold tracking-wider transition-colors
                         ${!isInfinite() ? "text-matrix-primary" : "text-matrix-primary/40 line-through decoration-matrix-primary/40"}
                     `}
                 >
                     RAD
                 </button>

                <Show 
                    when={!isInfinite()}
                    fallback={
<div class="flex items-center justify-center w-16 h-4 bg-matrix-primary/5 border border-matrix-primary/20">
                              <span class="text-base font-bold text-matrix-primary">∞</span>
                         </div>
                    }
                >
                    <div class="flex items-center gap-2">
                        <input 
                            type="range" 
                            min="1" 
                            max="10" 
                            value={depth()}
                            onInput={(e) => updateStepParams(props.step.id, { max_depth: parseInt(e.currentTarget.value) })}
                            class="
                                w-16 h-1 appearance-none bg-matrix-border/50 cursor-pointer
                                [&::-webkit-slider-thumb]:appearance-none 
                                [&::-webkit-slider-thumb]:w-2.5 
                                [&::-webkit-slider-thumb]:h-2.5
                                [&::-webkit-slider-thumb]:bg-matrix-primary 
                                [&::-webkit-slider-thumb]:hover:scale-125
                                [&::-webkit-slider-thumb]:transition-transform
                            "
                        />
                        <span class="text-sm font-mono font-bold text-matrix-primary w-3 text-right">
                            {depth()}
                        </span>
                    </div>
                </Show>
            </div>

            <div class="w-px h-3 bg-matrix-border/50"></div>

            {/* Tests */}
            <button
                onClick={() => updateStepParams(props.step.id, { exclude_tests: !props.step.op.params.exclude_tests })}
                class={`
                    h-5 px-2 text-lg font-bold uppercase tracking-wider border transition-all flex items-center
                    ${!props.step.op.params.exclude_tests 
                        ? "bg-matrix-primary text-matrix-bg border-matrix-primary" 
                        : "bg-transparent text-matrix-primary/50 border-matrix-border hover:border-matrix-primary/50 hover:text-matrix-primary"}
                `}
            >
                TESTS
            </button>
        </div>
      </Show>
    </div>
  )
}

export const RecipeBuilder = () => {
  const { recipeState, addStep, resetRecipe, moveStep } = useRecipe();
  const [isDraggingFile, setIsDraggingFile] = createSignal(false);

  const handleOuterDrop = async (e: DragEvent) => {
    e.preventDefault();
    setIsDraggingFile(false);
    
    if (!e.dataTransfer) return;

    const type = e.dataTransfer.getData("type");
    if (type === "reorder_step") return;

    // 1. Collect Paths to Resolve
    const pathsToResolve: string[] = [];

    // Check for Internal Drag (from FileExplorer)
    const rawData = e.dataTransfer.getData("application/json");
    if (rawData) {
      try {
        const data = JSON.parse(rawData);
        if (data.type === 'add_file' && data.value) {
          pathsToResolve.push(data.value);
        }
      } catch (err) { /* ignore */ }
    } 
    // Check for External OS Drag
    else if (e.dataTransfer.files.length > 0) {
      // Note: Tauri's drag-drop implementation usually handles files specifically,
      // but for now we assume the internal explorer is the primary source.
      // If handling OS files, you'd iterate e.dataTransfer.files here.
    }

    if (pathsToResolve.length === 0) return;

    // 2. Resolve Absolute -> Relative via Backend
    try {
      const resolved = await invoke<ResolvedPathDTO[]>("resolve_paths", { 
        paths: pathsToResolve 
      });

      // 3. Add Valid Steps
      for (const res of resolved) {
        if (res.relative_path) {
          addStep({ type: 'file', value: res.relative_path });
        } else {
          console.warn("Dropped file is outside of any active workspace root:", res.original);
          // Optional: Add a toast notification here
        }
      }
    } catch (err) {
      console.error("Failed to resolve paths", err);
    }
  };

  return (
    <div class="flex flex-col h-full bg-matrix-bg/50 relative">
        
      {/* Header */}
      <div class="sticky top-0 bg-matrix-bg/95 backdrop-blur z-10 px-4 h-10 border-b border-matrix-border/30 flex justify-between items-center shrink-0">
         <div class="flex items-center gap-2">
            <span class="text-base uppercase tracking-widest opacity-40 font-bold">
                Active Session:
            </span>
            <span class={`text-base font-bold font-mono ${recipeState.activeRecipeName ? 'text-matrix-highlight' : 'text-matrix-primary/50 italic'}`}>
                {recipeState.activeRecipeName || "Untitled"}
            </span>
            <Show when={recipeState.isDirty}>
                <span class="text-matrix-highlight font-bold animate-pulse">*</span>
            </Show>
         </div>

         <Show when={recipeState.steps.length > 0}>
<button 
                 onClick={resetRecipe}
                 class="
                     flex items-center gap-1 text-lg font-bold uppercase tracking-wider 
                     text-matrix-primary hover:bg-matrix-primary hover:text-matrix-bg
                     px-2 py-1 transition-all border border-transparent hover:border-matrix-primary
                 "
              >
                 [ CLEAR ]
              </button>
         </Show>
      </div>

      <div 
        class={`
            flex-1 p-2 overflow-y-auto custom-scrollbar space-y-2 transition-colors
            ${isDraggingFile() ? 'bg-matrix-primary/10 border-2 border-dashed border-matrix-primary' : ''}
        `}
        onDrop={handleOuterDrop}
        onDragOver={(e) => { e.preventDefault(); setIsDraggingFile(true); }}
        onDragLeave={() => setIsDraggingFile(false)}
      >
        <For each={recipeState.steps}>
            {(step, index) => (
                <StepItem step={step} index={index()} onMove={moveStep} />
            )}
        </For>

        {recipeState.steps.length === 0 && !isDraggingFile() && (
            <div class="flex flex-col items-center justify-center h-full opacity-30 select-none pointer-events-none">
<div class="text-base tracking-widest font-bold">[ WORKBENCH EMPTY ]</div>
                 <div class="text-sm mt-1 font-mono">DROP FILES OR SEARCH SYMBOLS</div>
            </div>
        )}
      </div>
    </div>
  );
}