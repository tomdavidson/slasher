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
  site: 'https://example.com/',
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
      social: [{ icon: 'github', label: 'GitHub', href: 'https://github.com/tomdavidson/solidus' }],
      sidebar: [{
        label: 'Guides',
        items: [
          // Each item here is one entry in the navigation menu.
          { label: 'Example Guide', slug: 'guides/example' },
        ],
      }, { label: 'Reference', autogenerate: { directory: 'reference' } }],
      plugins: [
        starlightThemeFlexoki(),
        starlightLlmsTxt(),
        starlightChangelogs(),
        // starlightVersions()
      ],
    }),
  ],
})
