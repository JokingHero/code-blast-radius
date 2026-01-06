import { For } from "solid-js";

interface Option<T> {
  label: string;
  value: T;
  title?: string;
}

interface SegmentedControlProps<T> {
  options: Option<T>[];
  value: T;
  onChange: (value: T) => void;
  disabled?: boolean;
}

export function SegmentedControl<T>(props: SegmentedControlProps<T>) {
  return (
    <div class="flex h-5 border border-matrix-border rounded-sm overflow-hidden shrink-0">
      <For each={props.options}>
        {(opt, i) => (
          <>
            {i() > 0 && <div class="w-px bg-matrix-border"></div>}
            <button
              onClick={() => props.onChange(opt.value)}
              disabled={props.disabled}
              title={opt.title}
              class={`
                px-2 flex items-center justify-center text-lg font-bold uppercase tracking-wider transition-all
                ${props.value === opt.value
                  ? "bg-matrix-primary text-matrix-bg shadow-[inset_0_0_5px_rgba(0,0,0,0.2)]"
                  : "bg-transparent text-matrix-primary/50 hover:text-matrix-primary hover:bg-matrix-primary/5"}
              `}
            >
              {opt.label}
            </button>
          </>
        )}
      </For>
    </div>
  );
}