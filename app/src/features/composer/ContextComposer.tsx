import { For, Show, createSignal, createEffect, createMemo } from "solid-js";
import { invoke } from "@tauri-apps/api/core";
import { useWorkspace, ContextFile } from "../../core/workspace.store";
import { useRecipe } from "../../core/recipe.store";
import { EngineRecipe } from "../../core/types";

/**
 * Heuristic for token estimation based on line counts.
 * We don't have the full content anymore, so we guess.
 * Average line of code ~40 chars / 3 chars per token ~= 13 tokens.
 * Plus XML overhead.
 */
const estimateTokensFromMetadata = (file: ContextFile) => {
  if (!file.relevant_lines) return 0;
  
  // Calculate total lines
  const totalLines = file.relevant_lines.reduce((acc, range) => {
    return acc + (range.end - range.start + 1);
  }, 0);

  // Approx 15 tokens per line + 25 overhead for XML tags
  return (totalLines * 15) + 25;
};

const formatTokenCount = (num: number) => {
  return new Intl.NumberFormat('en-US', { 
    notation: "compact", 
    maximumFractionDigits: 1 
  }).format(num);
};

// --- Single File Component ---

const ContextFileItem = (props: { 
  file: ContextFile; 
  forceState?: boolean | null; 
  recipePayload: EngineRecipe;
}) => {
  const [isExpanded, setIsExpanded] = createSignal(false);
  const [content, setContent] = createSignal<string | null>(null);
  const [isLoading, setIsLoading] = createSignal(false);
  const [error, setError] = createSignal<string | null>(null);
  const [copied, setCopied] = createSignal(false);

  // Handle Force Expand/Collapse from parent
  createEffect(() => {
    if (props.forceState !== null && props.forceState !== undefined) {
      const shouldExpand = props.forceState;
      if (shouldExpand && !isExpanded()) {
        toggleNode(); // Trigger fetch if expanding
      } else if (!shouldExpand) {
        setIsExpanded(false);
      }
    }
  });

  const fetchContent = async () => {
    if (content() || isLoading()) return;

    setIsLoading(true);
    setError(null);
    
    try {
      const text = await invoke<string>("get_file_content", { 
        fileId: props.file.file_id, 
        recipeJson: props.recipePayload 
      });
      setContent(text);
    } catch (err) {
      console.error(err);
      setError("Failed to load content");
    } finally {
      setIsLoading(false);
    }
  };

  const toggleNode = () => {
    if (!isExpanded()) {
        fetchContent();
        setIsExpanded(true);
    } else {
        setIsExpanded(false);
    }
  };

  const handleCopyFile = async (e: MouseEvent) => {
    e.stopPropagation();
    
    // If we have content, copy it. If not, fetch it then copy.
    let textToCopy = content();
    if (!textToCopy) {
        setIsLoading(true);
        try {
            textToCopy = await invoke<string>("get_file_content", { 
                fileId: props.file.file_id, 
                recipeJson: props.recipePayload 
            });
            setContent(textToCopy);
        } catch (err) {
            console.error(err);
        } finally {
            setIsLoading(false);
        }
    }

    if (textToCopy) {
        navigator.clipboard.writeText(textToCopy);
        setCopied(true);
        setTimeout(() => setCopied(false), 2000);
    }
  };

  return (
    <div class="border border-matrix-border/50 bg-matrix-bg overflow-hidden animate-[fadeIn_0.3s_ease-out]">
      {/* Header */}
      <div 
        onClick={toggleNode}
        class={`
            flex items-center justify-between px-3 py-2 cursor-pointer transition-colors select-none border-b
            ${isExpanded() 
                ? 'bg-matrix-panel border-matrix-border/30' 
                : 'bg-matrix-panel/50 border-transparent hover:bg-matrix-panel hover:border-matrix-border/30'}
        `}
      >
        <div class="flex items-center gap-3 min-w-0">
          <div class={`
            w-4 h-4 flex items-center justify-center transition-transform duration-200 text-sm text-matrix-primary/70
            ${isExpanded() ? 'rotate-90' : 'rotate-0'}
          `}>
             ▶
          </div>
          <span class="text-sm font-bold uppercase text-matrix-bg bg-matrix-primary/40 px-1.5">
            {props.file.language}
          </span>
          <span class="font-bold text-base font-mono text-matrix-primary truncate" title={props.file.path}>
            {props.file.path.split(/[/\\]/).slice(-2).join('/')}
          </span>
        </div>
        
        <div class="flex items-center gap-3">
            <span class="text-sm opacity-40 font-mono hidden sm:inline-block">
                {props.file.relevant_lines?.length || 0} RANGES
            </span>
            <button
                onClick={handleCopyFile}
                class={`
                    text-lg uppercase font-bold tracking-wider px-2 py-0.5 border transition-all
                    ${copied() 
                        ? 'border-matrix-primary text-matrix-primary bg-matrix-primary/10' 
                        : 'border-transparent text-matrix-primary/40 hover:text-matrix-highlight hover:border-matrix-primary/30'}
                `}
            >
                {isLoading() && !isExpanded() ? "..." : (copied() ? "COPIED" : "COPY")}
            </button>
        </div>
      </div>

      <Show when={isExpanded()}>
        <div class="relative group/code min-h-[50px] bg-black/20">
            {/* Loading State */}
            <Show when={isLoading()}>
                <div class="flex items-center justify-center p-4 gap-2 text-matrix-primary/50 animate-pulse">
                     <div class="w-2 h-2 bg-matrix-primary rounded-full"></div>
                     <span class="text-sm font-mono uppercase tracking-widest">Fetching Content...</span>
                </div>
            </Show>
            
            {/* Error State */}
            <Show when={error()}>
                <div class="p-4 text-matrix-error border-l-2 border-matrix-error bg-matrix-error/5 font-mono text-sm">
                    ERR: {error()}
                </div>
            </Show>

            {/* Content State */}
            <Show when={content()}>
                <div class="absolute top-0 right-0 p-1 opacity-0 group-hover/code:opacity-100 transition-opacity pointer-events-none">
                    <div class="text-sm bg-black/80 text-matrix-primary px-2 border border-matrix-border">
                         {props.file.path}
                    </div>
                </div>
                <pre class="p-3 overflow-x-auto text-base opacity-80 max-h-[600px] scrollbar-thin leading-relaxed font-mono selection:bg-matrix-primary selection:text-matrix-bg">
                {content()}
                </pre>
            </Show>
        </div>
      </Show>
    </div>
  );
};

// --- Main List Component ---

export const ContextComposer = () => {
  const { state: workspaceState } = useWorkspace();
  const { recipeState } = useRecipe();
  
  const [forceExpand, setForceExpand] = createSignal<boolean | null>(null);
  const [isCopiedAll, setIsCopiedAll] = createSignal(false);
  const [isGeneratingXml, setIsGeneratingXml] = createSignal(false);

  // Generate the payload object required by the backend
  const currentRecipePayload = createMemo<EngineRecipe>(() => ({
    name: recipeState.activeRecipeName || "Interactive Session",
    description: null,
    operations: recipeState.steps.map((s) => s.op),
    transforms: {}, // Add transforms here if UI supports specific file overrides
    default_transform: null,
  }));

  // Estimate total tokens from metadata
  const totalTokens = createMemo(() => {
    return workspaceState.contextFiles.reduce((acc, file) => {
      return acc + estimateTokensFromMetadata(file);
    }, 0);
  });

  const handleCopyAll = async () => {
    if (isGeneratingXml()) return;
    
    setIsGeneratingXml(true);
    try {
        // Offload the XML generation to Rust to avoid UI freeze
        const xml = await invoke<string>("generate_xml_bundle", { 
            recipeJson: currentRecipePayload() 
        });
        
        await navigator.clipboard.writeText(xml);
        setIsCopiedAll(true);
        setTimeout(() => setIsCopiedAll(false), 2000);
    } catch (e) {
        console.error("Failed to generate XML bundle", e);
    } finally {
        setIsGeneratingXml(false);
    }
  };

  const toggleAll = () => {
    // If null or false, set to true. If true, set to false.
    setForceExpand((prev) => prev === true ? false : true);
  }

  const canAction = () => workspaceState.contextFiles.length > 0 && !workspaceState.isSyncing;

  return (
    <div class="h-full flex flex-col min-h-0 bg-matrix-panel/20">
      
      {/* Toolbar */}
      <div class="sticky top-0 bg-matrix-panel/90 backdrop-blur z-10 px-3 h-10 border-b border-matrix-border/30 flex justify-between items-center shrink-0">
        <div class="flex items-center gap-3">
            <span class="text-base uppercase tracking-widest opacity-40 font-bold">
                Output Stream
            </span>
            <Show when={workspaceState.contextFiles.length > 0}>
                <div class="flex items-center gap-3">
                  <span class="text-sm text-matrix-primary/50 font-mono">
                      ({workspaceState.contextFiles.length} FILES)
                  </span>
                  <div class="w-px h-3 bg-matrix-border/50"></div>
                  <span 
                    class="text-sm text-matrix-highlight/70 font-mono font-bold"
                    title="Estimated based on line count"
                  >
                      ~{formatTokenCount(totalTokens())} TOKENS
                  </span>
                </div>
            </Show>
        </div>
        
        <div class="flex items-center gap-2">
            <Show when={canAction()}>
                <button
                    onClick={toggleAll}
                    class="text-lg font-bold uppercase tracking-wider px-2 py-1 text-matrix-primary/50 hover:text-matrix-primary transition-colors"
                >
                    {forceExpand() === true ? "Collapse All" : "Expand All"}
                </button>
                <div class="w-px h-3 bg-matrix-border/50"></div>
            </Show>

            <button 
                onClick={handleCopyAll}
                disabled={!canAction() || isGeneratingXml()}
                class={`
                    text-lg font-bold uppercase tracking-widest px-3 py-1 border transition-all flex items-center gap-2
                    ${canAction() && !isGeneratingXml()
                        ? 'border-matrix-primary text-matrix-primary hover:bg-matrix-primary hover:text-matrix-bg hover:shadow-glow' 
                        : 'border-transparent text-matrix-primary/20 cursor-not-allowed'}
                `}
            >
                <Show when={isGeneratingXml()}>
                    <div class="w-3 h-3 border border-current border-t-transparent rounded-full animate-spin"></div>
                </Show>
                {workspaceState.isSyncing 
                    ? "SYNCING..." 
                    : isGeneratingXml() 
                        ? "BUILDING XML..." 
                        : (isCopiedAll() ? "XML COPIED!" : "[ COPY XML ]")
                }
            </button>
        </div>
      </div>

      {/* List Area */}
      <div class="flex-1 p-3 overflow-y-auto custom-scrollbar">
        <Show when={workspaceState.contextFiles.length === 0}>
            <div class="flex flex-col items-center justify-center h-full opacity-20 text-base font-mono tracking-widest text-center select-none">
                <div class="mb-2 text-4xl opacity-50">∅</div>
                <div>AWAITING INPUT</div>
                <div class="text-sm mt-2">ADD SYMBOLS TO GENERATE CONTEXT</div>
            </div>
        </Show>

        <div class="space-y-2 pb-10">
            <For each={workspaceState.contextFiles}>
                {(file) => (
                    <ContextFileItem 
                        file={file} 
                        forceState={forceExpand()} 
                        recipePayload={currentRecipePayload()}
                    />
                )}
            </For>
        </div>
      </div>
    </div>
  )
}