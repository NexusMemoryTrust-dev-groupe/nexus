import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';

export default defineConfig({
  base: './',
  plugins: [react()],
  clearScreen: false,
  server: {
    port: 5173,
    strictPort: true,
    watch: {
      ignored: ['**/src-tauri/**'],
    },
  },
  build: {
    chunkSizeWarningLimit: 1200, // Three.js (1.1MB) + Tiptap (583KB) are large vendor libs, cannot be split further
    rollupOptions: {
      output: {
        manualChunks: {
          'vendor-react': ['react', 'react-dom'],
          'vendor-three': ['three', '@react-three/fiber', '@react-three/drei', '@react-three/postprocessing', 'postprocessing'],
          'vendor-tiptap': [
            '@tiptap/react', '@tiptap/starter-kit',
            '@tiptap/extension-heading', '@tiptap/extension-highlight',
            '@tiptap/extension-image', '@tiptap/extension-link',
            '@tiptap/extension-table', '@tiptap/extension-table-row',
            '@tiptap/extension-table-cell', '@tiptap/extension-table-header',
            '@tiptap/extension-task-item', '@tiptap/extension-task-list',
            '@tiptap/extension-placeholder', '@tiptap/extension-typography',
            '@tiptap/extension-bullet-list', '@tiptap/extension-ordered-list',
            '@tiptap/extension-code-block', '@tiptap/extension-horizontal-rule',
            '@tiptap/extension-list-item', 'tiptap-markdown',
          ],
          'vendor-graph': ['@antv/g6'],
          'vendor-motion': ['framer-motion'],
        },
      },
    },
  },
});
