import { For, Show, createSignal, createEffect } from "solid-js";
import { useWorkspace, ContextFile } from "../../core/workspace.store";

/**
 * Matches Rust's escape_xml_attribute
 * Escapes: & < > " '
 */
const escapeXmlAttribute = (input: string): string => {
  return input
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;")
    .replace(/'/g, "&apos;");
};

/**
 * Matches Rust's escape_xml_content
 * Escapes: & < >
 * (Quotes are technically allowed in XML content, only strictly required in attributes)
 */
const escapeXmlContent = (input: string): string => {
  return input
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;");
};

const ContextFileItem = (props: { 
  file: ContextFile; 
  forceState?: boolean | null; 
}) => {
  const [isExpanded, setIsExpanded] = createSignal(false);
  const [copied, setCopied] = createSignal(false);

  createEffect(() => {
    if (props.forceState !== null && props.forceState !== undefined) {
      setIsExpanded(props.forceState);
    }
  });

  const handleCopyFile = (e: MouseEvent) => {
    e.stopPropagation();
    navigator.clipboard.writeText(props.file.content);
    setCopied(true);
    setTimeout(() => setCopied(false), 2000);
  };

  return (
    <div class="border border-matrix-border/50 rounded bg-matrix-bg overflow-hidden animate-[fadeIn_0.3s_ease-out]">
      {/* Header */}
      <div 
        onClick={() => setIsExpanded(!isExpanded())}
        class={`
            flex items-center justify-between px-3 py-2 cursor-pointer transition-colors select-none border-b
            ${isExpanded() 
                ? 'bg-matrix-panel border-matrix-border/30' 
                : 'bg-matrix-panel/50 border-transparent hover:bg-matrix-panel hover:border-matrix-border/30'}
        `}
      >
        <div class="flex items-center gap-3 min-w-0">
          <div class={`
            w-4 h-4 flex items-center justify-center transition-transform duration-200 text-tiny text-matrix-primary/70
            ${isExpanded() ? 'rotate-90' : 'rotate-0'}
          `}>
             ▶
          </div>
          <span class="text-[10px] font-bold uppercase text-matrix-bg bg-matrix-primary/40 px-1.5 rounded-sm">
            {props.file.language}
          </span>
          <span class="font-bold text-xs font-mono text-matrix-primary truncate" title={props.file.path}>
            {props.file.path.split(/[/\\]/).slice(-2).join('/')}
          </span>
        </div>
        
        <div class="flex items-center gap-3">
            <span class="text-[10px] opacity-40 font-mono hidden sm:inline-block">
                {props.file.relevant_lines?.length || 0} BLOCKS
            </span>
            <button
                onClick={handleCopyFile}
                class={`
                    text-[10px] uppercase font-bold tracking-wider px-2 py-0.5 rounded border transition-all
                    ${copied() 
                        ? 'border-matrix-primary text-matrix-primary bg-matrix-primary/10' 
                        : 'border-transparent text-matrix-primary/40 hover:text-matrix-highlight hover:border-matrix-primary/30'}
                `}
            >
                {copied() ? "COPIED" : "COPY"}
            </button>
        </div>
      </div>

      <Show when={isExpanded()}>
        <div class="relative group/code">
            <div class="absolute top-0 right-0 p-1 opacity-0 group-hover/code:opacity-100 transition-opacity pointer-events-none">
                 <div class="text-[10px] bg-black/80 text-matrix-primary px-2 border border-matrix-border">
                    {props.file.path}
                 </div>
            </div>
            <pre class="p-3 overflow-x-auto text-xs opacity-80 max-h-[600px] scrollbar-thin leading-relaxed font-mono selection:bg-matrix-primary selection:text-matrix-bg bg-black/20">
            {props.file.content}
            </pre>
        </div>
      </Show>
    </div>
  );
};

export const ContextComposer = () => {
  const { state } = useWorkspace();
  const [forceExpand, setForceExpand] = createSignal<boolean | null>(null);
  const [isCopiedAll, setIsCopiedAll] = createSignal(false);

  // --- 2. Updated Copy Logic ---
  const handleCopyAll = () => {
    let xml = "<documents>\n";

    state.contextFiles.forEach(file => {
        // Match Rust formatting exactly
        xml += `  <document path="${escapeXmlAttribute(file.path)}" language="${escapeXmlAttribute(file.language)}">\n`;
        xml += `    <source_code>\n`;
        xml += escapeXmlContent(file.content);
        xml += `\n    </source_code>\n`;
        xml += `  </document>\n`;
    });

    xml += "</documents>";

    navigator.clipboard.writeText(xml);
    setIsCopiedAll(true);
    setTimeout(() => setIsCopiedAll(false), 2000);
  };
  // -----------------------------

  const toggleAll = () => {
    setForceExpand((prev) => prev === true ? false : true);
  }

  const canAction = () => state.contextFiles.length > 0 && !state.isSyncing;

  return (
    <div class="h-full flex flex-col min-h-0 bg-matrix-panel/20">
      
      {/* Toolbar */}
      <div class="sticky top-0 bg-matrix-panel/90 backdrop-blur z-10 px-3 h-10 border-b border-matrix-border/30 flex justify-between items-center shrink-0">
        <div class="flex items-center gap-3">
            <span class="text-xs uppercase tracking-widest opacity-40 font-bold">
                Output Stream
            </span>
            <Show when={state.contextFiles.length > 0}>
                <span class="text-[10px] text-matrix-primary/50 font-mono">
                    ({state.contextFiles.length} FILES)
                </span>
            </Show>
        </div>
        
        <div class="flex items-center gap-2">
            <Show when={canAction()}>
                <button
                    onClick={toggleAll}
                    class="text-[10px] font-bold uppercase tracking-wider px-2 py-1 text-matrix-primary/50 hover:text-matrix-primary transition-colors"
                >
                    {forceExpand() === true ? "Collapse All" : "Expand All"}
                </button>
                <div class="w-px h-3 bg-matrix-border/50"></div>
            </Show>

            <button 
                onClick={handleCopyAll}
                disabled={!canAction()}
                class={`
                    text-xs font-bold uppercase tracking-widest px-3 py-1 border transition-all flex items-center gap-2
                    ${canAction() 
                        ? 'border-matrix-primary text-matrix-primary hover:bg-matrix-primary hover:text-matrix-bg hover:shadow-glow' 
                        : 'border-transparent text-matrix-primary/20 cursor-not-allowed'}
                `}
            >
                {state.isSyncing ? "SYNCING..." : (isCopiedAll() ? "XML COPIED!" : "[ COPY XML ]")}
            </button>
        </div>
      </div>

      {/* List Area */}
      <div class="flex-1 p-3 overflow-y-auto custom-scrollbar">
        <Show when={state.contextFiles.length === 0}>
            <div class="flex flex-col items-center justify-center h-full opacity-20 text-xs font-mono tracking-widest text-center select-none">
                <div class="mb-2 text-4xl opacity-50">∅</div>
                <div>AWAITING INPUT</div>
                <div class="text-[10px] mt-2">ADD SYMBOLS TO GENERATE CONTEXT</div>
            </div>
        </Show>

        <div class="space-y-2 pb-10">
            <For each={state.contextFiles}>
                {(file) => (
                    <ContextFileItem 
                        file={file} 
                        forceState={forceExpand()} 
                    />
                )}
            </For>
        </div>
      </div>
    </div>
  )
}