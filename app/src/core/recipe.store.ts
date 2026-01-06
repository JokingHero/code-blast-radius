import { createStore } from "solid-js/store";
import { invoke } from "@tauri-apps/api/core";
import {
  EngineRecipe,
  SavedRecipe,
  UiRecipeStep,
  FileTransformMode,
} from "./types";
import { useWorkspace } from "./workspace.store";

interface RecipeState {
  steps: UiRecipeStep[];
  savedRecipes: SavedRecipe[];
  viewMode: "full" | "skeleton";
  isExecuting: boolean;
  activeRecipeName: string | null;
  isDirty: boolean;
}

const [state, setState] = createStore<RecipeState>({
  steps: [],
  savedRecipes: [],
  viewMode: "full",
  isExecuting: false,
  activeRecipeName: null,
  isDirty: false,
});

export const useRecipe = () => {
  const { setContextFiles, setLoading } = useWorkspace();

  const markDirty = () => setState("isDirty", true);

  const addStep = (
    step: Partial<UiRecipeStep> & {
      type: "file" | "symbol";
      value: string;
      params?: any;
    }
  ) => {
    const id = crypto.randomUUID();
    let newStep: UiRecipeStep;
    
    if (step.type === "file") {
      newStep = {
        id,
        op: { type: "AddFiles", params: { pattern: step.value } },
      };
    } else {
      newStep = {
        id,
        op: {
          type: "BlastRadius",
          params: {
            symbol: step.value,
            max_depth: step.params?.max_depth ?? 5,
            exclude_tests: step.params?.exclude_tests ?? true,
          },
        },
      };
    }

    // --- BUG FIX START ---
    const exists = state.steps.find((s) => {
        if (s.op.type !== newStep.op.type) return false;
        
        // Check specifically based on type to avoid undefined===undefined false positives
        if (s.op.type === 'AddFiles' || s.op.type === 'RemoveFiles') {
            return s.op.params.pattern === newStep.op.params.pattern;
        }
        if (s.op.type === 'BlastRadius') {
            return s.op.params.symbol === newStep.op.params.symbol;
        }
        return false;
    });
    // --- BUG FIX END ---

    if (!exists) {
      setState("steps", (prev) => [...prev, newStep]);
      markDirty();
      runAnalysis();
    }
  };

  const removeStep = (id: string) => {
    setState("steps", (prev) => prev.filter((s) => s.id !== id));
    markDirty();
    runAnalysis();
  };

  // New Action for Reordering
  const moveStep = (fromIndex: number, toIndex: number) => {
      if (fromIndex === toIndex) return;
      
      setState("steps", (prev) => {
          const newSteps = [...prev];
          const [moved] = newSteps.splice(fromIndex, 1);
          newSteps.splice(toIndex, 0, moved);
          return newSteps;
      });
      markDirty();
      runAnalysis();
  };

  const updateStepParams = (id: string, params: any) => {
    setState(
      "steps",
      (step) => step.id === id,
      "op",
      "params",
      (prev) => ({ ...prev, ...params })
    );
    markDirty();
    runAnalysis();
  };

  const toggleStepType = (id: string) => {
    setState("steps", (step) => step.id === id, "op", (prevOp) => {
      if (prevOp.type === "AddFiles") {
        return { ...prevOp, type: "RemoveFiles" as const };
      }
      if (prevOp.type === "RemoveFiles") {
        return { ...prevOp, type: "AddFiles" as const };
      }
      return prevOp;
    });
    markDirty();
    runAnalysis();
  };

  const setViewMode = (mode: "full" | "skeleton") => {
    setState("viewMode", mode);
    markDirty();
    runAnalysis();
  };

  const runAnalysis = async () => {
    if (state.steps.length === 0) {
      setContextFiles([]);
      return;
    }
    setState("isExecuting", true);

    let defaultTransform: FileTransformMode | null = null;
    if (state.viewMode === "skeleton") {
      defaultTransform = { mode: "FocusOn", symbols: [] };
    }

    const payload: EngineRecipe = {
      name: "Interactive Session",
      description: null,
      operations: state.steps.map((s) => s.op),
      transforms: {},
      default_transform: defaultTransform,
    };

    try {
      const resultJson = await invoke<string>("execute_recipe", {
        recipeJson: payload,
      });
      const result = JSON.parse(resultJson);
      setContextFiles(result.files || []);
    } catch (e) {
      console.error("Analysis failed", e);
    } finally {
      setState("isExecuting", false);
    }
  };

  const fetchSavedRecipes = async () => {
    try {
      const recipes = await invoke<SavedRecipe[]>("get_saved_recipes");
      recipes.sort((a, b) => a.name.localeCompare(b.name));
      setState("savedRecipes", recipes);
    } catch (e) {
      console.error("Failed to fetch recipes", e);
    }
  };

  const saveCurrentRecipe = async (name: string) => {
    const payload: EngineRecipe = {
      name,
      description: "Saved via GUI",
      operations: state.steps.map((s) => s.op),
      transforms: {},
      default_transform:
        state.viewMode === "skeleton" ? { mode: "FocusOn", symbols: [] } : null,
    };
    try {
      setLoading(true);
      await invoke("save_named_recipe", { recipeJson: payload });
      await fetchSavedRecipes();
      setState("activeRecipeName", name);
      setState("isDirty", false);
    } catch (e) {
      console.error(e);
    } finally {
      setLoading(false);
    }
  };

  const loadSavedRecipe = (recipe: SavedRecipe) => {
    const newSteps = recipe.operations.map((op) => ({
      id: crypto.randomUUID(),
      op,
    }));
    const newViewMode = recipe.default_transform ? "skeleton" : "full";

    setState({
      steps: newSteps,
      viewMode: newViewMode,
      activeRecipeName: recipe.name,
      isDirty: false,
    });

    runAnalysis();
  };

  const deleteSavedRecipe = async (name: string) => {
    try {
      await invoke("delete_named_recipe", { name });
      await fetchSavedRecipes();
      if (state.activeRecipeName === name) {
        setState("activeRecipeName", null);
        setState("isDirty", true);
      }
    } catch (e) {
      console.error(e);
    }
  };

  const resetRecipe = () => {
      setState({
          steps: [],
          viewMode: 'full',
          activeRecipeName: null,
          isDirty: false
      });
      setContextFiles([]);
  }

  return {
    recipeState: state,
    addStep,
    removeStep,
    moveStep, // Exported
    updateStepParams,
    toggleStepType,
    setViewMode,
    runAnalysis,
    fetchSavedRecipes,
    saveCurrentRecipe,
    loadSavedRecipe,
    deleteSavedRecipe,
    resetRecipe
  };
};