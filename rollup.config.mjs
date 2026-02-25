import url from "@rollup/plugin-url";
import commonjs from "@rollup/plugin-commonjs";
import resolve from "@rollup/plugin-node-resolve";
import typescript from "@rollup/plugin-typescript";
import path from "path";

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
    output: {
      dir: `dist/${env}/${fmt}`,
      format: fmt,
      entryFileNames: env === "slim" ? "index_slim.js" : "index.js",
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
      env === "threads" && url({
        include: ["**/*.wasm"],
        limit: 0, // Never inline
        emitFiles: true,
        fileName: "[name][extname]",
      }),
      // Handle wasm-bindgen-rayon workerHelpers import
      env === "threads" && {
        name: 'wasm-bindgen-rayon-workaround',
        resolveId(source, importer) {
          if (source === '../../..') {
            return { id: path.resolve('pkg-threads/geo_polygonize.js') };
          }
          return null;
        }
      },
    ].filter(Boolean),
  };
};

export default [
  rolls("es", "standard"),
  rolls("cjs", "standard"),
  rolls("es", "slim"),
  rolls("cjs", "slim"),
  rolls("es", "threads"),
];
