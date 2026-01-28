import { createStore } from "solid-js/store";
import { invoke } from "@tauri-apps/api/core";
import {
  EngineRecipe,
  SavedRecipe,
  UiRecipeStep,
  RecipeOperation,
  BlastRadiusParams
} from "./types";
import { useWorkspace } from "./workspace.store";

interface RecipeState {
  steps: UiRecipeStep[];
  savedRecipes: SavedRecipe[];
  isExecuting: boolean;
  activeRecipeName: string | null;
  isDirty: boolean;
}

const [state, setState] = createStore<RecipeState>({
  steps: [],
  savedRecipes: [],
  isExecuting: false,
  activeRecipeName: null,
  isDirty: false,
});

export const useRecipe = () => {
  const { 
    state: workspaceState, 
    setContextFiles, 
    setLoading, 
    refreshWorkspace 
  } = useWorkspace();

  const markDirty = () => setState("isDirty", true);

  // Helper to allow UI components to pass loose data that we sanitize here
  type AddStepInput = 
    | { kind: "file"; path: string; mode?: "include" | "exclude" }
    | { kind: "symbol"; name: string; max_depth?: number; exclude_tests?: boolean };

  const addStep = (input: AddStepInput) => {
    const id = crypto.randomUUID();
    let newOp: RecipeOperation;

    if (input.kind === "file") {
      const type = input.mode === "exclude" ? "RemoveFiles" : "AddFiles";
      newOp = {
        type,
        params: { pattern: input.path }
      };
    } else {
      newOp = {
        type: "BlastRadius",
        params: {
          symbol: input.name,
          // Defaults matching Rust/UI logic
          max_depth: input.max_depth ?? 5,
          exclude_tests: input.exclude_tests ?? true,
        },
      };
    }

    const exists = state.steps.find((s) => {
      if (s.op.type !== newOp.type) return false;
      
      if (s.op.type === "AddFiles" && newOp.type === "AddFiles") {
         return s.op.params.pattern === newOp.params.pattern;
      }
      if (s.op.type === "RemoveFiles" && newOp.type === "RemoveFiles") {
         return s.op.params.pattern === newOp.params.pattern;
      }
      if (s.op.type === "BlastRadius" && newOp.type === "BlastRadius") {
         return s.op.params.symbol === newOp.params.symbol;
      }
      return false;
    });

    if (!exists) {
      setState("steps", (prev) => [...prev, { id, op: newOp }]);
      markDirty();
      runAnalysis();
    }
  };

  const removeStep = (id: string) => {
    setState("steps", (prev) => prev.filter((s) => s.id !== id));
    markDirty();
    runAnalysis();
  };

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

  const updateStepParams = (id: string, params: Partial<BlastRadiusParams>) => {
    setState("steps", (step) => step.id === id, "op", "params", (prev: any) => ({
       ...prev, 
       ...params 
    }));
    markDirty();
    runAnalysis();
  };

  const toggleStepType = (id: string) => {
    setState("steps", (step) => step.id === id, "op", (prevOp) => {
      if (prevOp.type === "AddFiles") {
        return { 
            type: "RemoveFiles", 
            params: { pattern: prevOp.params.pattern } 
        } as RecipeOperation;
      }
      if (prevOp.type === "RemoveFiles") {
        return { 
            type: "AddFiles", 
            params: { pattern: prevOp.params.pattern } 
        } as RecipeOperation;
      }
      return prevOp;
    });
    markDirty();
    runAnalysis();
  };

  const runAnalysis = async () => {
    if (state.steps.length === 0) {
      setContextFiles([]);
      return;
    }

    if (workspaceState.isDirty) {
      await refreshWorkspace();
    }

    setState("isExecuting", true);

    const payload: EngineRecipe = {
      name: "Interactive Session",
      description: null,
      operations: state.steps.map((s) => s.op),
      transforms: {},
      default_transform: null, // Always Full Text for V1
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
      default_transform: null, // Always Full Text for V1
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
    
    setState({
      steps: newSteps,
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
          activeRecipeName: null,
          isDirty: false
      });
      setContextFiles([]);
  }

  return {
    recipeState: state,
    addStep,
    removeStep,
    moveStep,
    updateStepParams,
    toggleStepType,
    runAnalysis,
    fetchSavedRecipes,
    saveCurrentRecipe,
    loadSavedRecipe,
    deleteSavedRecipe,
    resetRecipe
  };
};