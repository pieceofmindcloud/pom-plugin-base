import { build } from "esbuild";
import { createRequire } from "node:module";
import { mkdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { join } from "node:path";

const require = createRequire(import.meta.url);
const manifest = JSON.parse(readFileSync("manifest.json", "utf8"));
const pluginCode = manifest.plugin_code;
if (typeof pluginCode !== "string" || !/^[a-z][a-z0-9_]{1,63}$/.test(pluginCode)) {
  throw new Error("manifest plugin_code must be a valid plugin identifier");
}

const hostModules = {
  react: "React",
  "react-dom": "ReactDOM",
  "react/jsx-runtime": "jsxRuntime",
};

function hostModule(specifier, property) {
  const names = Object.keys(require(specifier)).filter(
    (name) => /^[A-Za-z_$][\w$]*$/.test(name) && name !== "default",
  );
  return [
    `const module = globalThis.__POM_HOST__.${property};`,
    "export default module;",
    ...names.map((name) => `export const ${name} = module.${name};`),
  ].join("\n");
}

function namespacePluginCode(source) {
  return source.replaceAll("pb-", `${pluginCode}-`);
}

function namespaceLocaleCatalog(catalog) {
  return Object.fromEntries(
    Object.entries(catalog).map(([key, value]) => [`${pluginCode}.${key}`, value]),
  );
}

const hostPlugin = {
  name: "pom-host",
  setup(builder) {
    builder.onResolve(
      { filter: /^(react|react-dom|react\/jsx-runtime)$/ },
      (args) => ({ path: args.path, namespace: "pom-host" }),
    );
    builder.onLoad({ filter: /.*/, namespace: "pom-host" }, (args) => ({
      contents: hostModule(args.path, hostModules[args.path]),
      loader: "js",
    }));
  },
};

rmSync("dist", { recursive: true, force: true });
mkdirSync("dist", { recursive: true });
await build({
  entryPoints: { screens: "src/screens/index.tsx" },
  outdir: "dist",
  bundle: true,
  format: "esm",
  target: "es2022",
  minify: true,
  jsx: "automatic",
  loader: { ".css": "empty" },
  define: {
    "process.env.NODE_ENV": '"production"',
    __POM_PLUGIN_CODE__: JSON.stringify(pluginCode),
  },
  plugins: [hostPlugin],
  logLevel: "info",
});
const screensPath = "dist/screens.js";
writeFileSync(screensPath, namespacePluginCode(readFileSync(screensPath, "utf8")));

const css = namespacePluginCode(readFileSync("src/plugin.css", "utf8"));
writeFileSync("dist/plugin.css", css);
mkdirSync("dist/i18n", { recursive: true });
for (const [locale, catalogPath] of Object.entries(manifest.i18n)) {
  const catalog = JSON.parse(readFileSync(join("..", catalogPath), "utf8"));
  const namespacedCatalog = namespaceLocaleCatalog(catalog);
  writeFileSync(`dist/i18n/${locale}.json`, `${JSON.stringify(namespacedCatalog, null, 2)}\n`);
}

console.log("dist/plugin.css", css.length, "bytes");
