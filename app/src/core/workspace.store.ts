import { createStore } from "solid-js/store";
import { invokeWithLoader } from "../lib/tauri";

export interface RecipeStep {
  id: string;
  type: 'add_file' | 'add_symbol' | 'remove_file';
  value: string;
}

interface WorkspaceState {
  isLoaded: boolean;
  rootPath: string;
  recipe: RecipeStep[];
  contextFiles: { path: string; content: string }[];
  settings: {
    noTests: boolean;
  }
}

const [state, setState] = createStore<WorkspaceState>({
  isLoaded: false,
  rootPath: "",
  recipe: [],
  contextFiles: [],
  settings: { noTests: true }
});

export const useWorkspace = () => {
  
  const loadWorkspace = async (path: string) => {
    try {
      // Wrapped in loader with specific message
      await invokeWithLoader("set_workspace", { path }, "INDEXING WORKSPACE");
      setState({ isLoaded: true, rootPath: path });
    } catch (e) {
      console.error(e);
      // Ideally, set an error state in the store here to show a toast
    }
  };

  const addToRecipe = async (type: RecipeStep['type'], value: string) => {
    const id = Math.random().toString(36).substr(2, 9);
    setState("recipe", (prev) => [...prev, { id, type, value }]);
    
    if (type === 'add_symbol') {
      try {
        // Resolving dependencies can be heavy, so we show a loader
        const jsonStr = await invokeWithLoader<string>(
            "resolve_recipe", 
            { targetSymbol: value, noTests: state.settings.noTests },
            "TRACING DEPENDENCIES" 
        );
        const result = JSON.parse(jsonStr);
        setState("contextFiles", result.files); 
      } catch (e) {
        console.error("Failed to resolve recipe", e);
      }
    }
  };

  const removeFromRecipe = (id: string) => {
    setState("recipe", (prev) => prev.filter(r => r.id !== id));
    // TODO: Trigger re-calc of context...
  };

  const toggleTests = () => {
    setState("settings", "noTests", (v) => !v);
    // If we have items, we might want to re-resolve here too, potentially needing a loader
  }

  return {
    state,
    loadWorkspace,
    addToRecipe,
    removeFromRecipe,
    toggleTests
  };
};