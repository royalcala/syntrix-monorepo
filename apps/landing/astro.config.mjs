import { defineConfig } from "astro/config";
import tailwindcss from "@tailwindcss/vite";
import mdx from "@astrojs/mdx";

export default defineConfig({
  site: "https://syntrix.mx",
  output: "static",
  integrations: [mdx()],
  vite: {
    plugins: [tailwindcss()],
  },
  i18n: {
    defaultLocale: "es",
    locales: ["es", "en", "pt-BR"],
    routing: {
      prefixDefaultLocale: true,
    },
  },
});
