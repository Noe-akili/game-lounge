/** @type {import('tailwindcss').Config} */
export default {
  content: [
    "./index.html",
    "./src/**/*.{vue,js,ts,jsx,tsx}",
  ],
  darkMode: 'class',
  theme: {
    extend: {
      colors: {
        bg: {
          DEFAULT: '#0a0a0f',
          card: '#12121a',
          surface: '#1a1a2e',
          hover: '#22223a',
        },
        neon: {
          violet: '#a855f7',
          blue: '#06b6d4',
          green: '#22c55e',
          red: '#ef4444',
          yellow: '#eab308',
        },
        txt: {
          DEFAULT: '#e2e8f0',
          muted: '#94a3b8',
          dim: '#64748b',
        }
      },
      fontFamily: {
        gaming: ['Montserrat', 'Inter', 'system-ui', 'sans-serif'],
        body: ['Inter', 'system-ui', 'sans-serif'],
      },
      boxShadow: {
        'neon-violet': '0 0 15px rgba(168, 85, 247, 0.3)',
        'neon-blue': '0 0 15px rgba(6, 182, 212, 0.3)',
        'neon-green': '0 0 15px rgba(34, 197, 94, 0.3)',
        'neon-red': '0 0 15px rgba(239, 68, 68, 0.3)',
        'neon-violet-strong': '0 0 25px rgba(168, 85, 247, 0.5), 0 0 50px rgba(168, 85, 247, 0.2)',
        'neon-blue-strong': '0 0 25px rgba(6, 182, 212, 0.5), 0 0 50px rgba(6, 182, 212, 0.2)',
        'gaming': '0 4px 20px rgba(0, 0, 0, 0.3), 0 0 40px rgba(168, 85, 247, 0.1)',
        'gaming-hover': '0 8px 30px rgba(0, 0, 0, 0.4), 0 0 60px rgba(168, 85, 247, 0.2)',
      },
      borderRadius: {
        '2xl': '16px',
        '3xl': '24px',
      },
      minHeight: {
        'btn': '56px',
      },
      animation: {
        'pulse-neon': 'pulseNeon 2s ease-in-out infinite',
        'glow': 'glow 1.5s ease-in-out infinite alternate',
        'float': 'float 3s ease-in-out infinite',
        'shimmer': 'shimmer 2s infinite',
        'border-glow': 'borderGlow 2s ease-in-out infinite',
      },
      keyframes: {
        pulseNeon: {
          '0%, 100%': { opacity: '1' },
          '50%': { opacity: '0.7' },
        },
        glow: {
          '0%': { boxShadow: '0 0 5px rgba(168, 85, 247, 0.5)' },
          '100%': { boxShadow: '0 0 20px rgba(168, 85, 247, 0.8), 0 0 30px rgba(168, 85, 247, 0.4)' },
        },
        float: {
          '0%, 100%': { transform: 'translateY(0px)' },
          '50%': { transform: 'translateY(-5px)' },
        },
        shimmer: {
          '0%': { backgroundPosition: '-200% 0' },
          '100%': { backgroundPosition: '200% 0' },
        },
        borderGlow: {
          '0%, 100%': { borderColor: 'rgba(168, 85, 247, 0.3)' },
          '50%': { borderColor: 'rgba(168, 85, 247, 0.7)' },
        },
      }
    },
  },
  plugins: [],
}
