import { wasm } from "@rollup/plugin-wasm";
import commonjs from "@rollup/plugin-commonjs";
import resolve from "@rollup/plugin-node-resolve";
import typescript from "@rollup/plugin-typescript";

const rolls = (fmt, env) => ({
  input: env !== "slim" ? "pkg-wrapper/index.ts" : "pkg-wrapper/index_slim.ts",
  output: {
    dir: `dist/${env}/${fmt}`,
    format: fmt,
    entryFileNames: `[name].js`,
    exports: "named", // Disable "Mixing named and default exports" warning
  },
  plugins: [
    resolve(),
    commonjs(),
    typescript(),
    env !== "slim" && wasm({ maxFileSize: 10000000, targetEnv: "auto-inline" }),
  ].filter(Boolean),
});

export default [
  rolls("es", "standard"),
  rolls("cjs", "standard"),
  rolls("es", "slim"),
  rolls("cjs", "slim"),
];
