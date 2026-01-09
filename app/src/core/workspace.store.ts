import { createStore } from "solid-js/store";
import { invokeWithLoader } from "../lib/tauri";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

// --- Types ---

export interface ContextFile {
  file_id: number;
  path: string;
  root_name?: string;
  language: string;
  is_test: boolean;
  relevant_lines: { start: number; end: number }[];
  content?: string | null; 
}

export interface RootConfig {
  id: string;
  path: string;
  name: string;
}

export interface WorkspaceConfig {
  name: string;
  roots: RootConfig[];
  mode: 'ad-hoc' | 'project' | 'unsaved-workspace';
}

interface GlobalSettings {
  recent: string[];
  last_opened: string | null;
}

interface WorkspaceState {
  isInitializing: boolean;
  isLoaded: boolean;
  isSyncing: boolean; // Indicates background activity (refresh/indexing)
  isDirty: boolean;   // Indicates file system changes are out of sync with index
  config: WorkspaceConfig | null;
  contextFiles: ContextFile[]; // The output of the current recipe
  recentWorkspaces: string[];
}

// --- State Initialization ---

const [state, setState] = createStore<WorkspaceState>({
  isInitializing: true,
  isLoaded: false,
  isSyncing: false,
  isDirty: false,
  config: null,
  contextFiles: [],
  recentWorkspaces: []
});

// --- Store Actions ---

export const useWorkspace = () => {

  // setters for other stores (like RecipeStore) to modify workspace view state
  const setContextFiles = (files: ContextFile[]) => setState("contextFiles", files);
  const setLoading = (loading: boolean) => setState("isSyncing", loading);

  // --- Event Listeners ---

  const setupListeners = async () => {
    await listen("workspace:dirty", () => {
        // Prevent unnecessary state updates if already dirty
        if (!state.isDirty && !state.isSyncing) {
            setState("isDirty", true);
        }
    });
  };

  // --- Session Management ---

  const refreshRecents = async () => {
    try {
      const settings = await invoke<GlobalSettings>("get_global_settings");
      setState("recentWorkspaces", settings.recent);
    } catch (e) {
      console.warn("Failed to fetch settings", e);
    }
  };

  const initSession = async () => {
    try {
      await setupListeners();
      const settings = await invoke<GlobalSettings>("get_global_settings");
      setState("recentWorkspaces", settings.recent);
      
      if (settings.last_opened) {
        try {
           await loadWorkspace(settings.last_opened);
        } catch (err) {
           setState("isLoaded", false);
           setState("config", null);
        }
      }
    } catch (e) {
      console.error("Initialization failed completely", e);
    } finally {
      setState("isInitializing", false);
    }
  };

  const clearHistory = async () => {
    try {
        await invoke("clear_recent_history");
        setState("recentWorkspaces", []);
    } catch (e) {
        console.error("Failed to clear history", e);
    }
  };

  // --- Workspace Lifecycle ---

  const setWorkspaceState = (config: WorkspaceConfig) => {
    setState({
      isLoaded: true,
      config: config,
      // We clear context files on a fresh load/mode switch to avoid stale data
      contextFiles: [],
      // A fresh load implies we are in sync with what was just loaded
      isDirty: false 
    });
  };

  const loadWorkspace = async (path: string) => {
    try {
      const config = await invokeWithLoader<WorkspaceConfig>("set_workspace", { path }, "LOADING WORKSPACE");
      setWorkspaceState(config);
      await refreshRecents();
    } catch (e: any) {
      if (typeof e === 'string' && e.includes("ERR_WORKSPACE_NOT_FOUND")) {
        console.warn(`Workspace at ${path} not found. Removing from history.`);
        // Refreshing recents will pull the updated list (minus the zombie) from the backend
        await refreshRecents();
        setState("isLoaded", false);
      } else {
        console.error("Failed to load workspace:", e);
        setState("isLoaded", false);
      }
    }
  };

  const saveWorkspace = async (path: string) => {
    try {
      // Ensure extension
      const finalPath = path.endsWith('.cblast') ? path : `${path}.cblast`;
      
      const config = await invokeWithLoader<WorkspaceConfig>("save_current_workspace", { path: finalPath }, "SAVING WORKSPACE");
      setState("config", config);
      await refreshRecents();
    } catch (e) {
      console.error("Failed to save workspace", e);
    }
  };

  const refreshWorkspace = async () => {
    try {
      setState("isSyncing", true);
      await invoke("refresh_workspace");
      // Successful refresh means index matches disk
      setState("isDirty", false);
    } catch (e) {
      console.error("Failed to refresh", e);
    } finally {
      setState("isSyncing", false);
    }
  };

  // --- Root Management ---

  const addRoot = async (path: string) => {
    try {
      const config = await invokeWithLoader<WorkspaceConfig>("add_root_to_workspace", { rootPath: path }, "ADDING ROOT");
      setState("config", config);
      // Adding a root triggers a scan in backend, so we are fresh
      setState("isDirty", false);
    } catch (e) {
      console.error("Failed to add root", e);
    }
  };

  const removeRoot = async (path: string) => {
    try {
      const config = await invokeWithLoader<WorkspaceConfig>("remove_root_from_workspace", { rootPath: path }, "REMOVING ROOT");
      setState("config", config);
      // Removing a root updates the graph, so we are fresh
      setState("isDirty", false);
    } catch (e) {
      console.error("Failed to remove root", e);
    }
  };

  return {
    state,
    setContextFiles,
    setLoading,
    initSession,
    refreshRecents,
    clearHistory,
    loadWorkspace,
    saveWorkspace,
    refreshWorkspace,
    addRoot,
    removeRoot
  };
};