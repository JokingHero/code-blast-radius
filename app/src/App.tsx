import "./App.css";
import { Show, onMount, For } from "solid-js";
import { open } from "@tauri-apps/plugin-dialog";
import { useWorkspace } from "./core/workspace.store";
import { TitleBar } from "./ui/TitleBar";
import { GlobalLoader } from "./ui/GlobalLoader";
import { SearchBar } from "./features/search/SearchBar";
import { FileExplorer } from "./features/explorer/FileExplorer";
import { RecipeBuilder } from "./features/composer/RecipeBuilder";
import { ContextComposer } from "./features/composer/ContextComposer";
import { ControlPanel } from "./features/controls/ControlPanel";
import { SavedRecipes } from "./features/library/SavedRecipes"; 

function App() {
  const { state, loadWorkspace, initSession } = useWorkspace();
  
  onMount(() => {
    initSession();
  });

  const handleOpenFolder = async () => {
    const selected = await open({ directory: true });
    if (selected && typeof selected === "string") {
      await loadWorkspace(selected);
    }
  };

  const handleOpenWorkspace = async () => {
    const selected = await open({
      multiple: false,
      directory: false,
      filters: [{ name: "Blast Radius Workspace", extensions: ["cblast"] }],
    });
    if (selected && typeof selected === "string") {
      await loadWorkspace(selected);
    }
  };

  const WelcomeScreen = () => (
    <div class="h-full w-full flex flex-col items-center justify-center relative overflow-hidden select-none bg-matrix-bg">
      <div class="absolute inset-0 pointer-events-none overflow-hidden">
        <div class="absolute top-1/4 left-1/4 w-96 h-96 bg-matrix-primary/5 rounded-full blur-[100px] animate-pulse"></div>
        <div class="absolute bottom-1/4 right-1/4 w-[500px] h-[500px] bg-matrix-primary/5 rounded-full blur-[120px] animate-pulse delay-1000"></div>
        <div class="absolute top-0 left-0 w-full h-full bg-[radial-gradient(circle_at_center,transparent_0%,#050505_100%)]"></div>
      </div>
      <div class="z-10 text-center mb-16 relative">
        <div class="w-32 h-32 mx-auto mb-8 relative group cursor-default">
          <div class="absolute inset-0 border border-matrix-primary/30 rounded-full animate-[spin_8s_linear_infinite]"></div>
          <div class="absolute inset-4 border border-matrix-primary/20 rounded-full animate-[spin_12s_linear_infinite_reverse]"></div>
          <div class="absolute inset-0 flex items-center justify-center">
            <div class="w-4 h-4 bg-matrix-primary shadow-glow rounded-sm rotate-45 group-hover:scale-150 transition-transform duration-500"></div>
          </div>
          <div class="absolute inset-0 bg-matrix-primary/5 rounded-full blur-xl group-hover:bg-matrix-primary/20 transition-all duration-500"></div>
        </div>

        <h1 class="text-5xl font-bold tracking-[0.2em] text-matrix-primary drop-shadow-glow mb-2">
          BLAST RADIUS
        </h1>
        <p class="text-xs text-matrix-primary/50 tracking-[0.5em] uppercase">
          Visual Code Dependency Analyzer
        </p>
      </div>

      <div class="z-10 flex flex-col gap-4 w-72">
        <button
          onClick={handleOpenFolder}
          class="group relative px-6 py-4 border border-matrix-primary/30 bg-matrix-panel/50 hover:border-matrix-primary hover:bg-matrix-primary/10 transition-all text-sm font-bold tracking-widest uppercase flex items-center justify-between overflow-hidden"
        >
          <div class="absolute inset-0 bg-matrix-primary/5 translate-x-[-100%] group-hover:translate-x-0 transition-transform duration-300"></div>
          <span class="relative z-10">Open Folder</span>
          <span class="relative z-10 opacity-50 group-hover:opacity-100 transition-opacity group-hover:translate-x-1 duration-300">
            →
          </span>
        </button>

        <button
          onClick={handleOpenWorkspace}
          class="group relative px-6 py-4 border border-matrix-primary/30 bg-matrix-panel/50 hover:border-matrix-primary hover:bg-matrix-primary/10 transition-all text-sm font-bold tracking-widest uppercase flex items-center justify-between overflow-hidden"
        >
          <div class="absolute inset-0 bg-matrix-primary/5 translate-x-[-100%] group-hover:translate-x-0 transition-transform duration-300"></div>
          <span class="relative z-10">Load Workspace</span>
          <span class="relative z-10 opacity-50 group-hover:opacity-100 transition-opacity group-hover:translate-x-1 duration-300">
            →
          </span>
        </button>
      </div>

      <Show when={state.recentWorkspaces.length > 0}>
        <div class="mt-16 text-center z-10 animate-[fadeIn_1s_ease-out]">
          <p class="text-[10px] text-matrix-primary/30 uppercase tracking-widest mb-4">
            Recent Sessions
          </p>
          <div class="flex flex-col gap-2 items-center">
            <For each={state.recentWorkspaces.slice(0, 3)}>
              {(path) => (
                <button
                  onClick={() => loadWorkspace(path)}
                  class="text-xs text-matrix-primary/60 hover:text-matrix-highlight hover:underline decoration-matrix-primary/30 underline-offset-4 truncate max-w-md transition-colors font-mono"
                >
                  {path}
                </button>
              )}
            </For>
          </div>
        </div>
      </Show>
    </div>
  );

  return (
    <div class="h-screen w-screen bg-matrix-bg flex flex-col border border-matrix-border overflow-hidden font-mono text-matrix-primary selection:bg-matrix-primary selection:text-matrix-bg">
      <GlobalLoader />
      <TitleBar />
      <div class="flex-1 flex flex-col overflow-hidden relative">
        <Show when={!state.isInitializing}>
          <Show when={state.isLoaded} fallback={<WelcomeScreen />}>
            <div class="flex-1 flex overflow-hidden">
              {/* LEFT: File Explorer (WIDER) */}
              <div class="w-80 border-r border-matrix-border flex flex-col bg-black/40 relative z-50">
                <div class="h-12 shrink-0 flex items-center px-3 border-b border-matrix-border bg-matrix-panel select-none justify-between">
                  <div class="flex items-center">
                    <div class="w-2 h-2 bg-matrix-text rounded-full mr-2 opacity-50"></div>
                    <span class="text-tiny font-bold tracking-wider opacity-70 text-matrix-primary">
                      EXPLORER
                    </span>
                  </div>
                  <div class="flex gap-1">
                    <Show when={state.config?.mode === 'project'}>
                      <span 
                        class="text-micro border border-matrix-border px-1 opacity-50 rounded bg-matrix-panel text-matrix-highlight"
                        title="This is a saved workspace project (.cblast)."
                      >
                        PROJ
                      </span>
                    </Show>
                    <Show when={state.config?.mode === 'unsaved-workspace'}>
                      <span 
                        class="text-micro border border-matrix-error px-1 rounded bg-matrix-error/10 text-matrix-error animate-pulse"
                        title="Unsaved multi-root workspace. Use '[ SAVE ]' to create a .cblast file."
                      >
                        UNSAVED
                      </span>
                    </Show>
                  </div>
                </div>
                <div class="flex-1 overflow-hidden relative">
                  <FileExplorer />
                  <div class="absolute bottom-0 left-0 w-full h-8 bg-gradient-to-t from-black/80 to-transparent pointer-events-none"></div>
                </div>
              </div>

              {/* MIDDLE: Composer Area */}
              <div class="flex-1 flex flex-col min-w-0 border-r border-matrix-border bg-matrix-bg relative">
                <div class="h-12 shrink-0 border-b border-matrix-border bg-matrix-panel relative z-30">
                  <SearchBar />
                </div>
                <div class="flex flex-row border-b border-matrix-border max-h-[40%] min-h-[180px] shrink-0 bg-matrix-bg/50">
                  <div class="flex-1 flex flex-col border-r border-matrix-border overflow-hidden relative">
                    <div class="sticky top-0 bg-matrix-bg/95 backdrop-blur z-10 px-4 py-1 border-b border-matrix-border/30 flex justify-between items-center h-8 shrink-0">
                      <span class="text-tiny uppercase tracking-widest opacity-40">
                        Active Recipe
                      </span>
                    </div>
                    <div class="p-2 overflow-y-auto custom-scrollbar flex-1">
                      <RecipeBuilder />
                    </div>
                  </div>
                  <div class="w-36 lg:w-44 bg-matrix-panel/30 shrink-0 overflow-y-auto custom-scrollbar">
                    <ControlPanel />
                  </div>
                </div>
                <div class="flex-1 flex flex-col min-h-0 bg-matrix-panel/20">
                  <div class="sticky top-0 bg-matrix-panel/90 backdrop-blur z-10 px-4 py-1 border-b border-matrix-border/30 h-8 flex items-center shrink-0">
                    <span class="text-tiny uppercase tracking-widest opacity-40">
                      Context Output Stream
                    </span>
                  </div>
                  <div class="flex-1 p-4 pt-2 overflow-y-auto custom-scrollbar">
                    <ContextComposer />
                  </div>
                </div>
              </div>

              {/* RIGHT: Saved Recipes */}
              <div class="w-64 bg-matrix-panel/50 flex flex-col">
                <div class="h-12 shrink-0 flex items-center px-3 border-b border-matrix-border bg-matrix-panel select-none justify-between">
                  <span class="text-tiny font-bold tracking-wider opacity-70 text-matrix-primary">
                    SAVED RECIPES
                  </span>
                </div>
                <SavedRecipes />
              </div>
            </div>
          </Show>
        </Show>
      </div>
    </div>
  );
}

export default App;