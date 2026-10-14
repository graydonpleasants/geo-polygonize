import { execFileSync } from "node:child_process";
import { describe, expect, it } from "vitest";

describe("CommonJS package", () => {
    it("loads and initializes standard/slim with managed packed lifecycles", () => {
        const output = execFileSync(process.execPath, ["scripts/test_wasm_commonjs_package.mjs"], {
            cwd: process.cwd(), encoding: "utf8", timeout: 60_000,
        });
        expect(output).toContain("built package smoke passed");
    }, 60_000);
});
