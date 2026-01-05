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
}

const [state, setState] = createStore<RecipeState>({
  steps: [],
  savedRecipes: [],
  viewMode: "full",
  isExecuting: false,
});

export const useRecipe = () => {
  const { setContextFiles, setLoading } = useWorkspace();

  const addStep = (
    step: Partial<UiRecipeStep> & { type: "file" | "symbol"; value: string }
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
            max_depth: 5,
            exclude_tests: true,
          },
        },
      };
    }

    const exists = state.steps.find(
      (s) =>
        s.op.type === newStep.op.type &&
        (s.op.params.pattern === newStep.op.params.pattern ||
          s.op.params.symbol === newStep.op.params.symbol)
    );

    if (!exists) {
      setState("steps", (prev) => [...prev, newStep]);
      runAnalysis();
    }
  };

  const removeStep = (id: string) => {
    setState("steps", (prev) => prev.filter((s) => s.id !== id));
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
    runAnalysis();
  };

  const setViewMode = (mode: "full" | "skeleton") => {
    setState("viewMode", mode);
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
    });

    runAnalysis();
  };

  const deleteSavedRecipe = async (name: string) => {
    try {
      await invoke("delete_named_recipe", { name });
      await fetchSavedRecipes();
    } catch (e) {
      console.error(e);
    }
  };

  return {
    recipeState: state,
    addStep,
    removeStep,
    updateStepParams,
    toggleStepType, // Export the new function
    setViewMode,
    runAnalysis,
    fetchSavedRecipes,
    saveCurrentRecipe,
    loadSavedRecipe,
    deleteSavedRecipe,
  };
};