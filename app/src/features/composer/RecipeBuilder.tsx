import { For, createSignal, Show, Switch, Match } from "solid-js";
import { Portal } from "solid-js/web";
import { useRecipe } from "../../core/recipe.store";
import { UiRecipeStep } from "../../core/types";
import { invoke } from "@tauri-apps/api/core";

// --- Types ---

// TypeScript doesn't know Tauri adds a 'path' property to File objects.
// We extend the interface to avoid TS errors.
interface TauriFile extends File {
  path?: string;
  name: string;
}

interface ResolvedPathDTO {
  original: string;
  relative_path: string | null;
  root_id: string | null;
  is_indexed: boolean;
}

// --- Components ---

const StepItem = (props: {
  step: UiRecipeStep;
  index: number;
  onMove: (from: number, to: number) => void;
}) => {
  const { removeStep, updateStepParams, toggleStepType } = useRecipe();
  const [isHovering, setIsHovering] = createSignal(false);

  // DnD Handlers (Reordering)
  const handleDragStart = (e: DragEvent) => {
    e.dataTransfer?.setData("text/plain", props.index.toString());
    e.dataTransfer?.setData("type", "reorder_step");
    e.dataTransfer!.effectAllowed = "move";
  };

  const handleDragOver = (e: DragEvent) => {
    e.preventDefault();
    e.dataTransfer!.dropEffect = "move";
  };

  const handleDrop = (e: DragEvent) => {
    e.preventDefault();
    e.stopPropagation();
    const fromIndexStr = e.dataTransfer?.getData("text/plain");
    const type = e.dataTransfer?.getData("type");
    if (type === "reorder_step" && fromIndexStr !== undefined) {
      const fromIndex = parseInt(fromIndexStr);
      if (!isNaN(fromIndex)) props.onMove(fromIndex, props.index);
    }
  };

  const isInfinite = () => {
    if (props.step.op.type === "BlastRadius") {
      return (props.step.op.params.max_depth || 0) > 20;
    }
    return false;
  };

  const depth = () => {
    if (props.step.op.type === "BlastRadius") {
      return isInfinite() ? 5 : props.step.op.params.max_depth || 5;
    }
    return 5;
  };

  const label = () => {
    if (props.step.op.type === "BlastRadius") {
      return props.step.op.params.symbol;
    }
    return props.step.op.params.pattern;
  };

  return (
    <div
      draggable={true}
      onDragStart={handleDragStart}
      onDragOver={handleDragOver}
      onDrop={handleDrop}
      onMouseEnter={() => setIsHovering(true)}
      onMouseLeave={() => setIsHovering(false)}
      class="
            border border-matrix-border bg-matrix-bg p-2 text-sm 
            transition-colors hover:border-matrix-primary/50 cursor-grab active:cursor-grabbing
            relative group/item
        "
    >
      <div class="flex items-center justify-between gap-2">
        <div class="flex items-center gap-3 overflow-hidden min-w-0 flex-1">
          <Switch>
            <Match when={props.step.op.type === "AddFiles"}>
              <button
                onClick={() => toggleStepType(props.step.id)}
                class="w-5 h-5 flex items-center justify-center text-matrix-bg bg-matrix-primary font-bold hover:bg-matrix-highlight transition-colors"
                title="Include Mode (Click to Exclude)"
              >
                +
              </button>
            </Match>
            <Match when={props.step.op.type === "RemoveFiles"}>
              <button
                onClick={() => toggleStepType(props.step.id)}
                class="w-5 h-5 flex items-center justify-center text-matrix-bg bg-matrix-primary font-bold hover:bg-matrix-highlight transition-colors"
                title="Exclude Mode (Click to Include)"
              >
                -
              </button>
            </Match>
            <Match when={props.step.op.type === "BlastRadius"}>
              <div class="w-5 h-5 flex items-center justify-center text-matrix-primary border border-matrix-primary">
                <span class="text-sm font-bold">?</span>
              </div>
            </Match>
          </Switch>

          <span
            class="truncate font-mono font-bold text-matrix-highlight/90 text-base flex-1"
            title={label()}
          >
            {label()}
          </span>
        </div>

        <button
          onClick={() => removeStep(props.step.id)}
          class={`text-matrix-primary hover:text-matrix-error font-bold px-1 shrink-0 transition-opacity ${isHovering() ? "opacity-100" : "opacity-0"}`}
        >
          [x]
        </button>
      </div>

      <Show when={props.step.op.type === "BlastRadius"}>
        <div class="mt-2 pt-2 border-t border-matrix-border/30 flex items-center gap-3 animate-[fadeIn_0.2s_ease-out]">
          <div class="flex items-center gap-2">
            <button
              onClick={() =>
                updateStepParams(props.step.id, {
                  max_depth: isInfinite() ? 5 : 100,
                })
              }
              class={`text-lg uppercase font-bold tracking-wider transition-colors ${!isInfinite() ? "text-matrix-primary" : "text-matrix-primary/40 line-through decoration-matrix-primary/40"}`}
            >
              RAD
            </button>
            <Show
              when={!isInfinite()}
              fallback={
                <div class="flex items-center justify-center w-16 h-4 bg-matrix-primary/5 border border-matrix-primary/20">
                  <span class="text-base font-bold text-matrix-primary">∞</span>
                </div>
              }
            >
              <div class="flex items-center gap-2">
                <input
                  type="range"
                  min="1"
                  max="10"
                  value={depth()}
                  onInput={(e) =>
                    updateStepParams(props.step.id, {
                      max_depth: parseInt(e.currentTarget.value),
                    })
                  }
                  class="w-16 h-1 appearance-none bg-matrix-border/50 cursor-pointer [&::-webkit-slider-thumb]:appearance-none [&::-webkit-slider-thumb]:w-2.5 [&::-webkit-slider-thumb]:h-2.5 [&::-webkit-slider-thumb]:bg-matrix-primary"
                />
                <span class="text-sm font-mono font-bold text-matrix-primary w-3 text-right">
                  {depth()}
                </span>
              </div>
            </Show>
          </div>
          <div class="w-px h-3 bg-matrix-border/50"></div>
          <button
            onClick={() => {
              if (props.step.op.type === "BlastRadius") {
                updateStepParams(props.step.id, {
                  exclude_tests: !props.step.op.params.exclude_tests,
                });
              }
            }}
            class={`h-5 px-2 text-lg font-bold uppercase tracking-wider border transition-all flex items-center ${props.step.op.type === "BlastRadius" && !props.step.op.params.exclude_tests ? "bg-matrix-primary text-matrix-bg border-matrix-primary" : "bg-transparent text-matrix-primary/50 border-matrix-border hover:border-matrix-primary/50 hover:text-matrix-primary"}`}
          >
            TESTS
          </button>
        </div>
      </Show>
    </div>
  );
};

// --- Main Component ---

export const RecipeBuilder = () => {
  const { recipeState, addStep, resetRecipe, moveStep } = useRecipe();
  const [isDraggingFile, setIsDraggingFile] = createSignal(false);

  // Modal State
  const [showPatternModal, setShowPatternModal] = createSignal(false);
  const [patternInput, setPatternInput] = createSignal("");
  let patternInputRef: HTMLInputElement | undefined;

  // --- Handlers ---

  const handleDragEnter = (e: DragEvent) => {
    e.preventDefault();
    e.stopPropagation();
    setIsDraggingFile(true);
  };

  const handleDragOver = (e: DragEvent) => {
    e.preventDefault();
    e.stopPropagation();
    e.dataTransfer!.dropEffect = "copy";
    setIsDraggingFile(true);
  };

  const handleDragLeave = (e: DragEvent) => {
    e.preventDefault();
    e.stopPropagation();
    if (e.currentTarget === e.target) {
      setIsDraggingFile(false);
    }
  };

  const handleOuterDrop = async (e: DragEvent) => {
    e.preventDefault();
    e.stopPropagation();
    setIsDraggingFile(false);

    if (!e.dataTransfer) return;

    const type = e.dataTransfer.getData("type");
    if (type === "reorder_step") return;

    // 1. Collect Paths to Resolve
    const pathsToResolve: string[] = [];

    // A. Check for Internal Drag (from the App's FileExplorer)
    const rawData = e.dataTransfer.getData("application/json");
    if (rawData) {
      try {
        const data = JSON.parse(rawData);
        if (data.type === "add_file" && data.value) {
          pathsToResolve.push(data.value);
        }
      } catch (err) {
        /* ignore */
      }
    }
    // B. Check for External OS Drag
    else if (e.dataTransfer.files && e.dataTransfer.files.length > 0) {
      for (let i = 0; i < e.dataTransfer.files.length; i++) {
        const file = e.dataTransfer.files[i] as TauriFile;
        if (file.path) {
          pathsToResolve.push(file.path);
        } else if (file.name) {
          console.warn(
            "Dropped file missing absolute path (Check Tauri Perms):",
            file.name,
          );
        }
      }
    }

    if (pathsToResolve.length === 0) return;

    // 2. Resolve Absolute -> Relative via Backend
    try {
      const resolved = await invoke<ResolvedPathDTO[]>("resolve_paths", {
        paths: pathsToResolve,
      });

      // 3. Add Valid Steps
      for (const res of resolved) {
        if (res.relative_path) {
          let finalPattern = res.relative_path;

          // HEURISTIC: If the path has no extension, it's likely a Folder.
          // A pattern like "src/components" does NOT match files inside it.
          // We automatically append "/**" to capture the folder contents.
          const hasExtension = finalPattern.split("/").pop()?.includes(".");
          if (!hasExtension) {
            finalPattern = `${finalPattern}/**`;
          }

          addStep({ kind: "file", path: finalPattern, mode: "include" });
        } else {
          console.warn(
            "Dropped item is outside workspace roots:",
            res.original,
          );
        }
      }
    } catch (err) {
      console.error("Failed to resolve paths", err);
    }
  };

  const handleAddPattern = (mode: "include" | "exclude") => {
    const val = patternInput().trim();
    if (!val) return;
    addStep({ kind: "file", path: val, mode: mode });
    setPatternInput("");
    setShowPatternModal(false);
  };

  return (
    <div class="flex flex-col h-full bg-matrix-bg/50 relative">
      {/* Header */}
      <div class="sticky top-0 bg-matrix-bg/95 backdrop-blur z-10 px-4 h-10 border-b border-matrix-border/30 flex justify-between items-center shrink-0">
        <div class="flex items-center gap-2">
          <span class="text-base uppercase tracking-widest opacity-40 font-bold">
            Active Session:
          </span>
          <span
            class={`text-base font-bold font-mono ${recipeState.activeRecipeName ? "text-matrix-highlight" : "text-matrix-primary/50 italic"}`}
          >
            {recipeState.activeRecipeName || "Untitled"}
          </span>
          <Show when={recipeState.isDirty}>
            <span class="text-matrix-highlight font-bold animate-pulse">*</span>
          </Show>
        </div>

        <div class="flex items-center gap-2">
          <button
            onClick={() => {
              setPatternInput("");
              setShowPatternModal(true);
              setTimeout(() => patternInputRef?.focus(), 50);
            }}
            class="flex items-center gap-1 text-lg font-bold uppercase tracking-wider text-matrix-primary hover:bg-matrix-primary hover:text-matrix-bg px-2 py-1 transition-all border border-transparent hover:border-matrix-primary"
          >
            [ PATTERN ]
          </button>

          <Show when={recipeState.steps.length > 0}>
            <button
              onClick={resetRecipe}
              class="flex items-center gap-1 text-lg font-bold uppercase tracking-wider text-matrix-primary hover:bg-matrix-primary hover:text-matrix-bg px-2 py-1 transition-all border border-transparent hover:border-matrix-primary"
            >
              [ CLEAR ]
            </button>
          </Show>
        </div>
      </div>

      {/* Main List */}
      <div
        class={`
            flex-1 p-2 overflow-y-auto custom-scrollbar space-y-2 transition-colors
            ${isDraggingFile() ? "bg-matrix-primary/10 border-2 border-dashed border-matrix-primary" : ""}
        `}
        onDragEnter={handleDragEnter}
        onDragOver={handleDragOver}
        onDragLeave={handleDragLeave}
        onDrop={handleOuterDrop}
      >
        <For each={recipeState.steps}>
          {(step, index) => (
            <StepItem step={step} index={index()} onMove={moveStep} />
          )}
        </For>

        {recipeState.steps.length === 0 && !isDraggingFile() && (
          <div class="flex flex-col items-center justify-center h-full opacity-30 select-none pointer-events-none">
            <div class="text-base tracking-widest font-bold">
              [ WORKBENCH EMPTY ]
            </div>
            <div class="text-sm mt-1 font-mono">
              DROP FILES OR SEARCH SYMBOLS
            </div>
          </div>
        )}

        <Show when={isDraggingFile()}>
          <div class="absolute inset-0 flex items-center justify-center pointer-events-none z-20">
            <div class="bg-matrix-bg border border-matrix-primary p-4 shadow-glow">
              <span class="text-xl font-bold tracking-widest animate-pulse">
                DROP TO ADD
              </span>
            </div>
          </div>
        </Show>
      </div>

      {/* PATTERN MODAL */}
      <Show when={showPatternModal()}>
        <Portal>
          <div class="fixed inset-0 z-[100] bg-matrix-bg/90 backdrop-blur-sm flex items-center justify-center p-4 animate-[fadeIn_0.1s_ease-out]">
            <div class="w-full max-w-lg border border-matrix-primary p-1 bg-matrix-panel shadow-glow">
              {/* Modal Header */}
              <div class="flex items-center justify-between bg-matrix-primary/20 p-2 mb-2">
                <div class="text-base font-bold text-matrix-highlight uppercase tracking-widest">
                  Add Glob Pattern
                </div>
                <button
                  onClick={() => setShowPatternModal(false)}
                  class="text-matrix-primary hover:text-matrix-error font-bold px-2"
                >
                  [x]
                </button>
              </div>

              {/* Input Area */}
              <div class="p-2 space-y-4">
                <input
                  ref={patternInputRef}
                  type="text"
                  value={patternInput()}
                  onInput={(e) => setPatternInput(e.currentTarget.value)}
                  onKeyDown={(e) => {
                    if (e.key === "Enter") handleAddPattern("include");
                    if (e.key === "Escape") setShowPatternModal(false);
                  }}
                  class="w-full bg-matrix-bg border border-matrix-border text-matrix-highlight p-3 text-lg font-mono outline-none focus:border-matrix-primary focus:shadow-[0_0_10px_rgba(0,255,65,0.2)] placeholder:text-matrix-primary/20"
                  placeholder="e.g. src/**/*.rs or src/components/**"
                />

                <div class="text-xs font-mono text-matrix-primary/80 border border-matrix-border/50 bg-black/40">
                  <div class="px-3 py-2 font-bold text-matrix-highlight uppercase tracking-wider border-b border-matrix-border/30 bg-matrix-primary/5 flex justify-between items-center">
                    <span>Glob Syntax</span>
                    <span class="opacity-50 text-[10px]">SUPPORTS GLOBSET</span>
                  </div>

                  <div class="p-3 grid grid-cols-[auto_1fr_auto] gap-x-4 gap-y-2 items-center">
                    {/* * */}
                    <div class="text-matrix-highlight font-bold bg-matrix-primary/10 px-1.5 py-0.5 rounded text-center">
                      *
                    </div>
                    <div class="opacity-70">Zero or more chars</div>
                    <div class="opacity-50">*.json</div>

                    {/* ? */}
                    <div class="text-matrix-highlight font-bold bg-matrix-primary/10 px-1.5 py-0.5 rounded text-center">
                      ?
                    </div>
                    <div class="opacity-70">Exactly one char</div>
                    <div class="opacity-50">test-?.js</div>

                    {/* ** */}
                    <div class="text-matrix-highlight font-bold bg-matrix-primary/10 px-1.5 py-0.5 rounded text-center">
                      **
                    </div>
                    <div class="opacity-70">Any depth (recursive)</div>
                    <div class="opacity-50">src/**/*.rs</div>

                    {/* {} */}
                    <div class="text-matrix-highlight font-bold bg-matrix-primary/10 px-1.5 py-0.5 rounded text-center">
                      {`{}`}
                    </div>
                    <div class="opacity-70">Alternatives / Group</div>
                    <div class="opacity-50">*.{`{ts,tsx}`}</div>

                    {/* [] */}
                    <div class="text-matrix-highlight font-bold bg-matrix-primary/10 px-1.5 py-0.5 rounded text-center">
                      []
                    </div>
                    <div class="opacity-70">Character range</div>
                    <div class="opacity-50">ver-[0-9].txt</div>

                    {/* [!...] */}
                    <div class="text-matrix-highlight font-bold bg-matrix-primary/10 px-1.5 py-0.5 rounded text-center">
                      [!...]
                    </div>
                    <div class="opacity-70">Negated char range</div>
                    <div class="opacity-50">file-[!a-z].log</div>
                  </div>
                </div>

                {/* Actions */}
                <div class="flex gap-3 pt-2">
                  <button
                    onClick={() => handleAddPattern("exclude")}
                    class="flex-1 py-3 text-lg font-bold border border-matrix-border text-matrix-error hover:bg-matrix-error hover:text-matrix-bg transition-colors uppercase tracking-widest"
                  >
                    [ - EXCLUDE ]
                  </button>
                  <button
                    onClick={() => handleAddPattern("include")}
                    class="flex-1 py-3 text-lg font-bold bg-matrix-primary text-matrix-bg hover:bg-matrix-highlight transition-colors uppercase tracking-widest shadow-glow"
                  >
                    [ + INCLUDE ]
                  </button>
                </div>
              </div>
            </div>
          </div>
        </Portal>
      </Show>
    </div>
  );
};