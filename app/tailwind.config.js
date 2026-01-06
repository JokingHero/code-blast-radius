module.exports = {
  content: ["./src/**/*.{js,jsx,ts,tsx}"],
  theme: {
    extend: {
      colors: {
        matrix: {
          bg: "#050505",       // Deep black
          panel: "#0a0f0a",    // Slightly lighter black for panels
          border: "#1a2e1a",   // Dark green borders
          text: "#003b00",     // Dimmed green text
          primary: "#00ff41",  // Bright Matrix Green (Terminal)
          highlight: "#ccffcc", // Almost white green
          error: "#ff0000",
        },
      },
      fontFamily: {
        mono: ['"JetBrains Mono"', '"Fira Code"', 'monospace'], // Developer friendly
      },
      boxShadow: {
        'glow': '0 0 10px rgba(0, 255, 65, 0.3)',
      },
      fontSize: {
        // We redefine the scale to be slightly larger overall
        'micro': '12px',      
        'tiny': '14px',       
        'xs': '15px',       
        'sm': '17px',        
        'base': '18px',
        'lg': '20px',
        'xl': '22px',
      }
    },
  },
  plugins: [],
}