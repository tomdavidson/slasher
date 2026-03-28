// @ts-check
import starlight from '@astrojs/starlight'
import { defineConfig } from 'astro/config'
import mermaid from 'astro-mermaid'
import starlightChangelogs from 'starlight-changelogs'
import starlightLlmsTxt from 'starlight-llms-txt'
import starlightThemeFlexoki from 'starlight-theme-flexoki'
import starlightVersions from 'starlight-versions'

// https://astro.build/config
export default defineConfig({
  site: 'https://solidus.tomdavidson.dev/',
  vite: {
    resolve: {
      alias: {
        'nanoid/non-secure': new URL('./nanoid-non-secure-shim.mjs', import.meta.url).pathname,
      },
    },
  },
  integrations: [
    mermaid({
      // Default theme: 'default', 'dark', 'forest', 'neutral', 'base'
      theme: 'forest',

      // Enable automatic theme switching based on data-theme attribute
      autoTheme: true,

      // Enable client-side logging (default: true). Set to false to suppress
      // console.log output in the browser. Errors are always logged.
      enableLog: false,

      // Additional mermaid configuration
      mermaidConfig: { flowchart: { curve: 'basis' } },

      // Register icon packs for use in diagrams
      iconPacks: [{
        name: 'logos',
        loader: () => fetch('https://unpkg.com/@iconify-json/logos@1/icons.json').then(res => res.json()),
      }, {
        name: 'iconoir',
        loader: () => fetch('https://unpkg.com/@iconify-json/iconoir@1/icons.json').then(res => res.json()),
      }],
    }),
    starlight({
      title: 'Solidus',
      description: 'The gold standard for slash command parsing.',
      head: [
        { tag: 'meta', attrs: { property: 'og:title', content: 'Solidus' } },
        { tag: 'meta', attrs: { property: 'og:description', content: 'The gold standard for slash command parsing. A formally specified, pure Rust parser for /command syntax in UTF-8 text.' } },
        { tag: 'meta', attrs: { property: 'og:type', content: 'website' } },
      ],
      social: [{ icon: 'github', label: 'GitHub', href: 'https://github.com/tomdavidson/solidus' }],
      sidebar: [
        { label: 'Overview', slug: 'index' },
        { label: 'Set List', slug: 'examples' },
        {
          label: 'The Spec',
          items: [
            { label: 'Syntax v1.1.0', slug: 'spec/syntax' },
            { label: 'Engine v0.5.0', slug: 'spec/engine' },
          ],
        },
        { label: 'Soundcheck', slug: 'soundcheck' },
        {
          label: 'SDKs',
          badge: { text: 'Coming Soon', variant: 'caution' },
          items: [
            { label: 'Rust', slug: 'sdks/rust' },
            { label: 'WASM / JavaScript', slug: 'sdks/wasm' },
            { label: 'WASI', slug: 'sdks/wasi' },
          ],
        },
      ],
      plugins: [
        starlightThemeFlexoki(),
        starlightLlmsTxt(),
        // starlightChangelogs(),
        // starlightVersions()
      ],
    }),
  ],
})
