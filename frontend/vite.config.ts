import { sveltekit } from '@sveltejs/kit/vite';
import { defineConfig } from 'vite';

export default defineConfig({
	plugins: [sveltekit()],
	server: {
		proxy: {
			'/api/ws': {
				target: 'ws://localhost:8484',
				ws: true,
			},
			'/api': {
				target: 'http://localhost:8484',
				changeOrigin: true,
			},
		},
	},
	build: {
		// Enable minification for smaller bundle size (using esbuild)
		minify: true,
		
		// Rollup options for code splitting
		rollupOptions: {
			output: {
				// Manual chunks for better caching
				manualChunks: {
					// Vendor chunk (framework code)
					vendor: ['svelte', 'svelte/store', 'svelte/transition'],
				},
				
				// Chunk file naming for better caching
				chunkFileNames: 'chunks/[name]-[hash].js',
				entryFileNames: 'entries/[name]-[hash].js',
				assetFileNames: 'assets/[name]-[hash][extname]',
			},
		},
		
		// Chunk size warnings (500 KB)
		chunkSizeWarningLimit: 500,
	},
	
	// Optimize dependencies
	optimizeDeps: {
		include: ['svelte', 'svelte/store'],
	},
});
