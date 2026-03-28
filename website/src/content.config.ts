import { docsLoader } from '@astrojs/starlight/loaders'
import { docsSchema } from '@astrojs/starlight/schema'
import { defineCollection } from 'astro:content'
// import { changelogsLoader } from 'starlight-changelogs/loader'

export const collections = {
  docs: defineCollection({ loader: docsLoader(), schema: docsSchema() }),
  // changelogs: defineCollection({
  //   loader: changelogsLoader([{
  //     provider: 'github',
  //     owner: 'tomdavidson',
  //     repo: 'solidus',
  //     base: 'changelog',
  //     enabled: !!import.meta.env.GH_API_TOKEN,
  //     token: import.meta.env.GH_API_TOKEN,
  //     title: 'Version History',
  //   }]),
  // }),
}
