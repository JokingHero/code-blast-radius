import { Component } from "solid-js";

const MatrixSpinner: Component = () => {
  return (
    <div class="relative flex items-center justify-center w-24 h-24">
      {/* HUD Ring 1: Spinning dashed border */}
      <div class="absolute inset-0 rounded-full border border-matrix-primary/30 border-t-matrix-primary animate-[spin_3s_linear_infinite]"></div>
      
      {/* HUD Ring 2: Counter-spinning inner ring */}
      <div class="absolute inset-2 rounded-full border border-matrix-primary/20 border-b-matrix-primary/80 animate-[spin_2s_linear_infinite_reverse]"></div>
      
      {/* Core: Pulsing Hexagon/Circle */}
      <div class="absolute inset-8 bg-matrix-primary/10 rounded-full animate-pulse flex items-center justify-center border border-matrix-primary/50 shadow-glow">
         <div class="w-1 h-1 bg-matrix-primary shadow-[0_0_10px_#00FF41]"></div>
      </div>

      {/* Scanning Line Effect */}
      <div class="absolute inset-0 rounded-full overflow-hidden opacity-50">
        <div class="w-full h-1/2 bg-gradient-to-b from-transparent to-matrix-primary/20 animate-[scan_2s_linear_infinite]"></div>
      </div>
    </div>
  );
};

export default MatrixSpinner;