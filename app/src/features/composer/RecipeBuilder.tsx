import { For, createSignal, Show, Switch, Match } from "solid-js";
import { useRecipe } from "../../core/recipe.store";
import { UiRecipeStep } from "../../core/types";

const StepItem = (props: { step: UiRecipeStep }) => {
  const { removeStep, updateStepParams, toggleStepType } = useRecipe();

  return (
    <div class="border border-matrix-border bg-matrix-bg rounded p-2 text-xs group transition-colors hover:border-matrix-primary/50">
      
      {/* Header Row */}
      <div class="flex items-center justify-between mb-1">
        <div class="flex items-center gap-2 overflow-hidden">
          <Switch>
            <Match when={props.step.op.type === 'AddFiles'}>
              <button 
                onClick={() => toggleStepType(props.step.id)}
                class="text-matrix-highlight font-bold shrink-0 hover:underline decoration-dashed underline-offset-4 cursor-pointer"
                title="Click to Switch to EXCLUDE"
              >
                [ INCLUDE ]
              </button>
            </Match>
            <Match when={props.step.op.type === 'RemoveFiles'}>
              <button 
                onClick={() => toggleStepType(props.step.id)}
                class="text-matrix-error font-bold shrink-0 hover:underline decoration-dashed underline-offset-4 cursor-pointer"
                title="Click to Switch to INCLUDE"
              >
                [ EXCLUDE ]
              </button>
            </Match>
            <Match when={props.step.op.type === 'BlastRadius'}>
              <span class="text-matrix-primary font-bold shrink-0">[ RADIUS ]</span>
            </Match>
          </Switch>
          
          <span 
            class="opacity-70 truncate font-mono" 
            title={props.step.op.params.pattern || props.step.op.params.symbol}
          >
            {props.step.op.params.pattern || props.step.op.params.symbol}
          </span>
        </div>
        
        <button 
          onClick={() => removeStep(props.step.id)}
          class="text-matrix-border hover:text-matrix-error transition font-bold px-1 shrink-0"
          title="Remove Step"
        >
          x
        </button>
      </div>

      {/* Controls Row (Only for BlastRadius) */}
      <Show when={props.step.op.type === 'BlastRadius'}>
        <div class="flex items-center gap-4 mt-2 pl-2 border-l border-matrix-border/30 animate-[fadeIn_0.2s_ease-out]">
          
          {/* Depth Slider */}
          <div class="flex items-center gap-2" title="Traversal Depth">
            <span class="opacity-50 text-[10px] uppercase">Depth</span>
            <input 
              type="range" 
              min="1" 
              max="10" 
              value={props.step.op.params.max_depth || 5}
              onInput={(e) => updateStepParams(props.step.id, { max_depth: parseInt(e.currentTarget.value) })}
              class="w-16 accent-matrix-primary h-1 bg-matrix-border rounded-lg appearance-none cursor-pointer"
            />
            <span class="font-bold font-mono">{props.step.op.params.max_depth || 5}</span>
          </div>

          {/* Tests Toggle */}
          <label class="flex items-center gap-2 cursor-pointer select-none group/toggle">
            <input 
              type="checkbox"
              checked={props.step.op.params.exclude_tests ?? true}
              onChange={(e) => updateStepParams(props.step.id, { exclude_tests: e.currentTarget.checked })}
              class="accent-matrix-primary w-3 h-3 cursor-pointer bg-transparent border border-matrix-primary"
            />
            <span class="opacity-50 text-[10px] uppercase group-hover/toggle:text-matrix-primary transition-colors">No Tests</span>
          </label>

        </div>
      </Show>
    </div>
  )
}

export const RecipeBuilder = () => {
  const { recipeState, addStep } = useRecipe();
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
            addStep({ type: 'file', value: data.value });
          }
        } catch (err) {
          console.error("Invalid drop data");
        }
      }
    }
  };

  const handleDragOver = (e: DragEvent) => {
    e.preventDefault();
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
      <For each={recipeState.steps}>
        {(step) => <StepItem step={step} />}
      </For>

      {recipeState.steps.length === 0 && !isDragging() && (
        <div class="flex flex-col items-center justify-center h-24 opacity-30 select-none pointer-events-none">
          <div class="text-xs tracking-widest">[ WORKBENCH EMPTY ]</div>
          <div class="text-[10px] mt-1">DROP FILES OR SEARCH SYMBOLS</div>
        </div>
      )}
      
      {isDragging() && (
        <div class="flex items-center justify-center h-24 text-matrix-primary text-xs font-bold animate-pulse pointer-events-none">
          [ RELEASE TO ADD FILE ]
        </div>
      )}
    </div>
  );
}