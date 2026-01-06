import { getCurrentWindow } from '@tauri-apps/api/window';
import { getVersion } from '@tauri-apps/api/app';
import { createSignal, onMount, Show } from "solid-js";
import { useWorkspace } from '../core/workspace.store';

export const TitleBar = () => {
  const appWindow = getCurrentWindow();
  const [isMaximized, setIsMaximized] = createSignal(false);
  const [version, setVersion] = createSignal("0.0.0");
  const { state } = useWorkspace(); 

  onMount(async () => {
    setIsMaximized(await appWindow.isMaximized());
    try {
      const v = await getVersion();
      setVersion(v);
    } catch (e) {
      console.warn("Failed to get app version", e);
    }
  });

  const minimize = () => appWindow.minimize();
  
  const toggleMaximize = async () => {
    await appWindow.toggleMaximize();
    setIsMaximized(await appWindow.isMaximized());
  };

  const close = () => appWindow.close();

  // Shared button class for consistent hitboxes and focus states
  const btnClass = "w-12 h-full flex items-center justify-center transition-colors focus:outline-none cursor-default";

  return (
    <div 
      data-tauri-drag-region 
      class="h-8 bg-matrix-bg border-b border-matrix-border flex items-center justify-between px-3 select-none shrink-0 relative"
    >
      {/* LEFT: App Title / Icon / Status */}
      <div class="flex items-center space-x-2 pointer-events-none opacity-90">
        {/* Logo/Icon */}
        <div class="w-2 h-2 bg-matrix-primary rounded-full shadow-[0_0_5px_rgba(0,255,65,0.8)] animate-pulse"></div>
        
        {/* Title & Version */}
        <div class="flex items-baseline gap-2">
            <span class="text-base font-bold tracking-widest text-matrix-primary">Code Blast Radius</span>
            <span class="text-sm text-matrix-primary/40 font-mono">v{version()}</span>
        </div>
        
        {/* Status: Syncing */}
        <Show when={state.isSyncing}>
            <div class="flex items-center space-x-2 ml-4 px-2 py-0.5 bg-matrix-primary/10 border border-matrix-primary/30 rounded">
                <div class="w-2 h-2 border border-matrix-primary border-t-transparent rounded-full animate-spin"></div>
                <span class="text-sm text-matrix-primary animate-pulse">SYNCING...</span>
            </div>
        </Show>

        {/* Status: Out of Sync (Dirty) */}
        <Show when={state.isDirty && !state.isSyncing}>
             <div class="flex items-center space-x-2 ml-4 px-2 py-0.5 bg-matrix-error/10 border border-matrix-error/50 rounded animate-[pulse_3s_infinite]">
                <span class="text-sm text-matrix-error font-bold tracking-wider">⚠ OUT OF SYNC</span>
            </div>
        </Show>
      </div>

      {/* RIGHT: Window Controls 
          Added z-50 to ensure they sit above the drag region
      */}
      <div class="flex items-center h-full absolute right-0 top-0 z-50">
        
        {/* Minimize */}
        <button 
          type="button"
          onClick={minimize}
          class={`${btnClass} text-matrix-primary hover:bg-matrix-primary/20`}
          title="Minimize"
        >
          {/* Simple Dash Line */}
          <div class="w-3 h-px bg-current pointer-events-none"></div>
        </button>

        {/* Maximize / Restore */}
        <button 
          type="button"
          onClick={toggleMaximize}
          class={`${btnClass} text-matrix-primary hover:bg-matrix-primary/20`}
          title="Maximize"
        >
          {isMaximized() ? (
             // Restore Icon (Overlapping Squares)
             <div class="relative w-3 h-3 border border-current pointer-events-none">
               <div class="absolute -top-1 -right-1 w-3 h-3 border border-current bg-matrix-bg"></div>
             </div>
          ) : (
            // Maximize Icon (Single Square)
            <div class="w-3 h-3 border border-current pointer-events-none"></div>
          )}
        </button>

        {/* Close */}
        <button 
          type="button"
          onClick={close}
          class={`${btnClass} text-matrix-primary hover:bg-matrix-error hover:text-matrix-bg`}
          title="Close"
        >
          {/* SVG X for perfect centering and scaling */}
          <svg width="10" height="10" viewBox="0 0 10 10" class="fill-current pointer-events-none">
            <path d="M1 0L0 1L4 5L0 9L1 10L5 6L9 10L10 9L6 5L10 1L9 0L5 4L1 0Z" />
          </svg>
        </button>
      </div>
    </div>
  );
};