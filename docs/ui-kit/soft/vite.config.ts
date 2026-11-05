import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'
import tailwindcss from '@tailwindcss/vite'
import { viteSingleFile } from 'vite-plugin-singlefile'

export default defineConfig({
  // viteSingleFile inlines all JS and CSS, so `npm run build` makes one demo.html that opens by double-click.
  plugins: [react(), tailwindcss(), viteSingleFile()],
  // The kit's components live one folder up; let them resolve this project's react/motion.
  resolve: { dedupe: ['react', 'react-dom', 'motion'] },
  server: { fs: { allow: ['..'] } }
})
