import { createSignal, For, Show, createEffect } from "solid-js";
import { readDir, DirEntry } from "@tauri-apps/plugin-fs";
import { useWorkspace } from "../../core/workspace.store";

// Recursive Component for Tree Items
const FileTreeItem = (props: { entry: DirEntry; parentPath: string }) => {
  const [isOpen, setIsOpen] = createSignal(false);
  const [children, setChildren] = createSignal<DirEntry[]>([]);
  const [loading, setLoading] = createSignal(false);
  
  // We construct the full path for this item
  // Note: Simple string concatenation works for display, but for FS operations
  // we usually need to be careful about separators. 
  // For the MVP, we assume parentPath doesn't end in separator.
  const fullPath = `${props.parentPath}/${props.entry.name}`;

  const toggleDir = async () => {
    if (!props.entry.isDirectory) return;

    if (!isOpen()) {
      setLoading(true);
      try {
        // Fetch contents only when opening
        const entries = await readDir(fullPath);
        
        // Sort: Folders first, then files
        const sorted = entries.sort((a, b) => {
          if (a.isDirectory === b.isDirectory) return a.name.localeCompare(b.name);
          return a.isDirectory ? -1 : 1;
        });
        
        setChildren(sorted);
      } catch (err) {
        console.error("Failed to read dir:", err);
      } finally {
        setLoading(false);
      }
    }
    setIsOpen(!isOpen());
  };

  const handleDragStart = (e: DragEvent) => {
    if (e.dataTransfer) {
      // We pass the type and the value separated by a delimiter, or just JSON
      e.dataTransfer.setData("application/json", JSON.stringify({
        type: "add_file",
        value: fullPath
      }));
      e.dataTransfer.effectAllowed = "copy";
    }
  };

  return (
    <div class="select-none">
      <div 
        class={`
          flex items-center py-1 px-2 cursor-pointer 
          hover:bg-matrix-primary/20 hover:text-matrix-highlight
          transition-colors text-sm border-l border-transparent
          ${isOpen() ? 'border-matrix-primary' : ''}
        `}
        style={{ "padding-left": "8px" }}
        onClick={toggleDir}
        draggable={!props.entry.isDirectory}
        onDragStart={handleDragStart}
      >
        <span class="mr-2 font-bold opacity-70 w-4 inline-block text-center">
          {props.entry.isDirectory ? (isOpen() ? "[-]" : "[+]") : ""}
        </span>
        
        <span class={`${props.entry.isDirectory ? "text-matrix-primary font-bold" : "text-matrix-text"}`}>
          {props.entry.name}
        </span>
        
        <Show when={loading()}>
          <span class="ml-2 text-xs opacity-50 animate-pulse">...</span>
        </Show>
      </div>

      <Show when={isOpen() && props.entry.isDirectory}>
        <div class="ml-4 border-l border-matrix-border/50">
          <For each={children()}>
            {(child) => <FileTreeItem entry={child} parentPath={fullPath} />}
          </For>
          <Show when={children().length === 0 && !loading()}>
            <div class="pl-6 py-1 text-xs opacity-30 italic">
              (empty)
            </div>
          </Show>
        </div>
      </Show>
    </div>
  );
};

// Root Component
export const FileExplorer = () => {
  const { state } = useWorkspace();
  const [rootEntries, setRootEntries] = createSignal<DirEntry[]>([]);
  const [error, setError] = createSignal<string | null>(null);

  // Re-fetch when rootPath changes
  createEffect(async () => {
    const path = state.rootPath;
    if (!path) return;

    try {
      const entries = await readDir(path);
      const sorted = entries.sort((a, b) => {
        if (a.isDirectory === b.isDirectory) return a.name.localeCompare(b.name);
        return a.isDirectory ? -1 : 1;
      });
      setRootEntries(sorted);
      setError(null);
    } catch (err) {
      console.error(err);
      setError("Unable to read directory. Check permissions.");
    }
  });

  return (
    <div class="h-full overflow-y-auto scrollbar-thin p-2">
      <Show when={!state.rootPath}>
        <div class="text-center mt-10 opacity-30 text-sm">
          NO WORKSPACE LOADED
        </div>
      </Show>

      <Show when={error()}>
        <div class="text-matrix-error text-xs p-2 border border-matrix-error">
          {error()}
        </div>
      </Show>

      <For each={rootEntries()}>
        {(entry) => (
          <FileTreeItem entry={entry} parentPath={state.rootPath} />
        )}
      </For>
    </div>
  );
};