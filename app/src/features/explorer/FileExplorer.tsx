import { createSignal, createEffect, For, Show, createMemo, onCleanup } from "solid-js";
import { Portal } from "solid-js/web"; 
import { readDir, DirEntry } from "@tauri-apps/plugin-fs";
import { open, save } from "@tauri-apps/plugin-dialog";
import { useWorkspace } from "../../core/workspace.store";
import { useRecipe } from "../../core/recipe.store";

// --- Helpers ---

const normalizePath = (path: string): string => {
  return path.replace(/\\/g, "/");
};

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

const FileNode = (props: { entry: DirEntry; parentPath: string; depth: number; filter: string }) => {
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

  // Filter logic: Only show children that match the filter (if set)
  const filteredChildren = createMemo(() => {
    const term = props.filter.toLowerCase();
    if (!term) return sortedChildren();
    return sortedChildren().filter(child => child.name.toLowerCase().includes(term));
  });

  const toggleNode = async (e: MouseEvent) => {
    e.stopPropagation();
    const { state } = useWorkspace(); 
    if (state.isSyncing) return; 
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
  
  // Auto-expand folder if it matches filter (primitive auto-expand)
  createEffect(() => {
    if (props.filter && !isOpen() && props.entry.isDirectory && props.entry.name.toLowerCase().includes(props.filter.toLowerCase())) {
       // We only trigger if the folder name *itself* matches, giving user a hint
       // Ideally we'd recursively check children, but that requires pre-loading the FS which is expensive.
       // For now, this is a "shallow" filter match.
       toggleNode(new MouseEvent('click'));
    }
  });

  const handleDragStart = (e: DragEvent) => {
    if (e.dataTransfer) {
      let value = normalizePath(fullPath);
      
      if (props.entry.isDirectory) {
        value = value.endsWith("/") ? `${value}**` : `${value}/**`;
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
        addStep({ type: "file", value: normalizePath(fullPath) });
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
<span class="text-sm font-bold text-matrix-primary opacity-70 group-hover:text-matrix-highlight">
                {isOpen() ? '[-]' : '[+]'}
              </span>
          </Show>
        </div>

        <span class={`
          truncate text-base tracking-tight select-none
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
          <For each={filteredChildren()}>
            {(child) => <FileNode entry={child} parentPath={fullPath} depth={props.depth + 1} filter={props.filter} />}
          </For>
          <Show when={!isLoading() && filteredChildren().length === 0 && !errorMsg()}>
            <div class="text-sm opacity-30 italic py-1" style={{ "padding-left": `${(props.depth + 1) * 12 + 24}px` }}>
              { sortedChildren().length > 0 ? '(No matches)' : '(empty)' }
            </div>
          </Show>
        </div>
      </Show>
    </div>
  );
};

const RootFolder = (props: { path: string; canRemove: boolean; onRemove: () => void; filter: string }) => {
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

    // Filter direct children of the root
    const filteredEntries = createMemo(() => {
        const term = props.filter.toLowerCase();
        if (!term) return entries();
        return entries().filter(e => e.name.toLowerCase().includes(term));
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
                    <span class="mr-2 text-sm font-bold text-matrix-primary shrink-0">
                        {isExpanded() ? "[-]" : "[+]"}
                    </span>
                    <div class="flex flex-col overflow-hidden min-w-0">
                        <span class={`text-base font-bold tracking-wide uppercase truncate ${error() ? 'text-matrix-error' : 'text-matrix-highlight'}`}>
                            {folderName()}
                        </span>
                        <span class="text-sm opacity-40 truncate font-mono">
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
                    <For each={filteredEntries()}>
                        {(entry) => <FileNode entry={entry} parentPath={props.path} depth={0} filter={props.filter} />}
                    </For>
                </div>
            </Show>
        </div>
    )
}

export const FileExplorer = () => {
  const { state, addRoot, removeRoot, saveWorkspace, loadWorkspace, refreshWorkspace, clearHistory } = useWorkspace();
  const [showRecent, setShowRecent] = createSignal(false);
  const [filter, setFilter] = createSignal("");
  
  let recentBtnRef: HTMLButtonElement | undefined;
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
        setCoords({ top: rect.bottom, left: rect.left, width: rect.width });
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
  const needsRefresh = () => state.isDirty && !state.isSyncing;

  return (
    <div class="h-full w-full flex flex-col bg-matrix-bg/50 border-r border-matrix-border">
      
      {/* --- TOP TOOLBAR: Workspace Actions --- */}
      <div class="border-b border-matrix-border/50 bg-matrix-panel flex flex-col shrink-0">
          
          {/* Row 1: Primary Actions (Load/Recent/Refresh) */}
          <div class="flex items-center text-sm divide-x divide-matrix-border/50">
              <button 
                onClick={handleLoadWorkspace}
                class="flex-1 py-2 hover:bg-matrix-primary/10 hover:text-matrix-highlight transition text-matrix-primary"
                title="Load existing workspace file (.cblast)"
              >
                [ LOAD ]
              </button>

              <div class="relative flex-1">
                <button 
                    id="recent-trigger"
                    ref={recentBtnRef}
                    onClick={toggleRecent}
                    class={`
                        w-full py-2 hover:bg-matrix-primary/10 hover:text-matrix-highlight 
                        transition text-matrix-primary flex items-center justify-center
                        ${showRecent() ? 'bg-matrix-primary/20 text-matrix-highlight shadow-[inset_0_0_10px_rgba(0,255,65,0.1)]' : ''}
                    `}
                    title="Recent Workspaces"
                >
                  {/* Clean Clock Icon for History */}
                  <svg class="w-4 h-4 fill-current opacity-80" viewBox="0 0 24 24">
                    <path d="M12 2C6.486 2 2 6.486 2 12s4.486 10 10 10 10-4.486 10-10S17.514 2 12 2zm0 18c-4.411 0-8-3.589-8-8s3.589-8 8-8 8 3.589 8 8-3.589 8-8 8z"></path>
                    <path d="M13 7h-2v6l5.25 3.15.75-1.23-4.5-2.67z"></path>
                  </svg>
                </button>

                <Show when={showRecent()}>
                    <Portal>
                        <div 
                            id="recent-menu" 
                            class="fixed bg-matrix-panel border border-matrix-primary shadow-glow z-[9999] flex flex-col animate-[fadeIn_0.1s_ease-out]"
                            style={{
                                top: `${coords().top}px`,
                                left: `${coords().left}px`,
                                "min-width": `${coords().width}px`,
                                width: "max-content",
                                "max-width": "400px" // Reasonable max width for paths
                            }}
                        >
                            <div class="max-h-80 overflow-y-auto custom-scrollbar">
                                <For each={state.recentWorkspaces} fallback={<div class="p-4 opacity-50 italic text-matrix-primary text-base text-center">No recent history</div>}>
                                    {(path) => (
                                        <button 
                                            onClick={() => { loadWorkspace(path); setShowRecent(false); }}
                                            class="w-full text-left p-3 text-matrix-primary hover:bg-matrix-primary hover:text-matrix-bg border-b border-matrix-border/30 last:border-0 transition-colors group"
                                            title={path}
                                        >
<div class="text-base font-bold truncate group-hover:text-black">{getFolderName(path)}</div>
                                             <div class="text-sm opacity-50 truncate font-mono group-hover:text-black">{path}</div>
                                        </button>
                                    )}
                                </For>
                            </div>
                            <div class="border-t border-matrix-primary p-1 bg-matrix-bg">
<button 
                                     onClick={() => { clearHistory(); setShowRecent(false); }}
                                     class="w-full text-center text-lg text-matrix-primary hover:bg-matrix-error hover:text-matrix-bg py-1.5 uppercase tracking-wider transition-colors"
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
                class={`
                    flex-none px-4 py-2 transition text-matrix-primary flex items-center justify-center
                    ${needsRefresh() 
                        ? 'bg-matrix-primary/10 animate-[pulse_2s_infinite] shadow-[inset_0_0_5px_rgba(0,255,65,0.5)] text-matrix-highlight font-bold' 
                        : 'hover:bg-matrix-primary/10 hover:text-matrix-highlight'}
                `}
                title={needsRefresh() ? "Files changed! Click to sync." : "Refresh current workspace"}
              >
                [ R ]
              </button>
          </div>

          {/* Row 2: Content Actions (Add Folder / Save) */}
          <div class="flex items-center text-sm border-t border-matrix-border/30">
             <button 
                onClick={handleAddRoot}
                class="flex-1 py-2 hover:bg-matrix-primary/10 text-matrix-primary hover:text-matrix-highlight transition flex items-center justify-center gap-1 group"
                title="Add another folder root to workspace"
             >
                <span class="font-bold group-hover:scale-125 transition-transform text-lg leading-none">+</span>
                <span class="text-base font-bold tracking-wide">ADD FOLDER</span>
             </button>

             <div class="w-px h-full bg-matrix-border/30"></div>

             <button 
                onClick={handleSaveWorkspace}
                class={`
                    flex-1 py-2 transition flex items-center justify-center gap-1
                    ${needsSave() 
                        ? 'text-matrix-primary bg-matrix-primary/5 hover:bg-matrix-primary hover:text-matrix-bg animate-[pulse_3s_infinite]' 
                        : 'text-matrix-primary/70 hover:opacity-100 hover:bg-matrix-primary/10'}
                `}
                title={needsSave() ? "Workspace changes unsaved! Click to create .cblast file" : "Save Workspace"}
             >
                <span class="text-base font-bold tracking-wide">[ SAVE ]</span>
             </button>
          </div>
      </div>

      {/* --- Filter & View Controls --- */}
      <Show when={state.config}>
        <div class="p-2 border-b border-matrix-border/50 bg-matrix-bg/80 shrink-0 flex gap-2">
          <div class="relative flex-1 group">
            <input 
                type="text"
                placeholder="Filter files..."
                value={filter()}
                onInput={(e) => setFilter(e.currentTarget.value)}
                class="w-full bg-matrix-panel border border-matrix-border/50 text-matrix-highlight px-2 pl-7 py-1 text-base outline-none focus:border-matrix-primary focus:shadow-glow font-mono transition-all"
            />
            <div class="absolute left-2 top-1/2 -translate-y-1/2 opacity-50 pointer-events-none group-focus-within:opacity-100 group-focus-within:text-matrix-primary">
                <svg width="10" height="10" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="3">
                    <circle cx="11" cy="11" r="8"></circle>
                    <line x1="21" y1="21" x2="16.65" y2="16.65"></line>
                </svg>
            </div>
            <Show when={filter()}>
                <button 
                    onClick={() => setFilter("")}
                    class="absolute right-2 top-1/2 -translate-y-1/2 text-matrix-primary hover:text-matrix-error"
                >
                    ✕
                </button>
            </Show>
          </div>
        </div>
      </Show>

      {/* --- Roots Scroll View --- */}
      <div class="flex-1 overflow-y-auto custom-scrollbar pb-10 bg-matrix-bg relative">
        <Show when={!state.config || state.config.roots.length === 0}>
<div class="flex flex-col items-center justify-center h-64 opacity-60 text-base font-mono text-matrix-primary p-6 text-center select-none">
               <div class="text-4xl mb-4 opacity-50 text-matrix-primary animate-pulse">∅</div>
               <div class="mb-4 tracking-widest font-bold">NO FOLDERS OPEN</div>
               <button 
                 onClick={handleAddRoot}
                 class="px-6 py-3 border border-matrix-primary/50 bg-matrix-panel hover:bg-matrix-primary hover:text-matrix-bg transition-all uppercase tracking-widest font-bold text-lg shadow-glow"
               >
                 Add Folder to Begin
               </button>
            </div>
        </Show>

        <Show when={state.config}>
            <div class="p-1">
                <For each={state.config?.roots}>
                    {(rootPath) => (
                        <RootFolder 
                            path={rootPath} 
                            canRemove={(state.config?.roots.length || 0) > 1}
                            onRemove={() => removeRoot(rootPath)}
                            filter={filter()}
                        />
                    )}
                </For>
            </div>
        </Show>
        
        {/* Shadow gradient at bottom to indicate scroll */}
        <div class="sticky bottom-0 left-0 w-full h-6 bg-gradient-to-t from-matrix-bg to-transparent pointer-events-none"></div>
      </div>
    </div>
  );
};