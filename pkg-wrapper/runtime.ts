export type RuntimeVariant = "scalar" | "simd";

const SIMD_TEST_BYTES = new Uint8Array([
    0, 97, 115, 109, 1, 0, 0, 0, 1, 5, 1, 96, 0, 1, 123, 3, 2, 1, 0, 10, 10, 1, 8,
    0, 65, 0, 253, 15, 253, 98, 11,
]);

export function supportsSimd(): boolean {
    try {
        return WebAssembly.validate(SIMD_TEST_BYTES);
    } catch {
        return false;
    }
}

export function selectRuntime<T>(
    scalar: T,
    simd: T,
    simdSupported = supportsSimd(),
): { variant: RuntimeVariant; module: T } {
    return simdSupported
        ? { variant: "simd", module: simd }
        : { variant: "scalar", module: scalar };
}
