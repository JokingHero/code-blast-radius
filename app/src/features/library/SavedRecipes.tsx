import { createEffect, For } from "solid-js";
import { useRecipe } from "../../core/recipe.store";
import { useWorkspace } from "../../core/workspace.store";
export const SavedRecipes = () => {
  const { recipeState, fetchSavedRecipes, loadSavedRecipe, deleteSavedRecipe } =
    useRecipe();
  const { state: workspaceState } = useWorkspace();
  // Refresh list when workspace loads
  createEffect(() => {
    if (workspaceState.isLoaded) {
      fetchSavedRecipes();
    }
  });
  return (
    <div class="flex-1 p-2 overflow-y-auto custom-scrollbar space-y-2">
      <For
        each={recipeState.savedRecipes}
        fallback={
          <div class="text-center opacity-30 text-tiny pt-4">
            NO SAVED RECIPES
          </div>
        }
      >
        {(recipe) => (
          <div class="border border-matrix-border bg-matrix-bg p-2 group hover:border-matrix-primary/50 transition-colors">
            <div class="flex justify-between items-start">
              <div
                class="cursor-pointer"
                onClick={() => loadSavedRecipe(recipe)}
              >
                <div class="text-xs font-bold text-matrix-highlight group-hover:underline decoration-matrix-primary/50 underline-offset-4">
                  {recipe.name}
                </div>
                <div class="text-[10px] opacity-50 mt-1">
                  {recipe.operations.length} STEPS •{" "}
                  {recipe.default_transform ? "X-RAY" : "FULL"}
                </div>
              </div>
              <button
                onClick={() => deleteSavedRecipe(recipe.name)}
                class="opacity-0 group-hover:opacity-100 text-matrix-border hover:text-matrix-error font-bold px-1 transition-opacity"
                title="Delete Recipe"
              >
                x
              </button>
            </div>
          </div>
        )}
      </For>
    </div>
  );
};
