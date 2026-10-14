import url from "@rollup/plugin-url";
import commonjs from "@rollup/plugin-commonjs";
import resolve from "@rollup/plugin-node-resolve";
import typescript from "@rollup/plugin-typescript";
import { rm } from "node:fs/promises";

const nodeCommonJS = (env) => ({
  name: "node-commonjs-runtime",
  async buildStart() {
    await rm(`dist/${env}/cjs/${env === "slim" ? "index_slim" : "index"}.js`, { force: true });
  },
  resolveImportMeta(property) {
    return property === "url"
      ? "require('node:url').pathToFileURL(__filename).href"
      : null;
  },
  transform(code, id) {
    if (!id.replaceAll("\\", "/").endsWith("/pkg-scalar/geo_polygonize.js")) return null;
    const fallback = /new URL\((?:\/\* @vite-ignore \*\/\s*)?'geo_polygonize_bg\.wasm', import\.meta\.url\)/g;
    if (code.match(fallback)?.length !== 1) {
      throw new Error("Expected one wasm-bindgen default Wasm URL for CommonJS");
    }
    return {
      code: code.replace(fallback,
        "require('node:fs').readFileSync(require('node:path').join(__dirname, '../../geo_polygonize.wasm'))"),
      map: null,
    };
  },
});

const rolls = (fmt, env) => {
  let input;
  if (env === "threads") {
    input = "pkg-wrapper/index_threads.ts";
  } else if (env === "slim") {
    input = "pkg-wrapper/index_slim.ts";
  } else {
    input = "pkg-wrapper/index.ts";
  }

  return {
    input,
    external: env === "threads" ? ["../pkg-threads/geo_polygonize.js"] : [],
    output: {
      dir: `dist/${env}/${fmt}`,
      format: fmt,
      entryFileNames: `${env === "slim" ? "index_slim" : "index"}.${fmt === "cjs" ? "cjs" : "js"}`,
      exports: "named",
    },
    plugins: [
      resolve(),
      commonjs(),
      typescript({
        declaration: true,
        outDir: `dist/${env}/${fmt}`,
        rootDir: "pkg-wrapper",
      }),
      env === "standard" && url({
        include: ["**/*.wasm"],
        limit: Infinity, // Always inline as data:application/wasm;base64,...
        emitFiles: false,
      }),
      fmt === "cjs" && nodeCommonJS(env),
    ].filter(Boolean),
  };
};

const workerRoll = {
  input: "pkg-wrapper/polygonize_worker.ts",
  output: {
    dir: "dist/standard/es",
    format: "es",
    entryFileNames: "polygonize_worker.js",
  },
  plugins: [
    resolve(),
    commonjs(),
    typescript({
      declaration: true,
      outDir: "dist/standard/es",
      rootDir: "pkg-wrapper",
    }),
    url({
      include: ["**/*.wasm"],
      limit: Infinity,
      emitFiles: false,
    }),
  ],
};

export default [
  rolls("es", "standard"),
  rolls("cjs", "standard"),
  rolls("es", "slim"),
  rolls("cjs", "slim"),
  rolls("es", "threads"),
  workerRoll,
];
