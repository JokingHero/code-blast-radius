import { getCurrentWindow } from '@tauri-apps/api/window';
import { createSignal, onMount } from "solid-js";

export const TitleBar = () => {
  const appWindow = getCurrentWindow();
  const [isMaximized, setIsMaximized] = createSignal(false);

  // Check initial state
  onMount(async () => {
    setIsMaximized(await appWindow.isMaximized());
  });

  const minimize = () => appWindow.minimize();
  
  const toggleMaximize = async () => {
    await appWindow.toggleMaximize();
    setIsMaximized(await appWindow.isMaximized());
  };

  const close = () => appWindow.close();

  return (
    <div 
      data-tauri-drag-region 
      class="h-8 bg-matrix-bg border-b border-matrix-border flex items-center justify-between px-3 select-none shrink-0"
    >
      {/* LEFT: App Title / Icon */}
      <div class="flex items-center space-x-2 pointer-events-none opacity-70">
        <div class="w-2 h-2 bg-matrix-primary rounded-full shadow-[0_0_5px_rgba(0,255,65,0.8)] animate-pulse"></div>
        <span class="text-xs font-bold tracking-widest text-matrix-primary">Code Blast Radius</span>
      </div>

      {/* RIGHT: Window Controls */}
      <div class="flex items-center space-x-1">
        
        {/* Minimize */}
        <button 
          onClick={minimize}
          class="w-8 h-full flex items-center justify-center text-matrix-primary hover:bg-matrix-primary/20 transition-colors group"
          title="Minimize"
        >
          <div class="w-3 h-px bg-current group-hover:shadow-[0_0_5px_rgba(0,255,65,0.8)]"></div>
        </button>

        {/* Maximize / Restore */}
        <button 
          onClick={toggleMaximize}
          class="w-8 h-full flex items-center justify-center text-matrix-primary hover:bg-matrix-primary/20 transition-colors"
          title="Maximize"
        >
          {isMaximized() ? (
            // Restore Icon
             <div class="relative w-3 h-3 border border-matrix-primary">
               <div class="absolute -top-1 -right-1 w-3 h-3 border border-matrix-primary bg-matrix-bg"></div>
             </div>
          ) : (
            // Maximize Icon
            <div class="w-3 h-3 border border-matrix-primary shadow-[0_0_2px_rgba(0,255,65,0.5)]"></div>
          )}
        </button>

        {/* Close */}
        <button 
          onClick={close}
          class="w-8 h-full flex items-center justify-center text-matrix-primary hover:bg-matrix-error hover:text-matrix-bg transition-colors"
          title="Close"
        >
          <svg width="10" height="10" viewBox="0 0 10 10" fill="none" stroke="currentColor" stroke-width="1.5">
            <path d="M1 1L9 9M9 1L1 9" />
          </svg>
        </button>
      </div>
    </div>
  );
};