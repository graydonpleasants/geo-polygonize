'use strict';

var _documentCurrentScript = typeof document !== 'undefined' ? document.currentScript : null;
let wasm$1;

function getArrayU8FromWasm0$1(ptr, len) {
    ptr = ptr >>> 0;
    return getUint8ArrayMemory0$1().subarray(ptr / 1, ptr / 1 + len);
}

let cachedDataViewMemory0$1 = null;
function getDataViewMemory0$1() {
    if (cachedDataViewMemory0$1 === null || cachedDataViewMemory0$1.buffer.detached === true || (cachedDataViewMemory0$1.buffer.detached === undefined && cachedDataViewMemory0$1.buffer !== wasm$1.memory.buffer)) {
        cachedDataViewMemory0$1 = new DataView(wasm$1.memory.buffer);
    }
    return cachedDataViewMemory0$1;
}

let cachedFloat64ArrayMemory0$1 = null;
function getFloat64ArrayMemory0$1() {
    if (cachedFloat64ArrayMemory0$1 === null || cachedFloat64ArrayMemory0$1.byteLength === 0) {
        cachedFloat64ArrayMemory0$1 = new Float64Array(wasm$1.memory.buffer);
    }
    return cachedFloat64ArrayMemory0$1;
}

function getStringFromWasm0$1(ptr, len) {
    ptr = ptr >>> 0;
    return decodeText$1(ptr, len);
}

let cachedUint32ArrayMemory0$1 = null;
function getUint32ArrayMemory0$1() {
    if (cachedUint32ArrayMemory0$1 === null || cachedUint32ArrayMemory0$1.byteLength === 0) {
        cachedUint32ArrayMemory0$1 = new Uint32Array(wasm$1.memory.buffer);
    }
    return cachedUint32ArrayMemory0$1;
}

let cachedUint8ArrayMemory0$1 = null;
function getUint8ArrayMemory0$1() {
    if (cachedUint8ArrayMemory0$1 === null || cachedUint8ArrayMemory0$1.byteLength === 0) {
        cachedUint8ArrayMemory0$1 = new Uint8Array(wasm$1.memory.buffer);
    }
    return cachedUint8ArrayMemory0$1;
}

function passArray32ToWasm0$1(arg, malloc) {
    const ptr = malloc(arg.length * 4, 4) >>> 0;
    getUint32ArrayMemory0$1().set(arg, ptr / 4);
    WASM_VECTOR_LEN$1 = arg.length;
    return ptr;
}

function passArray8ToWasm0$1(arg, malloc) {
    const ptr = malloc(arg.length * 1, 1) >>> 0;
    getUint8ArrayMemory0$1().set(arg, ptr / 1);
    WASM_VECTOR_LEN$1 = arg.length;
    return ptr;
}

function passArrayF64ToWasm0$1(arg, malloc) {
    const ptr = malloc(arg.length * 8, 8) >>> 0;
    getFloat64ArrayMemory0$1().set(arg, ptr / 8);
    WASM_VECTOR_LEN$1 = arg.length;
    return ptr;
}

function passStringToWasm0$1(arg, malloc, realloc) {
    if (realloc === undefined) {
        const buf = cachedTextEncoder$1.encode(arg);
        const ptr = malloc(buf.length, 1) >>> 0;
        getUint8ArrayMemory0$1().subarray(ptr, ptr + buf.length).set(buf);
        WASM_VECTOR_LEN$1 = buf.length;
        return ptr;
    }

    let len = arg.length;
    let ptr = malloc(len, 1) >>> 0;

    const mem = getUint8ArrayMemory0$1();

    let offset = 0;

    for (; offset < len; offset++) {
        const code = arg.charCodeAt(offset);
        if (code > 0x7F) break;
        mem[ptr + offset] = code;
    }
    if (offset !== len) {
        if (offset !== 0) {
            arg = arg.slice(offset);
        }
        ptr = realloc(ptr, len, len = offset + arg.length * 3, 1) >>> 0;
        const view = getUint8ArrayMemory0$1().subarray(ptr + offset, ptr + len);
        const ret = cachedTextEncoder$1.encodeInto(arg, view);

        offset += ret.written;
        ptr = realloc(ptr, len, offset, 1) >>> 0;
    }

    WASM_VECTOR_LEN$1 = offset;
    return ptr;
}

function takeFromExternrefTable0$1(idx) {
    const value = wasm$1.__wbindgen_externrefs.get(idx);
    wasm$1.__externref_table_dealloc(idx);
    return value;
}

let cachedTextDecoder$1 = new TextDecoder('utf-8', { ignoreBOM: true, fatal: true });
cachedTextDecoder$1.decode();
const MAX_SAFARI_DECODE_BYTES$1 = 2146435072;
let numBytesDecoded$1 = 0;
function decodeText$1(ptr, len) {
    numBytesDecoded$1 += len;
    if (numBytesDecoded$1 >= MAX_SAFARI_DECODE_BYTES$1) {
        cachedTextDecoder$1 = new TextDecoder('utf-8', { ignoreBOM: true, fatal: true });
        cachedTextDecoder$1.decode();
        numBytesDecoded$1 = len;
    }
    return cachedTextDecoder$1.decode(getUint8ArrayMemory0$1().subarray(ptr, ptr + len));
}

const cachedTextEncoder$1 = new TextEncoder();

if (!('encodeInto' in cachedTextEncoder$1)) {
    cachedTextEncoder$1.encodeInto = function (arg, view) {
        const buf = cachedTextEncoder$1.encode(arg);
        view.set(buf);
        return {
            read: arg.length,
            written: buf.length
        };
    };
}

let WASM_VECTOR_LEN$1 = 0;

const WasmPolygonResultFinalization$1 = (typeof FinalizationRegistry === 'undefined')
    ? { register: () => {}, unregister: () => {} }
    : new FinalizationRegistry(ptr => wasm$1.__wbg_wasmpolygonresult_free(ptr >>> 0, 1));

let WasmPolygonResult$1 = class WasmPolygonResult {
    static __wrap(ptr) {
        ptr = ptr >>> 0;
        const obj = Object.create(WasmPolygonResult.prototype);
        obj.__wbg_ptr = ptr;
        WasmPolygonResultFinalization$1.register(obj, obj.__wbg_ptr, obj);
        return obj;
    }
    __destroy_into_raw() {
        const ptr = this.__wbg_ptr;
        this.__wbg_ptr = 0;
        WasmPolygonResultFinalization$1.unregister(this);
        return ptr;
    }
    free() {
        const ptr = this.__destroy_into_raw();
        wasm$1.__wbg_wasmpolygonresult_free(ptr, 0);
    }
    /**
     * @returns {number}
     */
    coords_len() {
        const ret = wasm$1.wasmpolygonresult_coords_len(this.__wbg_ptr);
        return ret >>> 0;
    }
    /**
     * @returns {number}
     */
    coords_ptr() {
        const ret = wasm$1.wasmpolygonresult_coords_ptr(this.__wbg_ptr);
        return ret >>> 0;
    }
    /**
     * @returns {number}
     */
    ring_offsets_len() {
        const ret = wasm$1.wasmpolygonresult_ring_offsets_len(this.__wbg_ptr);
        return ret >>> 0;
    }
    /**
     * @returns {number}
     */
    ring_offsets_ptr() {
        const ret = wasm$1.wasmpolygonresult_ring_offsets_ptr(this.__wbg_ptr);
        return ret >>> 0;
    }
    /**
     * @returns {number}
     */
    polygon_offsets_len() {
        const ret = wasm$1.wasmpolygonresult_polygon_offsets_len(this.__wbg_ptr);
        return ret >>> 0;
    }
    /**
     * @returns {number}
     */
    polygon_offsets_ptr() {
        const ret = wasm$1.wasmpolygonresult_polygon_offsets_ptr(this.__wbg_ptr);
        return ret >>> 0;
    }
    /**
     * @returns {number}
     */
    stride() {
        const ret = wasm$1.wasmpolygonresult_stride(this.__wbg_ptr);
        return ret;
    }
};
if (Symbol.dispose) WasmPolygonResult$1.prototype[Symbol.dispose] = WasmPolygonResult$1.prototype.free;

/**
 * @param {string} geojson_str
 * @returns {string}
 */
function polygonize$1(geojson_str) {
    let deferred3_0;
    let deferred3_1;
    try {
        const ptr0 = passStringToWasm0$1(geojson_str, wasm$1.__wbindgen_malloc, wasm$1.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN$1;
        const ret = wasm$1.polygonize(ptr0, len0);
        var ptr2 = ret[0];
        var len2 = ret[1];
        if (ret[3]) {
            ptr2 = 0; len2 = 0;
            throw takeFromExternrefTable0$1(ret[2]);
        }
        deferred3_0 = ptr2;
        deferred3_1 = len2;
        return getStringFromWasm0$1(ptr2, len2);
    } finally {
        wasm$1.__wbindgen_free(deferred3_0, deferred3_1, 1);
    }
}

/**
 * @param {Float64Array} coords
 * @param {Uint32Array} offsets
 * @param {number} stride
 * @param {boolean} node_input
 * @param {number} snap_grid_size
 * @returns {WasmPolygonResult}
 */
function polygonize_buffers$1(coords, offsets, stride, node_input, snap_grid_size) {
    const ptr0 = passArrayF64ToWasm0$1(coords, wasm$1.__wbindgen_malloc);
    const len0 = WASM_VECTOR_LEN$1;
    const ptr1 = passArray32ToWasm0$1(offsets, wasm$1.__wbindgen_malloc);
    const len1 = WASM_VECTOR_LEN$1;
    const ret = wasm$1.polygonize_buffers(ptr0, len0, ptr1, len1, stride, node_input, snap_grid_size);
    if (ret[2]) {
        throw takeFromExternrefTable0$1(ret[1]);
    }
    return WasmPolygonResult$1.__wrap(ret[0]);
}

/**
 * @param {Uint8Array} ipc_bytes
 * @param {boolean} node_input
 * @param {number} snap_grid_size
 * @param {boolean} extract_only_polygonal
 * @returns {Uint8Array}
 */
function polygonize_geoarrow$1(ipc_bytes, node_input, snap_grid_size, extract_only_polygonal) {
    const ptr0 = passArray8ToWasm0$1(ipc_bytes, wasm$1.__wbindgen_malloc);
    const len0 = WASM_VECTOR_LEN$1;
    const ret = wasm$1.polygonize_geoarrow(ptr0, len0, node_input, snap_grid_size, extract_only_polygonal);
    if (ret[3]) {
        throw takeFromExternrefTable0$1(ret[2]);
    }
    var v2 = getArrayU8FromWasm0$1(ret[0], ret[1]).slice();
    wasm$1.__wbindgen_free(ret[0], ret[1] * 1, 1);
    return v2;
}

const EXPECTED_RESPONSE_TYPES$1 = new Set(['basic', 'cors', 'default']);

async function __wbg_load$1(module, imports) {
    if (typeof Response === 'function' && module instanceof Response) {
        if (typeof WebAssembly.instantiateStreaming === 'function') {
            try {
                return await WebAssembly.instantiateStreaming(module, imports);
            } catch (e) {
                const validResponse = module.ok && EXPECTED_RESPONSE_TYPES$1.has(module.type);

                if (validResponse && module.headers.get('Content-Type') !== 'application/wasm') {
                    console.warn("`WebAssembly.instantiateStreaming` failed because your server does not serve Wasm with `application/wasm` MIME type. Falling back to `WebAssembly.instantiate` which is slower. Original error:\n", e);

                } else {
                    throw e;
                }
            }
        }

        const bytes = await module.arrayBuffer();
        return await WebAssembly.instantiate(bytes, imports);
    } else {
        const instance = await WebAssembly.instantiate(module, imports);

        if (instance instanceof WebAssembly.Instance) {
            return { instance, module };
        } else {
            return instance;
        }
    }
}

function __wbg_get_imports$1() {
    const imports = {};
    imports.wbg = {};
    imports.wbg.__wbg___wbindgen_throw_dd24417ed36fc46e = function(arg0, arg1) {
        throw new Error(getStringFromWasm0$1(arg0, arg1));
    };
    imports.wbg.__wbg_error_7534b8e9a36f1ab4 = function(arg0, arg1) {
        let deferred0_0;
        let deferred0_1;
        try {
            deferred0_0 = arg0;
            deferred0_1 = arg1;
            console.error(getStringFromWasm0$1(arg0, arg1));
        } finally {
            wasm$1.__wbindgen_free(deferred0_0, deferred0_1, 1);
        }
    };
    imports.wbg.__wbg_new_8a6f238a6ece86ea = function() {
        const ret = new Error();
        return ret;
    };
    imports.wbg.__wbg_stack_0ed75d68575b0f3c = function(arg0, arg1) {
        const ret = arg1.stack;
        const ptr1 = passStringToWasm0$1(ret, wasm$1.__wbindgen_malloc, wasm$1.__wbindgen_realloc);
        const len1 = WASM_VECTOR_LEN$1;
        getDataViewMemory0$1().setInt32(arg0 + 4 * 1, len1, true);
        getDataViewMemory0$1().setInt32(arg0 + 4 * 0, ptr1, true);
    };
    imports.wbg.__wbindgen_cast_2241b6af4c4b2941 = function(arg0, arg1) {
        // Cast intrinsic for `Ref(String) -> Externref`.
        const ret = getStringFromWasm0$1(arg0, arg1);
        return ret;
    };
    imports.wbg.__wbindgen_init_externref_table = function() {
        const table = wasm$1.__wbindgen_externrefs;
        const offset = table.grow(4);
        table.set(0, undefined);
        table.set(offset + 0, undefined);
        table.set(offset + 1, null);
        table.set(offset + 2, true);
        table.set(offset + 3, false);
    };

    return imports;
}

function __wbg_finalize_init$1(instance, module) {
    wasm$1 = instance.exports;
    __wbg_init$1.__wbindgen_wasm_module = module;
    cachedDataViewMemory0$1 = null;
    cachedFloat64ArrayMemory0$1 = null;
    cachedUint32ArrayMemory0$1 = null;
    cachedUint8ArrayMemory0$1 = null;


    wasm$1.__wbindgen_start();
    return wasm$1;
}

function initSync$1(module) {
    if (wasm$1 !== undefined) return wasm$1;


    if (typeof module !== 'undefined') {
        if (Object.getPrototypeOf(module) === Object.prototype) {
            ({module} = module);
        } else {
            console.warn('using deprecated parameters for `initSync()`; pass a single object instead');
        }
    }

    const imports = __wbg_get_imports$1();
    if (!(module instanceof WebAssembly.Module)) {
        module = new WebAssembly.Module(module);
    }
    const instance = new WebAssembly.Instance(module, imports);
    return __wbg_finalize_init$1(instance, module);
}

async function __wbg_init$1(module_or_path) {
    if (wasm$1 !== undefined) return wasm$1;


    if (typeof module_or_path !== 'undefined') {
        if (Object.getPrototypeOf(module_or_path) === Object.prototype) {
            ({module_or_path} = module_or_path);
        } else {
            console.warn('using deprecated parameters for the initialization function; pass a single object instead');
        }
    }

    if (typeof module_or_path === 'undefined') {
        module_or_path = new URL('geo_polygonize_bg.wasm', (typeof document === 'undefined' ? require('u' + 'rl').pathToFileURL(__filename).href : (_documentCurrentScript && _documentCurrentScript.tagName.toUpperCase() === 'SCRIPT' && _documentCurrentScript.src || new URL('index_slim.js', document.baseURI).href)));
    }
    const imports = __wbg_get_imports$1();

    if (typeof module_or_path === 'string' || (typeof Request === 'function' && module_or_path instanceof Request) || (typeof URL === 'function' && module_or_path instanceof URL)) {
        module_or_path = fetch(module_or_path);
    }

    const { instance, module } = await __wbg_load$1(await module_or_path, imports);

    return __wbg_finalize_init$1(instance, module);
}

var scalarExports = /*#__PURE__*/Object.freeze({
    __proto__: null,
    WasmPolygonResult: WasmPolygonResult$1,
    default: __wbg_init$1,
    initSync: initSync$1,
    polygonize: polygonize$1,
    polygonize_buffers: polygonize_buffers$1,
    polygonize_geoarrow: polygonize_geoarrow$1
});

let wasm;

function getArrayU8FromWasm0(ptr, len) {
    ptr = ptr >>> 0;
    return getUint8ArrayMemory0().subarray(ptr / 1, ptr / 1 + len);
}

let cachedDataViewMemory0 = null;
function getDataViewMemory0() {
    if (cachedDataViewMemory0 === null || cachedDataViewMemory0.buffer.detached === true || (cachedDataViewMemory0.buffer.detached === undefined && cachedDataViewMemory0.buffer !== wasm.memory.buffer)) {
        cachedDataViewMemory0 = new DataView(wasm.memory.buffer);
    }
    return cachedDataViewMemory0;
}

let cachedFloat64ArrayMemory0 = null;
function getFloat64ArrayMemory0() {
    if (cachedFloat64ArrayMemory0 === null || cachedFloat64ArrayMemory0.byteLength === 0) {
        cachedFloat64ArrayMemory0 = new Float64Array(wasm.memory.buffer);
    }
    return cachedFloat64ArrayMemory0;
}

function getStringFromWasm0(ptr, len) {
    ptr = ptr >>> 0;
    return decodeText(ptr, len);
}

let cachedUint32ArrayMemory0 = null;
function getUint32ArrayMemory0() {
    if (cachedUint32ArrayMemory0 === null || cachedUint32ArrayMemory0.byteLength === 0) {
        cachedUint32ArrayMemory0 = new Uint32Array(wasm.memory.buffer);
    }
    return cachedUint32ArrayMemory0;
}

let cachedUint8ArrayMemory0 = null;
function getUint8ArrayMemory0() {
    if (cachedUint8ArrayMemory0 === null || cachedUint8ArrayMemory0.byteLength === 0) {
        cachedUint8ArrayMemory0 = new Uint8Array(wasm.memory.buffer);
    }
    return cachedUint8ArrayMemory0;
}

function passArray32ToWasm0(arg, malloc) {
    const ptr = malloc(arg.length * 4, 4) >>> 0;
    getUint32ArrayMemory0().set(arg, ptr / 4);
    WASM_VECTOR_LEN = arg.length;
    return ptr;
}

function passArray8ToWasm0(arg, malloc) {
    const ptr = malloc(arg.length * 1, 1) >>> 0;
    getUint8ArrayMemory0().set(arg, ptr / 1);
    WASM_VECTOR_LEN = arg.length;
    return ptr;
}

function passArrayF64ToWasm0(arg, malloc) {
    const ptr = malloc(arg.length * 8, 8) >>> 0;
    getFloat64ArrayMemory0().set(arg, ptr / 8);
    WASM_VECTOR_LEN = arg.length;
    return ptr;
}

function passStringToWasm0(arg, malloc, realloc) {
    if (realloc === undefined) {
        const buf = cachedTextEncoder.encode(arg);
        const ptr = malloc(buf.length, 1) >>> 0;
        getUint8ArrayMemory0().subarray(ptr, ptr + buf.length).set(buf);
        WASM_VECTOR_LEN = buf.length;
        return ptr;
    }

    let len = arg.length;
    let ptr = malloc(len, 1) >>> 0;

    const mem = getUint8ArrayMemory0();

    let offset = 0;

    for (; offset < len; offset++) {
        const code = arg.charCodeAt(offset);
        if (code > 0x7F) break;
        mem[ptr + offset] = code;
    }
    if (offset !== len) {
        if (offset !== 0) {
            arg = arg.slice(offset);
        }
        ptr = realloc(ptr, len, len = offset + arg.length * 3, 1) >>> 0;
        const view = getUint8ArrayMemory0().subarray(ptr + offset, ptr + len);
        const ret = cachedTextEncoder.encodeInto(arg, view);

        offset += ret.written;
        ptr = realloc(ptr, len, offset, 1) >>> 0;
    }

    WASM_VECTOR_LEN = offset;
    return ptr;
}

function takeFromExternrefTable0(idx) {
    const value = wasm.__wbindgen_externrefs.get(idx);
    wasm.__externref_table_dealloc(idx);
    return value;
}

let cachedTextDecoder = new TextDecoder('utf-8', { ignoreBOM: true, fatal: true });
cachedTextDecoder.decode();
const MAX_SAFARI_DECODE_BYTES = 2146435072;
let numBytesDecoded = 0;
function decodeText(ptr, len) {
    numBytesDecoded += len;
    if (numBytesDecoded >= MAX_SAFARI_DECODE_BYTES) {
        cachedTextDecoder = new TextDecoder('utf-8', { ignoreBOM: true, fatal: true });
        cachedTextDecoder.decode();
        numBytesDecoded = len;
    }
    return cachedTextDecoder.decode(getUint8ArrayMemory0().subarray(ptr, ptr + len));
}

const cachedTextEncoder = new TextEncoder();

if (!('encodeInto' in cachedTextEncoder)) {
    cachedTextEncoder.encodeInto = function (arg, view) {
        const buf = cachedTextEncoder.encode(arg);
        view.set(buf);
        return {
            read: arg.length,
            written: buf.length
        };
    };
}

let WASM_VECTOR_LEN = 0;

const WasmPolygonResultFinalization = (typeof FinalizationRegistry === 'undefined')
    ? { register: () => {}, unregister: () => {} }
    : new FinalizationRegistry(ptr => wasm.__wbg_wasmpolygonresult_free(ptr >>> 0, 1));

class WasmPolygonResult {
    static __wrap(ptr) {
        ptr = ptr >>> 0;
        const obj = Object.create(WasmPolygonResult.prototype);
        obj.__wbg_ptr = ptr;
        WasmPolygonResultFinalization.register(obj, obj.__wbg_ptr, obj);
        return obj;
    }
    __destroy_into_raw() {
        const ptr = this.__wbg_ptr;
        this.__wbg_ptr = 0;
        WasmPolygonResultFinalization.unregister(this);
        return ptr;
    }
    free() {
        const ptr = this.__destroy_into_raw();
        wasm.__wbg_wasmpolygonresult_free(ptr, 0);
    }
    /**
     * @returns {number}
     */
    coords_len() {
        const ret = wasm.wasmpolygonresult_coords_len(this.__wbg_ptr);
        return ret >>> 0;
    }
    /**
     * @returns {number}
     */
    coords_ptr() {
        const ret = wasm.wasmpolygonresult_coords_ptr(this.__wbg_ptr);
        return ret >>> 0;
    }
    /**
     * @returns {number}
     */
    ring_offsets_len() {
        const ret = wasm.wasmpolygonresult_ring_offsets_len(this.__wbg_ptr);
        return ret >>> 0;
    }
    /**
     * @returns {number}
     */
    ring_offsets_ptr() {
        const ret = wasm.wasmpolygonresult_ring_offsets_ptr(this.__wbg_ptr);
        return ret >>> 0;
    }
    /**
     * @returns {number}
     */
    polygon_offsets_len() {
        const ret = wasm.wasmpolygonresult_polygon_offsets_len(this.__wbg_ptr);
        return ret >>> 0;
    }
    /**
     * @returns {number}
     */
    polygon_offsets_ptr() {
        const ret = wasm.wasmpolygonresult_polygon_offsets_ptr(this.__wbg_ptr);
        return ret >>> 0;
    }
    /**
     * @returns {number}
     */
    stride() {
        const ret = wasm.wasmpolygonresult_stride(this.__wbg_ptr);
        return ret;
    }
}
if (Symbol.dispose) WasmPolygonResult.prototype[Symbol.dispose] = WasmPolygonResult.prototype.free;

/**
 * @param {string} geojson_str
 * @returns {string}
 */
function polygonize(geojson_str) {
    let deferred3_0;
    let deferred3_1;
    try {
        const ptr0 = passStringToWasm0(geojson_str, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.polygonize(ptr0, len0);
        var ptr2 = ret[0];
        var len2 = ret[1];
        if (ret[3]) {
            ptr2 = 0; len2 = 0;
            throw takeFromExternrefTable0(ret[2]);
        }
        deferred3_0 = ptr2;
        deferred3_1 = len2;
        return getStringFromWasm0(ptr2, len2);
    } finally {
        wasm.__wbindgen_free(deferred3_0, deferred3_1, 1);
    }
}

/**
 * @param {Float64Array} coords
 * @param {Uint32Array} offsets
 * @param {number} stride
 * @param {boolean} node_input
 * @param {number} snap_grid_size
 * @returns {WasmPolygonResult}
 */
function polygonize_buffers(coords, offsets, stride, node_input, snap_grid_size) {
    const ptr0 = passArrayF64ToWasm0(coords, wasm.__wbindgen_malloc);
    const len0 = WASM_VECTOR_LEN;
    const ptr1 = passArray32ToWasm0(offsets, wasm.__wbindgen_malloc);
    const len1 = WASM_VECTOR_LEN;
    const ret = wasm.polygonize_buffers(ptr0, len0, ptr1, len1, stride, node_input, snap_grid_size);
    if (ret[2]) {
        throw takeFromExternrefTable0(ret[1]);
    }
    return WasmPolygonResult.__wrap(ret[0]);
}

/**
 * @param {Uint8Array} ipc_bytes
 * @param {boolean} node_input
 * @param {number} snap_grid_size
 * @param {boolean} extract_only_polygonal
 * @returns {Uint8Array}
 */
function polygonize_geoarrow(ipc_bytes, node_input, snap_grid_size, extract_only_polygonal) {
    const ptr0 = passArray8ToWasm0(ipc_bytes, wasm.__wbindgen_malloc);
    const len0 = WASM_VECTOR_LEN;
    const ret = wasm.polygonize_geoarrow(ptr0, len0, node_input, snap_grid_size, extract_only_polygonal);
    if (ret[3]) {
        throw takeFromExternrefTable0(ret[2]);
    }
    var v2 = getArrayU8FromWasm0(ret[0], ret[1]).slice();
    wasm.__wbindgen_free(ret[0], ret[1] * 1, 1);
    return v2;
}

const EXPECTED_RESPONSE_TYPES = new Set(['basic', 'cors', 'default']);

async function __wbg_load(module, imports) {
    if (typeof Response === 'function' && module instanceof Response) {
        if (typeof WebAssembly.instantiateStreaming === 'function') {
            try {
                return await WebAssembly.instantiateStreaming(module, imports);
            } catch (e) {
                const validResponse = module.ok && EXPECTED_RESPONSE_TYPES.has(module.type);

                if (validResponse && module.headers.get('Content-Type') !== 'application/wasm') {
                    console.warn("`WebAssembly.instantiateStreaming` failed because your server does not serve Wasm with `application/wasm` MIME type. Falling back to `WebAssembly.instantiate` which is slower. Original error:\n", e);

                } else {
                    throw e;
                }
            }
        }

        const bytes = await module.arrayBuffer();
        return await WebAssembly.instantiate(bytes, imports);
    } else {
        const instance = await WebAssembly.instantiate(module, imports);

        if (instance instanceof WebAssembly.Instance) {
            return { instance, module };
        } else {
            return instance;
        }
    }
}

function __wbg_get_imports() {
    const imports = {};
    imports.wbg = {};
    imports.wbg.__wbg___wbindgen_throw_dd24417ed36fc46e = function(arg0, arg1) {
        throw new Error(getStringFromWasm0(arg0, arg1));
    };
    imports.wbg.__wbg_error_7534b8e9a36f1ab4 = function(arg0, arg1) {
        let deferred0_0;
        let deferred0_1;
        try {
            deferred0_0 = arg0;
            deferred0_1 = arg1;
            console.error(getStringFromWasm0(arg0, arg1));
        } finally {
            wasm.__wbindgen_free(deferred0_0, deferred0_1, 1);
        }
    };
    imports.wbg.__wbg_new_8a6f238a6ece86ea = function() {
        const ret = new Error();
        return ret;
    };
    imports.wbg.__wbg_stack_0ed75d68575b0f3c = function(arg0, arg1) {
        const ret = arg1.stack;
        const ptr1 = passStringToWasm0(ret, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len1 = WASM_VECTOR_LEN;
        getDataViewMemory0().setInt32(arg0 + 4 * 1, len1, true);
        getDataViewMemory0().setInt32(arg0 + 4 * 0, ptr1, true);
    };
    imports.wbg.__wbindgen_cast_2241b6af4c4b2941 = function(arg0, arg1) {
        // Cast intrinsic for `Ref(String) -> Externref`.
        const ret = getStringFromWasm0(arg0, arg1);
        return ret;
    };
    imports.wbg.__wbindgen_init_externref_table = function() {
        const table = wasm.__wbindgen_externrefs;
        const offset = table.grow(4);
        table.set(0, undefined);
        table.set(offset + 0, undefined);
        table.set(offset + 1, null);
        table.set(offset + 2, true);
        table.set(offset + 3, false);
    };

    return imports;
}

function __wbg_finalize_init(instance, module) {
    wasm = instance.exports;
    __wbg_init.__wbindgen_wasm_module = module;
    cachedDataViewMemory0 = null;
    cachedFloat64ArrayMemory0 = null;
    cachedUint32ArrayMemory0 = null;
    cachedUint8ArrayMemory0 = null;


    wasm.__wbindgen_start();
    return wasm;
}

function initSync(module) {
    if (wasm !== undefined) return wasm;


    if (typeof module !== 'undefined') {
        if (Object.getPrototypeOf(module) === Object.prototype) {
            ({module} = module);
        } else {
            console.warn('using deprecated parameters for `initSync()`; pass a single object instead');
        }
    }

    const imports = __wbg_get_imports();
    if (!(module instanceof WebAssembly.Module)) {
        module = new WebAssembly.Module(module);
    }
    const instance = new WebAssembly.Instance(module, imports);
    return __wbg_finalize_init(instance, module);
}

async function __wbg_init(module_or_path) {
    if (wasm !== undefined) return wasm;


    if (typeof module_or_path !== 'undefined') {
        if (Object.getPrototypeOf(module_or_path) === Object.prototype) {
            ({module_or_path} = module_or_path);
        } else {
            console.warn('using deprecated parameters for the initialization function; pass a single object instead');
        }
    }

    if (typeof module_or_path === 'undefined') {
        module_or_path = new URL('geo_polygonize_bg.wasm', (typeof document === 'undefined' ? require('u' + 'rl').pathToFileURL(__filename).href : (_documentCurrentScript && _documentCurrentScript.tagName.toUpperCase() === 'SCRIPT' && _documentCurrentScript.src || new URL('index_slim.js', document.baseURI).href)));
    }
    const imports = __wbg_get_imports();

    if (typeof module_or_path === 'string' || (typeof Request === 'function' && module_or_path instanceof Request) || (typeof URL === 'function' && module_or_path instanceof URL)) {
        module_or_path = fetch(module_or_path);
    }

    const { instance, module } = await __wbg_load(await module_or_path, imports);

    return __wbg_finalize_init(instance, module);
}

var simdExports = /*#__PURE__*/Object.freeze({
    __proto__: null,
    WasmPolygonResult: WasmPolygonResult,
    default: __wbg_init,
    initSync: initSync,
    polygonize: polygonize,
    polygonize_buffers: polygonize_buffers,
    polygonize_geoarrow: polygonize_geoarrow
});

// We provide a helper to choose based on feature detection if the user wants to use it
let isSimdSupported;
const SIMD_TEST_BYTES = new Uint8Array([0, 97, 115, 109, 1, 0, 0, 0, 1, 5, 1, 96, 0, 1, 123, 3, 2, 1, 0, 10, 10, 1, 8, 0, 65, 0, 253, 15, 253, 98, 11]);
async function initBest(scalarModule, simdModule) {
    if (isSimdSupported === undefined) {
        try {
            isSimdSupported = WebAssembly.validate(SIMD_TEST_BYTES);
        }
        catch (e) {
            isSimdSupported = false;
        }
    }
    if (isSimdSupported && simdModule) {
        await __wbg_init(simdModule);
        return simdExports;
    }
    else {
        await __wbg_init$1(scalarModule);
        return scalarExports;
    }
}

exports.WasmPolygonResult = WasmPolygonResult$1;
exports.initBest = initBest;
exports.initSync = initSync$1;
exports.polygonize = polygonize$1;
exports.polygonize_buffers = polygonize_buffers$1;
exports.polygonize_geoarrow = polygonize_geoarrow$1;
