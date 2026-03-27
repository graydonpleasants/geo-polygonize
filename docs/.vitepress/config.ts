import { defineConfig } from 'vitepress'

export default defineConfig({
  base: '/geo-polygonize/',
  title: "geo-polygonize",
  description: "A native Rust port of the JTS/GEOS polygonization algorithm (Wasm)",
  ignoreDeadLinks: [
    /^\/playground\//,
  ],
  themeConfig: {
    nav: [
      { text: 'Home', link: '/' },
      { text: 'Guide', link: '/guide/getting-started' },
      { text: 'Reference', link: '/reference/' },
      { text: 'Examples', link: '/examples/' }
    ],
    sidebar: [
      {
        text: 'Guide',
        items: [
          { text: 'Getting Started', link: '/guide/getting-started' },
          { text: 'WASM Integration', link: '/guide/wasm' },
        ]
      },
      {
        text: 'Reference',
        items: [
          { text: 'Options', link: '/reference/options' },
          { text: 'WASM API', link: '/reference/wasm-api' },
        ]
      },
      {
        text: 'Examples',
        link: '/examples/'
      }
    ],
    socialLinks: [
      { icon: 'github', link: 'https://github.com/graydonpleasants/geo-polygonize' }
    ]
  }
})
