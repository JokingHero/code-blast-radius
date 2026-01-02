import { Show } from "solid-js";
import { Portal } from "solid-js/web";
import { isLoading, loadingMessage } from "../store";
import MatrixSpinner from "./MatrixSpinner";

export const GlobalLoader = () => {
  return (
    <Show when={isLoading()}>
      <Portal>
        {/* Backdrop with Blur and Dim */}
        <div class="fixed inset-0 z-[9999] bg-matrix-bg/90 backdrop-blur-sm flex flex-col items-center justify-center select-none cursor-wait">
          
          <MatrixSpinner />
          
          {/* Loading Text with Typing/Blinking Effect */}
          <div class="mt-8 text-center space-y-2">
            <div class="text-matrix-primary font-mono text-sm tracking-[0.2em] font-bold uppercase drop-shadow-[0_0_5px_rgba(0,255,65,0.8)]">
              {loadingMessage()}
            </div>
            <div class="text-matrix-primary/50 text-[10px] animate-pulse">
              [ PLEASE WAIT // CALCULATING DEPENDENCIES ]
            </div>
          </div>

          {/* Decorative Corner Brackets */}
          <div class="absolute top-10 left-10 w-8 h-8 border-t-2 border-l-2 border-matrix-primary opacity-50"></div>
          <div class="absolute bottom-10 right-10 w-8 h-8 border-b-2 border-r-2 border-matrix-primary opacity-50"></div>
          <div class="absolute top-10 right-10 w-8 h-8 border-t-2 border-r-2 border-matrix-primary opacity-50"></div>
          <div class="absolute bottom-10 left-10 w-8 h-8 border-b-2 border-l-2 border-matrix-primary opacity-50"></div>
        </div>
      </Portal>
    </Show>
  );
};