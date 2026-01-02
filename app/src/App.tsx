import './App.css'

import { Show } from "solid-js";
import { open } from "@tauri-apps/plugin-dialog";

import { useWorkspace } from "./core/workspace.store";
import { TitleBar } from "./ui/TitleBar";
import { GlobalLoader } from "./ui/GlobalLoader"; // Import
import { SearchBar } from "./features/search/SearchBar";
import { FileExplorer } from "./features/explorer/FileExplorer";
import { RecipeBuilder } from "./features/composer/RecipeBuilder";
import { ContextComposer } from "./features/composer/ContextComposer";
import { ControlPanel } from "./features/controls/ControlPanel";

function App() {
  const { state, loadWorkspace } = useWorkspace();

  const handleOpenFolder = async () => {
    const selected = await open({ directory: true });
    if (selected) {
      await loadWorkspace(selected);
    }
  };

  return (
    <div class="h-screen w-screen bg-matrix-bg flex flex-col border border-matrix-border overflow-hidden font-mono text-matrix-primary selection:bg-matrix-primary selection:text-matrix-bg">
      {/* 1. Global Components Layer */}
      <GlobalLoader /> 
      <TitleBar />

      <div class="flex-1 flex flex-col overflow-hidden relative">
        <Show when={!state.isLoaded}>
          {/* ... existing Hero ... */}
          <div class="h-full flex flex-col items-center justify-center space-y-8 bg-[radial-gradient(ellipse_at_center,_var(--tw-gradient-stops))] from-matrix-panel to-matrix-bg">
            <div class="text-center space-y-2">
              <div class="text-5xl font-bold tracking-widest text-matrix-highlight drop-shadow-[0_0_10px_rgba(0,255,65,0.5)]">
                CODE BLAST RADIUS
              </div>
              <div class="text-xs tracking-[0.5em] opacity-50">
                MANAGE CONTEXT LIKE A SURGEON
              </div>
            </div>
            <button 
              onClick={handleOpenFolder}
              class="px-8 py-3 border border-matrix-primary text-matrix-primary hover:bg-matrix-primary hover:text-matrix-bg transition shadow-glow uppercase tracking-widest text-sm cursor-pointer"
            >
              [ INITIALIZE WORKSPACE ]
            </button>
          </div>
        </Show>

        <Show when={state.isLoaded}>
             {/* ... existing main layout ... */}
             <div class="flex-1 flex overflow-hidden">
                <div class="w-64 border-r border-matrix-border flex flex-col bg-black/40">
                    <div class="h-12 shrink-0 flex items-center px-3 border-b border-matrix-border bg-matrix-panel select-none">
                        <div class="w-2 h-2 bg-matrix-text rounded-full mr-2 opacity-50"></div>
                        <span class="text-[10px] font-bold tracking-wider opacity-70 text-matrix-primary">PROJECT FILES</span>
                    </div>
                    <div class="flex-1 overflow-hidden relative">
                        <FileExplorer />
                        <div class="absolute bottom-0 left-0 w-full h-8 bg-gradient-to-t from-black/80 to-transparent pointer-events-none"></div>
                    </div>
                </div>

                <div class="flex-1 flex flex-col min-w-0 border-r border-matrix-border bg-matrix-bg relative">
                    <div class="h-12 shrink-0 border-b border-matrix-border bg-matrix-panel relative z-30">
                        <SearchBar />
                    </div>
                    <div class="flex flex-row border-b border-matrix-border max-h-[40%] min-h-[180px] shrink-0 bg-matrix-bg/50">
                        <div class="flex-1 flex flex-col border-r border-matrix-border overflow-hidden relative">
                            <div class="sticky top-0 bg-matrix-bg/95 backdrop-blur z-10 px-4 py-1 border-b border-matrix-border/30 flex justify-between items-center h-8 shrink-0">
                                <span class="text-[9px] uppercase tracking-widest opacity-40">Active Recipe</span>
                                <span class="text-[9px] opacity-30 font-mono">{state.recipe.length.toString().padStart(2, '0')} ITEMS</span>
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
                            <span class="text-[9px] uppercase tracking-widest opacity-40">Context Output Stream</span>
                        </div>
                        <div class="flex-1 p-4 pt-2 overflow-y-auto custom-scrollbar">
                            <ContextComposer />
                        </div>
                    </div>
                </div>

                <div class="w-64 bg-matrix-panel/50 flex flex-col">
                    <div class="h-12 shrink-0 flex items-center px-3 border-b border-matrix-border bg-matrix-panel select-none justify-between">
                        <span class="text-[10px] font-bold tracking-wider opacity-70 text-matrix-primary">SAVED RECIPES</span>
                        <button class="text-[10px] border border-matrix-border px-1 hover:bg-matrix-primary hover:text-matrix-bg transition">+</button>
                    </div>
                    <div class="flex-1 p-2 overflow-y-auto custom-scrollbar space-y-2">
                        <div class="group border border-matrix-border/50 hover:border-matrix-primary bg-matrix-bg p-3 cursor-pointer transition-all hover:shadow-glow relative overflow-hidden">
                            <div class="text-xs font-bold text-matrix-highlight mb-1 group-hover:text-white">Fix Auth Bug</div>
                            <div class="text-[9px] opacity-40 flex gap-2 font-mono">
                                <span>4 FILES</span>
                                <span>|</span>
                                <span>2 SYMS</span>
                            </div>
                        </div>
                    </div>
                </div>
            </div>
        </Show>
      </div>
    </div>
  );
}

export default App;