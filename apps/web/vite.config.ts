import { createHash } from 'node:crypto';
import { readFileSync } from 'node:fs';
import { defineConfig, loadEnv } from 'vite';
import { svelte } from '@sveltejs/vite-plugin-svelte';

export default defineConfig(({ mode }) => {
  const env = loadEnv(mode, '.', 'API_');
  const target = env.API_PROXY_TARGET || 'http://127.0.0.1:8080';
  const proxy = {
    '/api': { target, changeOrigin: true },
    '/health': { target, changeOrigin: true },
  };

  return {
    plugins: [svelte(), {
      name: 'flo-offline-shell',
      apply: 'build',
      generateBundle(_options, bundle) {
        const worker = readFileSync(new URL('./src/offline-worker.js', import.meta.url), 'utf8');
        const assets = ['/', '/manifest.webmanifest', '/fonts/micro-5.ttf', '/icons/flo-local-192.png',
          '/icons/flo-local-512.png', '/icons/flo-local-maskable-512.png', '/icons/flo-local-apple-180.png',
          ...Object.keys(bundle).filter((name) => name.startsWith('assets/')).map((name) => '/' + name)];
        const version = createHash('sha256').update(worker).update(JSON.stringify(assets)).digest('hex').slice(0, 16);
        this.emitFile({ type: 'asset', fileName: 'sw.js', source:
          'const SHELL_VERSION = ' + JSON.stringify(version) + ';\nconst SHELL_ASSETS = ' + JSON.stringify(assets) + ';\n' + worker });
      },
    }],
    server: {
      host: '127.0.0.1',
      port: 5173,
      strictPort: true,
      proxy,
    },
    preview: {
      host: '127.0.0.1',
      port: 4173,
      strictPort: true,
      proxy,
    },
  };
});
