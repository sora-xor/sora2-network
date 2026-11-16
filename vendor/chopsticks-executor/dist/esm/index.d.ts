/* tslint:disable */
/* eslint-disable */

type HexString = `0x${string}`;
export interface JsCallback {
    getStorage: (key: HexString) => Promise<string | undefined>
    getNextKey: (prefix: HexString, key: HexString) => Promise<string | undefined>
    offchainGetStorage: (key: HexString) => Promise<string | undefined>
    offchainTimestamp: () => Promise<number>
    offchainRandomSeed: () => Promise<HexString>
    offchainSubmitTransaction: (tx: HexString) => Promise<boolean>
}



export function calculate_state_root(entries: any, trie_version: any): Promise<any>;

export function create_proof(nodes: any, updates: any): Promise<any>;

export function decode_proof(trie_root_hash: any, nodes: any): Promise<any>;

export function get_runtime_version(code: any): Promise<any>;

export function run_task(task: any, js: JsCallback): Promise<any>;

export function testing(js: JsCallback, key: any): Promise<any>;

export type InitInput = RequestInfo | URL | Response | BufferSource | WebAssembly.Module;

export interface InitOutput {
    readonly memory: WebAssembly.Memory;
    readonly calculate_state_root: (a: any, b: any) => any;
    readonly create_proof: (a: any, b: any) => any;
    readonly decode_proof: (a: any, b: any) => any;
    readonly get_runtime_version: (a: any) => any;
    readonly run_task: (a: any, b: any) => any;
    readonly testing: (a: any, b: any) => any;
    readonly wasm_bindgen_fd2640d4c3f72623___convert__closures_____invoke___js_sys_59ee1e5504fa6bb2___Function_fn_wasm_bindgen_fd2640d4c3f72623___JsValue_____wasm_bindgen_fd2640d4c3f72623___sys__Undefined___js_sys_59ee1e5504fa6bb2___Function_fn_wasm_bindgen_fd2640d4c3f72623___JsValue_____wasm_bindgen_fd2640d4c3f72623___sys__Undefined_______true_: (a: number, b: number, c: any, d: any) => void;
    readonly wasm_bindgen_fd2640d4c3f72623___convert__closures_____invoke___wasm_bindgen_fd2640d4c3f72623___JsValue__core_cb393ef8445f6999___result__Result_____wasm_bindgen_fd2640d4c3f72623___JsError___true_: (a: number, b: number, c: any) => [number, number];
    readonly __wbindgen_malloc: (a: number, b: number) => number;
    readonly __wbindgen_realloc: (a: number, b: number, c: number, d: number) => number;
    readonly __wbindgen_exn_store: (a: number) => void;
    readonly __externref_table_alloc: () => number;
    readonly __wbindgen_externrefs: WebAssembly.Table;
    readonly __wbindgen_free: (a: number, b: number, c: number) => void;
    readonly __wbindgen_destroy_closure: (a: number, b: number) => void;
    readonly __externref_table_dealloc: (a: number) => void;
    readonly __wbindgen_start: () => void;
}

export type SyncInitInput = BufferSource | WebAssembly.Module;

/**
 * Instantiates the given `module`, which can either be bytes or
 * a precompiled `WebAssembly.Module`.
 *
 * @param {{ module: SyncInitInput }} module - Passing `SyncInitInput` directly is deprecated.
 *
 * @returns {InitOutput}
 */
export function initSync(module: { module: SyncInitInput } | SyncInitInput): InitOutput;

/**
 * If `module_or_path` is {RequestInfo} or {URL}, makes a request and
 * for everything else, calls `WebAssembly.instantiate` directly.
 *
 * @param {{ module_or_path: InitInput | Promise<InitInput> }} module_or_path - Passing `InitInput` directly is deprecated.
 *
 * @returns {Promise<InitOutput>}
 */
export default function __wbg_init (module_or_path?: { module_or_path: InitInput | Promise<InitInput> } | InitInput | Promise<InitInput>): Promise<InitOutput>;
