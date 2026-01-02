import { createSignal, Show, For, onMount } from "solid-js";
import { invoke } from "@tauri-apps/api/core";
import { useWorkspace } from "../../core/workspace.store";

export const SearchBar = () => {
  const { addToRecipe } = useWorkspace();
  const [query, setQuery] = createSignal("");
  const [results, setResults] = createSignal<any[]>([]);
  const [isOpen, setIsOpen] = createSignal(false);
  let inputRef: HTMLInputElement | undefined;

  // Auto-focus on mount
  onMount(() => {
    setTimeout(() => {
      inputRef?.focus();
    }, 100);
  });

  const handleInput = async (e: any) => {
    const val = e.target.value;
    setQuery(val);
    if (val.length > 2) {
      try {
        const res = await invoke<any[]>("search_symbols", { query: val });
        setResults(res);
        setIsOpen(true);
      } catch (err) {
        console.error(err);
      }
    } else {
      setIsOpen(false);
    }
  };

  const handleKeyDown = (e: KeyboardEvent) => {
    if (e.key === "Escape") {
      setIsOpen(false);
      inputRef?.blur();
    }
  };

  const selectResult = (item: any) => {
    addToRecipe("add_symbol", item.name);
    setQuery("");
    setIsOpen(false);
    inputRef?.focus();
  };

  return (
    <div class="w-full h-full relative group bg-matrix-panel">
      
      {/* MATRIX TYPING EFFECT OVERLAY */}
      <Show when={query() === ""}>
        <div class="absolute inset-0 flex items-center px-4 pointer-events-none select-none">
          <span class="text-matrix-primary font-bold mr-2">{">>>"}</span>
          <span class="text-matrix-primary/50 text-sm tracking-widest">
            AWAITING INPUT
          </span>
          <span class="ml-2 w-2 h-4 bg-matrix-primary animate-[pulse_1s_infinite]"></span>
        </div>
      </Show>

      {/* Actual Input */}
      <input
        ref={inputRef}
        type="text"
        value={query()}
        onInput={handleInput}
        onKeyDown={handleKeyDown}
        class="w-full h-full bg-transparent border-none text-matrix-highlight px-4 focus:outline-none focus:bg-matrix-primary/5 font-mono text-sm relative z-10 font-bold"
        spellcheck={false}
        // FIXED: Lowercase 'autocomplete' for SolidJS
        autocomplete="off" 
      />
      
      {/* Bottom border glow */}
      <div class="absolute bottom-0 left-0 w-full h-[1px] bg-matrix-border group-focus-within:bg-matrix-primary group-focus-within:shadow-[0_0_10px_rgba(0,255,65,0.5)] transition-all z-20"></div>

      {/* Autocomplete Overlay */}
      <Show when={isOpen()}>
        <div class="absolute top-full left-0 w-full mt-[1px] bg-matrix-bg border border-matrix-primary shadow-glow max-h-96 overflow-y-auto z-50">
          <For each={results()}>
            {(item) => (
              <div 
                onClick={() => selectResult(item)}
                class="p-2 hover:bg-matrix-primary hover:text-matrix-bg cursor-pointer border-b border-matrix-border flex justify-between items-center transition-colors text-xs group/item"
              >
                <div class="flex items-center space-x-2 overflow-hidden">
                  <span class="font-bold shrink-0">{item.name}</span>
                  <span class="text-[9px] uppercase border border-matrix-border px-1 rounded opacity-60 group-hover/item:border-matrix-bg group-hover/item:opacity-100">
                    {item.kind}
                  </span>
                </div>
                <div class="opacity-50 truncate max-w-[50%] text-right font-light text-[10px]">
                    {item.path}
                </div>
              </div>
            )}
          </For>
        </div>
      </Show>
    </div>
  );
};