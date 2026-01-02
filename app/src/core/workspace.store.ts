import { createStore } from "solid-js/store";
import { invoke } from "@tauri-apps/api/core"; 

export interface RecipeStep {
  id: string;
  type: 'add_file' | 'add_symbol' | 'remove_file';
  value: string; // File path or symbol name
}

interface WorkspaceState {
  isLoaded: boolean;
  rootPath: string;
  recipe: RecipeStep[];
  contextFiles: { path: string; content: string }[]; // The resulting composer content
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
  // Actions
  const loadWorkspace = async (path: string) => {
    try {
      await invoke("set_workspace", { path });
      setState({ isLoaded: true, rootPath: path });
    } catch (e) {
      console.error(e);
    }
  };

  const addToRecipe = async (type: RecipeStep['type'], value: string) => {
    const id = Math.random().toString(36).substr(2, 9);
    setState("recipe", (prev) => [...prev, { id, type, value }]);
    
    // If it's a symbol, immediately resolve context to update "Composer" view
    if (type === 'add_symbol') {
      const jsonStr = await invoke<string>("resolve_recipe", { 
        targetSymbol: value, 
        noTests: state.settings.noTests 
      });
      const result = JSON.parse(jsonStr);
      // Merge logic would go here, simplified for now:
      setState("contextFiles", result.files); 
    }
  };

  const removeFromRecipe = (id: string) => {
    setState("recipe", (prev) => prev.filter(r => r.id !== id));
    // Trigger re-calc of context...
  };

  const toggleTests = () => {
    setState("settings", "noTests", (v) => !v);
  }

  return {
    state,
    loadWorkspace,
    addToRecipe,
    removeFromRecipe,
    toggleTests
  };
};