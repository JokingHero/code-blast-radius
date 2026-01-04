import { createStore } from "solid-js/store";
import { invokeWithLoader } from "../lib/tauri";
import { invoke } from "@tauri-apps/api/core";

// ... [Previous Interfaces remain the same] ...

interface RecipeOperation {
  type: 'AddFiles' | 'RemoveFiles' | 'BlastRadius';
  params: {
    pattern?: string; 
    symbol?: string;  
    max_depth?: number;
    exclude_tests?: boolean;
  };
}

interface EngineRecipe {
  name: string;
  description: string | null;
  operations: RecipeOperation[];
  transforms: Record<string, any>;
}

interface ContextFile {
  path: string;
  language: string;
  content: string;
  is_test: boolean;
  relevant_lines: { start: number; end: number }[];
}

interface ContextOutput {
  target: string;
  files: ContextFile[];
}

interface GlobalSettings {
  recent: string[];
  last_opened: string | null;
}

export interface RecipeStep {
  id: string;
  type: 'add_file' | 'add_symbol' | 'remove_file';
  value: string;
}

export interface WorkspaceConfig {
  name: string;
  roots: string[]; 
  mode: 'ad-hoc' | 'project' | 'unsaved-workspace';
}

interface WorkspaceState {
  isInitializing: boolean;
  isLoaded: boolean;
  config: WorkspaceConfig | null;
  recipe: RecipeStep[];
  contextFiles: ContextFile[];
  settings: {
    noTests: boolean;
  }
  recentWorkspaces: string[];
}

const [state, setState] = createStore<WorkspaceState>({
  isInitializing: true,
  isLoaded: false,
  config: null,
  recipe: [],
  contextFiles: [],
  settings: { noTests: true },
  recentWorkspaces: []
});

const debounce = (func: Function, wait: number) => {
  let timeout: number;
  return (...args: any[]) => {
    clearTimeout(timeout);
    timeout = setTimeout(() => func(...args), wait);
  };
};

export const useWorkspace = () => {

  // --- CORE: Analysis ---

  const runAnalysis = async () => {
    const operations: RecipeOperation[] = state.recipe.map(step => {
      if (step.type === 'add_file') {
        return {
          type: 'AddFiles',
          params: { pattern: step.value } 
        };
      } else if (step.type === 'add_symbol') {
        return {
          type: 'BlastRadius',
          params: { 
            symbol: step.value,
            max_depth: 5,
            exclude_tests: state.settings.noTests
          }
        };
      }
      return null;
    }).filter(Boolean) as RecipeOperation[];

    if (operations.length === 0) {
      setState("contextFiles", []);
      return;
    }

    const payload: EngineRecipe = {
      name: "Interactive Session",
      description: null,
      operations: operations,
      transforms: {} 
    };

    try {
      const resultJson = await invoke<string>("execute_recipe", { recipeJson: payload });
      const result: ContextOutput = JSON.parse(resultJson);
      setState("contextFiles", result.files || []);
    } catch (e) {
      console.error("Analysis failed", e);
    }
  };

  const triggerAnalysis = debounce(runAnalysis, 500);

  // --- Actions ---

  const addToRecipe = (type: 'add_file' | 'add_symbol', value: string) => {
    if (state.recipe.find(r => r.value === value && r.type === type)) return;
    setState("recipe", (prev) => [...prev, { id: crypto.randomUUID(), type, value }]);
    triggerAnalysis();
  };

  const removeFromRecipe = (id: string) => {
    setState("recipe", (prev) => prev.filter(r => r.id !== id));
    triggerAnalysis();
  };

  const toggleTests = () => {
    setState("settings", "noTests", (v) => !v);
    triggerAnalysis();
  }

  // --- Workspace Lifecycle ---

  const saveWorkspace = async (path: string) => {
    try {
      const finalPath = path.endsWith('.cblast') ? path : `${path}.cblast`;
      const config = await invokeWithLoader<WorkspaceConfig>("save_current_workspace", { path: finalPath }, "SAVING WORKSPACE");
      setState("config", config);
    } catch (e) {
      console.error("Failed to save workspace", e);
    }
  };
  
  const loadWorkspace = async (path: string) => {
    try {
      const config = await invokeWithLoader<WorkspaceConfig>("set_workspace", { path }, "LOADING WORKSPACE");
      setState({ 
        isLoaded: true, 
        config: config,
        recipe: [],
        contextFiles: []
      });
      refreshRecents();
    } catch (e) {
      console.error("Failed to load workspace:", e);
      setState("isLoaded", false);
    }
  };

  const refreshWorkspace = async () => {
    try {
      await invokeWithLoader("refresh_workspace", {}, "SYNCING FILES");
      // Re-run analysis to reflect file changes in output
      if (state.recipe.length > 0) {
        triggerAnalysis();
      }
    } catch (e) {
      console.error("Failed to refresh", e);
    }
  }

  const refreshRecents = async () => {
    try {
      const settings = await invoke<GlobalSettings>("get_global_settings");
      setState("recentWorkspaces", settings.recent);
    } catch (e) {
      console.warn("Failed to fetch settings", e);
    }
  }

  const clearHistory = async () => {
    try {
        await invoke("clear_recent_history");
        setState("recentWorkspaces", []);
    } catch (e) {
        console.error("Failed to clear history", e);
    }
  }

  const initSession = async () => {
    try {
      const settings = await invoke<GlobalSettings>("get_global_settings");
      setState("recentWorkspaces", settings.recent);
      if (settings.last_opened) {
        await loadWorkspace(settings.last_opened);
      }
    } catch (e) {
      console.error("Initialization failed", e);
    } finally {
      setState("isInitializing", false);
    }
  };

  const addRoot = async (path: string) => {
    try {
      const config = await invokeWithLoader<WorkspaceConfig>("add_root_to_workspace", { rootPath: path }, "ADDING ROOT");
      setState("config", config);
    } catch (e) {
      console.error("Failed to add root", e);
    }
  }

  const removeRoot = async (path: string) => {
    try {
      const config = await invokeWithLoader<WorkspaceConfig>("remove_root_from_workspace", { rootPath: path }, "REMOVING ROOT");
      setState("config", config);
    } catch (e) {
      console.error("Failed to remove root", e);
    }
  }

  return {
    state,
    saveWorkspace,
    loadWorkspace,
    refreshWorkspace,
    initSession,
    addRoot,
    removeRoot,
    clearHistory,
    addToRecipe,
    removeFromRecipe,
    toggleTests
  };
};