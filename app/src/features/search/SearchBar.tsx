import { createSignal, Show, For, onMount, createEffect } from "solid-js";
import { invoke } from "@tauri-apps/api/core";
import { useRecipe } from "../../core/recipe.store";
import { useWorkspace } from "../../core/workspace.store";

export const SearchBar = () => {
  const { state: workspaceState } = useWorkspace();
  const { addStep } = useRecipe();

  // Local State
  const [query, setQuery] = createSignal("");
  const [results, setResults] = createSignal<any[]>([]);
  const [isOpen, setIsOpen] = createSignal(false);
  
  // Config State
  const [radius, setRadius] = createSignal(5);
  const [limitRadius, setLimitRadius] = createSignal(true); 
  const [includeTests, setIncludeTests] = createSignal(false);

  let inputRef: HTMLInputElement | undefined;

  onMount(() => {
    setTimeout(() => inputRef?.focus(), 100);
  });

  createEffect(() => {
    if (workspaceState.isSyncing) setIsOpen(false);
  });

  const handleInput = async (e: any) => {
    const val = e.target.value;
    setQuery(val);
    if (workspaceState.isSyncing) return;

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

  const selectResult = (item: any) => {
    addStep({ 
        type: "symbol", 
        value: item.name,
        // @ts-ignore
        params: {
            max_depth: limitRadius() ? radius() : 100, 
            exclude_tests: !includeTests()
        }
    });
    setQuery("");
    setIsOpen(false);
    inputRef?.focus();
  };

  const isLocked = () => workspaceState.isSyncing;

  return (
    <div class="w-full h-full relative group bg-matrix-panel flex items-center select-none overflow-visible">
      
      {/* LEFT: SEARCH INPUT */}
      <div class="flex-1 h-full relative border-r border-matrix-border/50">
        <Show when={query() === ""}>
            <div class="absolute inset-0 flex items-center px-4 pointer-events-none">
            <Show 
                when={!isLocked()}
                fallback={
                    <div class="flex items-center text-matrix-primary/50 animate-pulse font-mono">
                        <span class="mr-2">::</span>
                        <span class="text-base tracking-widest">SYSTEM_INDEXING...</span>
                    </div>
                }
            >
                <span class="text-matrix-primary font-bold mr-3">{">"}</span>
                <span class="text-matrix-primary/30 text-base tracking-[0.15em] font-mono">
                  QUERY_DATABASE
                </span>
                <span class="ml-2 w-2 h-4 bg-matrix-primary/50 animate-[pulse_1s_infinite]"></span>
            </Show>
            </div>
        </Show>
        
        <input
            ref={inputRef}
            type="text"
            value={query()}
            onInput={handleInput}
            onKeyDown={(e) => { if (e.key === "Escape") { setIsOpen(false); inputRef?.blur(); } }}
            disabled={isLocked()}
            class={`
                w-full h-full bg-transparent border-none px-4 focus:outline-none 
                font-mono text-sm relative z-10 font-bold transition-colors
                ${isLocked() ? "cursor-wait text-matrix-primary/30" : "text-matrix-highlight focus:bg-matrix-primary/5"}
            `}
            spellcheck={false}
            autocomplete="off"
        />
      </div>

      {/* RIGHT: CONTROL DECK */}
      <div class="flex items-center h-full bg-black/20 px-3 gap-3 shrink-0 text-sm">
        
        {/* RADIUS */}
        <div class="flex items-center gap-2 group/radius">
            <button 
                onClick={() => setLimitRadius(!limitRadius())}
                disabled={isLocked()}
                class={`
                    text-lg uppercase font-bold tracking-wider transition-colors cursor-pointer hover:text-matrix-highlight
                    ${limitRadius() ? "text-matrix-primary" : "text-matrix-primary/40 line-through decoration-matrix-primary/40"}
                `}
            >
                RADIUS
            </button>

            <Show 
                when={limitRadius()}
                fallback={
                    <div class="flex items-center justify-center w-20 h-4 bg-matrix-primary/5 rounded border border-matrix-primary/20">
                         <span class="text-base font-bold text-matrix-primary">∞</span>
                    </div>
                }
            >
                <div class="flex items-center gap-2">
                    <input 
                        type="range" 
                        min="1" 
                        max="10" 
                        value={radius()}
                        disabled={isLocked()}
                        onInput={(e) => setRadius(parseInt(e.currentTarget.value))}
                        class="
                        w-14 h-1 appearance-none bg-matrix-border/50 cursor-pointer rounded-full
                        [&::-webkit-slider-thumb]:appearance-none 
                        [&::-webkit-slider-thumb]:w-2.5 
                        [&::-webkit-slider-thumb]:h-2.5 
                        [&::-webkit-slider-thumb]:rounded-full
                        [&::-webkit-slider-thumb]:bg-matrix-primary 
                        [&::-webkit-slider-thumb]:shadow-[0_0_5px_#00ff41]
                        [&::-webkit-slider-thumb]:hover:scale-125
                        [&::-webkit-slider-thumb]:transition-transform
                        "
                    />
                    <span class="text-sm font-mono font-bold text-matrix-primary w-4 text-right">
                        {radius()}
                    </span>
                </div>
            </Show>
        </div>

        {/* TESTS */}
        <button
            onClick={() => setIncludeTests(!includeTests())}
            disabled={isLocked()}
            class={`
                h-5 px-2 text-lg font-bold uppercase tracking-wider border transition-all flex items-center rounded-sm
                ${includeTests() 
                    ? "bg-matrix-primary text-matrix-bg border-matrix-primary" 
                    : "bg-transparent text-matrix-primary/50 border-matrix-border hover:border-matrix-primary/50 hover:text-matrix-primary"}
            `}
        >
            TESTS
        </button>

      </div>

      {/* DROPDOWN */}
      <div class="absolute bottom-0 left-0 w-full h-[1px] bg-matrix-border group-focus-within:bg-matrix-primary group-focus-within:shadow-[0_0_10px_rgba(0,255,65,0.5)] transition-all z-20"></div>

      <Show when={isOpen() && !workspaceState.isSyncing}>
        <div class="absolute top-full left-0 w-full mt-[1px] bg-matrix-bg/95 backdrop-blur-md border border-matrix-primary shadow-glow max-h-96 overflow-y-auto z-50">
          <For each={results()}>
            {(item) => (
              <div
                onClick={() => selectResult(item)}
                class="flex justify-between items-center p-2 border-b border-matrix-border/50 cursor-pointer hover:bg-matrix-primary hover:text-matrix-bg transition-colors group/item"
              >
                <div class="flex items-center gap-3 overflow-hidden">
                  <span class="font-mono font-bold text-base shrink-0">{item.name}</span>
                  <span class="text-sm uppercase tracking-wider border border-matrix-border px-1.5 py-px rounded-sm opacity-60 group-hover/item:border-matrix-bg group-hover/item:opacity-100">
                    {item.kind}
                  </span>
                </div>
                <div class="text-sm font-mono opacity-50 truncate max-w-[50%] text-right group-hover/item:opacity-80">
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