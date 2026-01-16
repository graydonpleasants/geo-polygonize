import url from "@rollup/plugin-url";
import commonjs from "@rollup/plugin-commonjs";
import resolve from "@rollup/plugin-node-resolve";
import typescript from "@rollup/plugin-typescript";

const rolls = (fmt, env) => ({
  input: env !== "slim" ? "pkg-wrapper/index.ts" : "pkg-wrapper/index_slim.ts",
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
    env !== "slim" && url({
      include: ["**/*.wasm"],
      limit: Infinity, // Always inline as data:application/wasm;base64,...
      emitFiles: false,
    }),
  ].filter(Boolean),
});

export default [
  rolls("es", "standard"),
  rolls("cjs", "standard"),
  rolls("es", "slim"),
  rolls("cjs", "slim"),
];
