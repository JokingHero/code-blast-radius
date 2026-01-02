import { createSignal, createEffect, For, Show, createMemo } from "solid-js";
import { readDir, DirEntry } from "@tauri-apps/plugin-fs";
import { useWorkspace } from "../../core/workspace.store";

// --- Helpers ---

/**
 * Robust Path Joiner (Cross-Platform).
 * Detects the OS separator from the parent path string itself.
 * This ensures C:\Windows + System32 becomes C:\Windows\System32
 * and /home/user + docs becomes /home/user/docs
 */
const joinPath = (parent: string, name: string): string => {
  // 1. Guess separator: if we see a backslash, assume Windows style.
  const isWindows = parent.includes('\\');
  const sep = isWindows ? '\\' : '/';

  // 2. Remove trailing separator from parent to avoid "C:\\" or "//"
  const cleanParent = parent.endsWith(sep) 
    ? parent.slice(0, -1) 
    : parent;

  return `${cleanParent}${sep}${name}`;
};

// --- Components ---

const ExplorerItem = (props: { entry: DirEntry; parentPath: string; depth: number }) => {
  const [isOpen, setIsOpen] = createSignal(false);
  const [children, setChildren] = createSignal<DirEntry[]>([]);
  const [isLoading, setIsLoading] = createSignal(false);
  const [errorMsg, setErrorMsg] = createSignal<string | null>(null);

  // Construct full path once
  const fullPath = joinPath(props.parentPath, props.entry.name);
  
  // Sorting: Folders first, then Files
  const sortedChildren = createMemo(() => {
    return [...children()].sort((a, b) => {
      if (a.isDirectory !== b.isDirectory) return a.isDirectory ? -1 : 1;
      return a.name.localeCompare(b.name);
    });
  });

  const toggleNode = async (e: MouseEvent) => {
    e.stopPropagation();
    
    if (!props.entry.isDirectory) return;

    if (isOpen()) {
      setIsOpen(false);
      return;
    }

    setIsLoading(true);
    setErrorMsg(null);
    
    try {
      const entries = await readDir(fullPath);
      setChildren(entries);
      setIsOpen(true);
    } catch (err: any) {
      console.error(`[Explorer] Failed to read ${fullPath}:`, err);
      // Capture error for tooltip
      setErrorMsg(typeof err === "string" ? err : err.message || "Access Denied");
    } finally {
      setIsLoading(false);
    }
  };

  const handleDragStart = (e: DragEvent) => {
    if (e.dataTransfer) {
      e.dataTransfer.setData("application/json", JSON.stringify({
        type: "add_file",
        value: fullPath
      }));
      e.dataTransfer.effectAllowed = "copy";
      if (e.currentTarget instanceof HTMLElement) e.currentTarget.style.opacity = "0.5";
    }
  };

  const handleDragEnd = (e: DragEvent) => {
    if (e.currentTarget instanceof HTMLElement) e.currentTarget.style.opacity = "1";
  }

  return (
    <div class="select-none font-mono">
      <div 
        onClick={toggleNode}
        draggable={!props.entry.isDirectory}
        onDragStart={handleDragStart}
        onDragEnd={handleDragEnd}
        class={`
          group flex items-center py-0.5 px-2 cursor-pointer 
          hover:bg-matrix-primary/10 border-l-2 border-transparent
          transition-all duration-100 relative
          ${isOpen() ? 'border-matrix-primary bg-matrix-primary/5' : ''}
          ${isLoading() ? 'cursor-wait' : ''}
        `}
        style={{ "padding-left": `${props.depth * 12 + 8}px` }}
        // TOOLTIP: Hover over this to see exact path or error
        title={errorMsg() ? `Error: ${errorMsg()}` : fullPath}
      >
        <Show when={props.depth > 0}>
           <div class="absolute left-0 top-0 bottom-0 w-px bg-matrix-border/30" style={{ left: `${(props.depth * 12)}px` }}></div>
        </Show>

        <div class="w-4 mr-1 flex items-center justify-center shrink-0">
          <Show when={isLoading()}>
            <div class="w-2 h-2 border border-matrix-primary border-t-transparent rounded-full animate-spin"></div>
          </Show>
          <Show when={!isLoading() && props.entry.isDirectory}>
             <span class="text-[10px] font-bold text-matrix-primary opacity-70 group-hover:text-matrix-highlight">
               {isOpen() ? '[-]' : '[+]'}
             </span>
          </Show>
        </div>

        <span class={`
          truncate text-xs tracking-tight select-none
          ${props.entry.isDirectory ? "text-matrix-primary font-bold" : "text-matrix-text group-hover:text-matrix-primary"}
          ${errorMsg() ? "text-matrix-error line-through decoration-matrix-error opacity-70" : ""}
        `}>
          {props.entry.name}
        </span>
        
        <Show when={errorMsg()}>
          <span class="ml-2 text-[8px] font-bold text-matrix-bg bg-matrix-error px-1 rounded-sm cursor-help">
            ERR
          </span>
        </Show>
      </div>

      <Show when={isOpen()}>
        <div class="relative">
          <For each={sortedChildren()}>
            {(child) => (
              <ExplorerItem 
                entry={child} 
                parentPath={fullPath} 
                depth={props.depth + 1} 
              />
            )}
          </For>
          <Show when={!isLoading() && sortedChildren().length === 0 && !errorMsg()}>
            <div 
              class="text-[10px] opacity-30 italic py-1"
              style={{ "padding-left": `${(props.depth + 1) * 12 + 24}px` }}
            >
              (empty)
            </div>
          </Show>
        </div>
      </Show>
    </div>
  );
};

export const FileExplorer = () => {
  const { state } = useWorkspace();
  const [rootEntries, setRootEntries] = createSignal<DirEntry[]>([]);
  const [loadingRoot, setLoadingRoot] = createSignal(false);
  const [rootError, setRootError] = createSignal<string | null>(null);

  createEffect(async () => {
    const path = state.rootPath;
    if (!path) {
        setRootEntries([]);
        return;
    }

    setLoadingRoot(true);
    setRootError(null);

    try {
      const entries = await readDir(path);
      const sorted = entries.sort((a, b) => {
        if (a.isDirectory === b.isDirectory) return a.name.localeCompare(b.name);
        return a.isDirectory ? -1 : 1;
      });
      setRootEntries(sorted);
    } catch (err: any) {
      console.error("Failed to load root:", err);
      setRootError(err.message || "UNABLE TO READ ROOT DIRECTORY");
    } finally {
      setLoadingRoot(false);
    }
  });

  return (
    <div class="h-full w-full flex flex-col bg-matrix-bg/50">
      <Show when={loadingRoot()}>
        <div class="flex flex-col items-center justify-center p-8 space-y-2 opacity-70">
          <div class="w-6 h-6 border-2 border-matrix-primary border-t-transparent rounded-full animate-spin"></div>
          <span class="text-[10px] animate-pulse">READING FILE SYSTEM...</span>
        </div>
      </Show>

      <Show when={rootError()}>
        <div class="p-4 flex flex-col items-center text-center text-matrix-error border border-matrix-error/30 m-2 bg-matrix-error/5">
          <span class="font-bold text-xl mb-2">!</span>
          <span class="text-xs font-mono">{rootError()}</span>
          <span class="text-[9px] opacity-50 mt-2 break-all">{state.rootPath}</span>
        </div>
      </Show>

      <div class="flex-1 overflow-y-auto custom-scrollbar pb-10">
        <Show when={!state.rootPath}>
           <div class="flex flex-col items-center justify-center h-40 opacity-30 text-xs">
              <span>WAITING FOR LINK</span>
              <span class="text-[9px] mt-1">[ NO WORKSPACE ]</span>
           </div>
        </Show>

        <For each={rootEntries()}>
          {(entry) => (
            <ExplorerItem 
              entry={entry} 
              parentPath={state.rootPath} 
              depth={0} 
            />
          )}
        </For>
      </div>
    </div>
  );
};