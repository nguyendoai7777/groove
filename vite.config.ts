import { defineConfig } from 'vite'
import vue from '@vitejs/plugin-vue'
import tailwindcss from '@tailwindcss/vite'
import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'

function resolvePathsAlias() {
  const tsConfigRaw = readFileSync('./tsconfig.json', 'utf-8')
  const cleanedTsConfig = tsConfigRaw.replace(/\/\/.*$/gm, '').replace(/\/\*[\s\S]*?\*\//g, '')
  const tsconfig = JSON.parse(cleanedTsConfig)
  const paths = tsconfig.compilerOptions.paths || {}
  const aliases: Record<string, string> = {}
  for (const [key, value] of Object.entries(paths)) {
    const aliasKey = key.replace(/\/\*$/, '')
    const aliasPath = (value as string[])[0].replace(/\/\*$/, '')
    aliases[aliasKey] = resolve(process.cwd(), aliasPath)
  }
  return aliases
}

const host = process.env.TAURI_DEV_HOST

export default defineConfig(async () => ({
  plugins: [vue(), tailwindcss()],
  resolve: {
    alias: resolvePathsAlias(),
    extensions: ['.ts', '.vue'],
    mainFields: ['module', 'main'],
  },
  build: {
    assetsInlineLimit: 0, // Disables base64 inlining for all assets
  },
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    host: host || false,
    hmr: host
      ? {
          protocol: 'ws',
          host,
          port: 1421,
        }
      : undefined,
    watch: {
      ignored: ['**/src-tauri/**'],
    },
  },
}))
