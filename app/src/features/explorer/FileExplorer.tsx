import { createSignal, createEffect, For, Show, createMemo, onCleanup } from "solid-js";
import { Portal } from "solid-js/web"; 
import { readDir, DirEntry } from "@tauri-apps/plugin-fs";
import { open, save } from "@tauri-apps/plugin-dialog";
import { useWorkspace } from "../../core/workspace.store";
import { useRecipe } from "../../core/recipe.store";

// --- Helpers ---

const joinPath = (parent: string, name: string): string => {
  const isWindows = parent.includes('\\');
  const sep = isWindows ? '\\' : '/';
  const cleanParent = parent.endsWith(sep) ? parent.slice(0, -1) : parent;
  return `${cleanParent}${sep}${name}`;
};

const getFolderName = (path: string): string => {
    const sep = path.includes("\\") ? "\\" : "/";
    return path.split(sep).pop() || path;
};

// --- Components ---

const FileNode = (props: { entry: DirEntry; parentPath: string; depth: number }) => {
  const [isOpen, setIsOpen] = createSignal(false);
  const [children, setChildren] = createSignal<DirEntry[]>([]);
  const [isLoading, setIsLoading] = createSignal(false);
  const [errorMsg, setErrorMsg] = createSignal<string | null>(null);

  const fullPath = joinPath(props.parentPath, props.entry.name);
  const { addStep } = useRecipe(); 
  
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
      setErrorMsg(typeof err === "string" ? err : err.message || "Access Denied");
    } finally {
      setIsLoading(false);
    }
  };

  const handleDragStart = (e: DragEvent) => {
    if (e.dataTransfer) {
      // Logic: If directory, append /** to act as a recursive glob
      let value = fullPath;
      if (props.entry.isDirectory) {
        // Standardize separators for globbing
        value = value.replace(/\\/g, "/") + "/**";
      }

      const payload = JSON.stringify({ type: "add_file", value });
      e.dataTransfer.setData("application/json", payload);
      e.dataTransfer.effectAllowed = "copy";
      if (e.currentTarget instanceof HTMLElement) e.currentTarget.style.opacity = "0.5";
    }
  };

  const handleDragEnd = (e: DragEvent) => {
    if (e.currentTarget instanceof HTMLElement) e.currentTarget.style.opacity = "1";
  }

  // Double click to add file to context
  const handleDblClick = (e: MouseEvent) => {
    e.stopPropagation();
    if (!props.entry.isDirectory) {
        addStep({ type: "file", value: fullPath });
    }
  }

  return (
    <div class="select-none font-mono">
      <div 
        onClick={toggleNode}
        onDblClick={handleDblClick}
        draggable={true} 
        onDragStart={handleDragStart}
        onDragEnd={handleDragEnd}
        class={`
          group flex items-center py-0.5 px-2 cursor-pointer 
          hover:bg-matrix-primary/10 border-l-2 border-transparent
          transition-all duration-100 relative
          ${isOpen() ? 'border-matrix-primary bg-matrix-primary/5' : ''}
          ${isLoading() ? 'cursor-wait' : ''}
        `}
        style={{ "padding-left": `${props.depth * 12 + 12}px` }}
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
             <span class="text-tiny font-bold text-matrix-primary opacity-70 group-hover:text-matrix-highlight">
               {isOpen() ? '[-]' : '[+]'}
             </span>
          </Show>
        </div>

        <span class={`
          truncate text-xs tracking-tight select-none
          ${props.entry.isDirectory 
              ? "text-matrix-primary font-bold" 
              : "text-matrix-primary/80 group-hover:text-matrix-highlight"} 
          ${errorMsg() ? "text-matrix-error line-through decoration-matrix-error opacity-70" : ""}
        `}>
          {props.entry.name}
        </span>
      </div>

      <Show when={isOpen()}>
        <div>
          <For each={sortedChildren()}>
            {(child) => <FileNode entry={child} parentPath={fullPath} depth={props.depth + 1} />}
          </For>
          <Show when={!isLoading() && sortedChildren().length === 0 && !errorMsg()}>
            <div class="text-tiny opacity-30 italic py-1" style={{ "padding-left": `${(props.depth + 1) * 12 + 24}px` }}>(empty)</div>
          </Show>
        </div>
      </Show>
    </div>
  );
};

const RootFolder = (props: { path: string, canRemove: boolean, onRemove: () => void }) => {
    const [entries, setEntries] = createSignal<DirEntry[]>([]);
    const [isExpanded, setIsExpanded] = createSignal(true);
    const [error, setError] = createSignal<string | null>(null);
    const folderName = createMemo(() => getFolderName(props.path));

    createEffect(async () => {
        try {
            const children = await readDir(props.path);
            setEntries(children.sort((a, b) => {
                if (a.isDirectory === b.isDirectory) return a.name.localeCompare(b.name);
                return a.isDirectory ? -1 : 1;
            }));
            setError(null);
        } catch (e: any) {
            setError("ACCESS DENIED");
        }
    });

    return (
        <div class="mb-4 group/root">
            {/* Root Header */}
            <div 
                class={`
                    flex items-center justify-between px-3 py-2 cursor-pointer transition-colors border-y border-matrix-border/50
                    ${error() ? 'bg-matrix-error/10 border-matrix-error/50' : 'bg-matrix-panel hover:bg-matrix-panel/80'}
                `}
                onClick={() => setIsExpanded(!isExpanded())}
                title={props.path}
            >
                <div class="flex items-center min-w-0">
                    <span class="mr-2 text-tiny font-bold text-matrix-primary shrink-0">
                        {isExpanded() ? "[-]" : "[+]"}
                    </span>
                    <div class="flex flex-col overflow-hidden min-w-0">
                        <span class={`text-xs font-bold tracking-wide uppercase truncate ${error() ? 'text-matrix-error' : 'text-matrix-highlight'}`}>
                            {folderName()}
                        </span>
                        <span class="text-micro opacity-40 truncate font-mono">
                            {props.path}
                        </span>
                    </div>
                </div>

                <Show when={props.canRemove}>
                    <button 
                        onClick={(e) => { e.stopPropagation(); props.onRemove(); }}
                        class="ml-2 text-matrix-primary opacity-0 group-hover/root:opacity-100 hover:text-matrix-error hover:scale-110 transition p-1"
                        title="Remove Folder from Workspace"
                    >
                        [x]
                    </button>
                </Show>
            </div>

            <Show when={isExpanded() && !error()}>
                <div class="mt-1">
                    <For each={entries()}>
                        {(entry) => <FileNode entry={entry} parentPath={props.path} depth={0} />}
                    </For>
                </div>
            </Show>
        </div>
    )
}

export const FileExplorer = () => {
  const { state, addRoot, removeRoot, saveWorkspace, loadWorkspace, refreshWorkspace, clearHistory } = useWorkspace();
  const [showRecent, setShowRecent] = createSignal(false);
  
  // Ref for the button to calculate position
  let recentBtnRef: HTMLButtonElement | undefined;
  // State for portal positioning
  const [coords, setCoords] = createSignal({ top: 0, left: 0, width: 0 });

  const handleAddRoot = async () => {
    const selected = await open({ directory: true });
    if (selected && typeof selected === 'string') {
        await addRoot(selected);
    }
  };

  const handleLoadWorkspace = async () => {
    const selected = await open({
      multiple: false,
      directory: false,
      filters: [{ name: 'Blast Radius Workspace', extensions: ['cblast'] }]
    });
    if (selected && typeof selected === 'string') {
        await loadWorkspace(selected);
    }
  };

  const handleSaveWorkspace = async () => {
    const defaultName = state.config?.name || 'my-workspace';
    const path = await save({
        filters: [{ name: 'Blast Radius Workspace', extensions: ['cblast'] }],
        defaultPath: `${defaultName}.cblast`
    });
    if (path) {
        await saveWorkspace(path);
    }
  };

  const toggleRecent = (e: MouseEvent) => {
    e.stopPropagation();
    if (!showRecent() && recentBtnRef) {
        const rect = recentBtnRef.getBoundingClientRect();
        // Capture coordinates relative to viewport
        setCoords({ 
            top: rect.bottom, 
            left: rect.left, 
            width: rect.width 
        });
        setShowRecent(true);
    } else {
        setShowRecent(false);
    }
  };

  const handleClickOutside = (e: MouseEvent) => {
    const target = e.target as HTMLElement;
    if (!target.closest('#recent-menu') && !target.closest('#recent-trigger')) {
        setShowRecent(false);
    }
  };

  const handleResize = () => setShowRecent(false);

  document.addEventListener('click', handleClickOutside);
  window.addEventListener('resize', handleResize);
  
  onCleanup(() => {
    document.removeEventListener('click', handleClickOutside);
    window.removeEventListener('resize', handleResize);
  });

  const needsSave = () => state.config?.mode === 'unsaved-workspace' || state.config?.mode === 'ad-hoc';

  return (
    <div class="h-full w-full flex flex-col bg-matrix-bg/50 border-r border-matrix-border">
      
      {/* --- TOP TOOLBAR: Workspace Actions --- */}
      <div class="border-b border-matrix-border/50 bg-matrix-panel flex flex-col">
          
          {/* Row 1: Primary Actions (Load/Recent/Refresh) */}
          <div class="flex items-center text-tiny divide-x divide-matrix-border/50">
              <button 
                onClick={handleLoadWorkspace}
                class="flex-1 py-2 hover:bg-matrix-primary/10 hover:text-matrix-highlight transition text-matrix-primary"
                title="Load existing workspace file (.cblast)"
              >
                [ LOAD ]
              </button>

              <div class="flex-1">
                <button 
                    id="recent-trigger"
                    ref={recentBtnRef}
                    onClick={toggleRecent}
                    class={`w-full py-2 hover:bg-matrix-primary/10 hover:text-matrix-highlight transition text-matrix-primary ${showRecent() ? 'bg-matrix-primary/20 text-matrix-highlight' : ''}`}
                    title="Recent workspaces"
                >
                    [ RECENT {showRecent() ? '^' : 'v'} ]
                </button>

                <Show when={showRecent()}>
                    <Portal>
                        <div 
                            id="recent-menu" 
                            class="fixed bg-matrix-panel border border-matrix-primary shadow-glow z-[9999] flex flex-col"
                            style={{
                                top: `${coords().top}px`,
                                left: `${coords().left}px`,
                                "min-width": `${coords().width}px`,
                                width: "max-content",
                                "max-width": "calc(100vw - 20px)"
                            }}
                        >
                            <div class="max-h-80 overflow-y-auto custom-scrollbar">
                                <For each={state.recentWorkspaces} fallback={<div class="p-2 opacity-50 italic text-matrix-primary whitespace-nowrap">No history</div>}>
                                    {(path) => (
                                        <button 
                                            onClick={() => { loadWorkspace(path); setShowRecent(false); }}
                                            class="w-full text-left p-2 text-matrix-primary hover:bg-matrix-primary hover:text-matrix-bg border-b border-matrix-border/30 last:border-0 whitespace-nowrap"
                                            title={path}
                                        >
                                            {path}
                                        </button>
                                    )}
                                </For>
                            </div>
                            <div class="border-t border-matrix-primary p-1 bg-matrix-bg/50">
                                <button 
                                    onClick={() => { clearHistory(); setShowRecent(false); }}
                                    class="w-full text-center text-tiny text-matrix-primary hover:text-matrix-error py-1 uppercase tracking-wider"
                                >
                                    [ Clear History ]
                                </button>
                            </div>
                        </div>
                    </Portal>
                </Show>
              </div>

              <button 
                onClick={refreshWorkspace}
                class="flex-none px-3 py-2 hover:bg-matrix-primary/10 hover:text-matrix-highlight transition text-matrix-primary"
                title="Refresh current workspace (Scan for changes)"
              >
                [ R ]
              </button>
          </div>

          {/* Row 2: Content Actions (Add Folder / Save) */}
          <div class="flex items-center text-tiny border-t border-matrix-border/30">
             <button 
                onClick={handleAddRoot}
                class="flex-1 py-2 hover:bg-matrix-primary/10 text-matrix-primary hover:text-matrix-highlight transition flex items-center justify-center gap-1 group"
             >
                <span class="font-bold group-hover:scale-125 transition-transform">+</span>
                <span>ADD FOLDER</span>
             </button>

             <button 
                onClick={handleSaveWorkspace}
                class={`
                    flex-1 py-2 transition flex items-center justify-center gap-1
                    ${needsSave() 
                        ? 'text-matrix-primary bg-matrix-primary/5 hover:bg-matrix-primary hover:text-matrix-bg animate-[pulse_3s_infinite]' 
                        : 'text-matrix-primary/70 hover:opacity-100 hover:bg-matrix-primary/10'}
                `}
                title={needsSave() ? "Workspace changes unsaved!" : "Save Workspace"}
             >
                <span>[ SAVE ]</span>
             </button>
          </div>
      </div>

      {/* --- Roots Scroll View --- */}
      <div class="flex-1 overflow-y-auto custom-scrollbar pb-10 bg-matrix-bg relative">
        <Show when={!state.config}>
           <div class="flex flex-col items-center justify-center h-40 opacity-30 text-xs font-mono text-matrix-primary">
              <span>NO DATA</span>
           </div>
        </Show>

        <Show when={state.config}>
            <For each={state.config?.roots}>
                {(rootPath) => (
                    <RootFolder 
                        path={rootPath} 
                        canRemove={(state.config?.roots.length || 0) > 1}
                        onRemove={() => removeRoot(rootPath)}
                    />
                )}
            </For>
        </Show>
        <div class="sticky bottom-0 left-0 w-full h-4 bg-gradient-to-t from-matrix-bg to-transparent pointer-events-none"></div>
      </div>
    </div>
  );
};