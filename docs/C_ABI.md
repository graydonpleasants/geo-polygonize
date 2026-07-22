# Arrow C ABI

`geo-polygonize-arrow` exposes an Arrow C Data Interface boundary for C and
other FFI consumers. Query `polygonize_ffi_abi_version()` before calling it;
version `1` is the current compatible ABI.

New callers must use `polygonize_with_options_ffi` with the canonical JSON
`PolygonizerOptions` payload. `polygonize_ffi` and its `PolygonizerOptions`
`repr(C)` request remain supported through `1.x`, but are legacy and will not
gain fields.

```c
uint32_t polygonize_ffi_abi_version(void);
int32_t polygonize_with_options_ffi(
    struct ArrowArray *input_array,
    struct ArrowSchema *input_schema,
    struct ArrowArray *output_array,
    struct ArrowSchema *output_schema,
    const uint8_t *options_json,
    size_t options_json_len);
const struct PolygonizeFfiLastError *polygonize_ffi_last_error(void);
```

## Status and error contract

The returned `int32_t` values are ABI-stable: `0` success, `1` invalid
argument, `2` invalid Arrow C input, `4` output schema export failure, `5`
invalid buffer shape, `6` invalid option, `7` invalid geometry, `8` topology
failure, `9` unsupported option combination, `10` internal invariant failure,
`11` Arrow adapter failure, `12` unknown failure, and `99` contained panic.

On a nonzero result, `polygonize_ffi_last_error()` returns a thread-local
`PolygonizeFfiLastError` with the matching status plus NUL-terminated `family`,
`stage`, `message`, and `witness` strings. `witness` is empty when unavailable;
otherwise it is normalized-error JSON, including noding witness IDs when
present. The pointer and strings remain valid until the next polygonize FFI
call on that thread. A successful call clears the error and the query returns
null. Copy strings before another call.

## Ownership and nullability

All Arrow pointers and the options pointer must be non-null. `options_json` is
borrowed only for the call. `input_schema` is borrowed and is never consumed.

After a valid input schema is accepted, `input_array` is consumed regardless
of success or failure; callers must not release or reuse it afterwards. If
schema validation fails before that point, both inputs remain caller-owned.

`output_array` and `output_schema` are caller-provided, safe-to-overwrite
destinations. They are unchanged on failure. On success, they receive Arrow C
objects owned by the caller; release each through its Arrow C `release`
callback exactly once. `polygonize_result_free` is a legacy no-op and must not
be used for Arrow C outputs.

Calls are thread-safe when every call uses separate Arrow objects. Last-error
state is per-thread. Do not concurrently access, reuse, or release any request
or output object while a call using it is in progress; the API makes no
callbacks and requires no special reentrancy handling beyond that ownership
rule.
