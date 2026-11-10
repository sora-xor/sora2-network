(function (global, factory) {
    typeof exports === 'object' && typeof module !== 'undefined' ? factory(exports, require('@polkadot/util')) :
    typeof define === 'function' && define.amd ? define(['exports', '@polkadot/util'], factory) :
    (global = typeof globalThis !== 'undefined' ? globalThis : global || self, factory(global.polkadotUtilCrypto = {}, global.polkadotUtil));
})(this, (function (exports, util) { 'use strict';

    const global = typeof globalThis !== "undefined" ? globalThis : typeof self !== "undefined" ? self : window;

    var _documentCurrentScript = typeof document !== 'undefined' ? document.currentScript : null;
    const packageInfo$2 = { name: '@polkadot/x-global', path: (({ url: (typeof document === 'undefined' && typeof location === 'undefined' ? require('u' + 'rl').pathToFileURL(__filename).href : typeof document === 'undefined' ? location.href : (_documentCurrentScript && _documentCurrentScript.src || new URL('bundle-polkadot-util-crypto.js', document.baseURI).href)) }) && (typeof document === 'undefined' && typeof location === 'undefined' ? require('u' + 'rl').pathToFileURL(__filename).href : typeof document === 'undefined' ? location.href : (_documentCurrentScript && _documentCurrentScript.src || new URL('bundle-polkadot-util-crypto.js', document.baseURI).href))) ? new URL((typeof document === 'undefined' && typeof location === 'undefined' ? require('u' + 'rl').pathToFileURL(__filename).href : typeof document === 'undefined' ? location.href : (_documentCurrentScript && _documentCurrentScript.src || new URL('bundle-polkadot-util-crypto.js', document.baseURI).href))).pathname.substring(0, new URL((typeof document === 'undefined' && typeof location === 'undefined' ? require('u' + 'rl').pathToFileURL(__filename).href : typeof document === 'undefined' ? location.href : (_documentCurrentScript && _documentCurrentScript.src || new URL('bundle-polkadot-util-crypto.js', document.baseURI).href))).pathname.lastIndexOf('/') + 1) : 'auto', type: 'esm', version: '14.0.3' };

    function evaluateThis(fn) {
        return fn('return this');
    }
    const xglobal =  (typeof globalThis !== 'undefined'
        ? globalThis
        : typeof global !== 'undefined'
            ? global
            : typeof self !== 'undefined'
                ? self
                : typeof window !== 'undefined'
                    ? window
                    : evaluateThis(Function));
    function extractGlobal(name, fallback) {
        return typeof xglobal[name] === 'undefined'
            ? fallback
            : xglobal[name];
    }
    function exposeGlobal(name, fallback) {
        if (typeof xglobal[name] === 'undefined') {
            xglobal[name] = fallback;
        }
    }

    const build = /*#__PURE__*/Object.freeze({
        __proto__: null,
        exposeGlobal: exposeGlobal,
        extractGlobal: extractGlobal,
        packageInfo: packageInfo$2,
        xglobal: xglobal
    });

    function invalidFallback() {
        return Number.NaN;
    }
    const BigInt$1 =  extractGlobal('BigInt', invalidFallback);

    exposeGlobal('BigInt', BigInt$1);

    function getDefaultExportFromCjs (x) {
    	return x && x.__esModule && Object.prototype.hasOwnProperty.call(x, 'default') ? x['default'] : x;
    }

    function getAugmentedNamespace(n) {
      if (n.__esModule) return n;
      var f = n.default;
    	if (typeof f == "function") {
    		var a = function a () {
    			if (this instanceof a) {
            return Reflect.construct(f, arguments, this.constructor);
    			}
    			return f.apply(this, arguments);
    		};
    		a.prototype = f.prototype;
      } else a = {};
      Object.defineProperty(a, '__esModule', {value: true});
    	Object.keys(n).forEach(function (k) {
    		var d = Object.getOwnPropertyDescriptor(n, k);
    		Object.defineProperty(a, k, d.get ? d : {
    			enumerable: true,
    			get: function () {
    				return n[k];
    			}
    		});
    	});
    	return a;
    }

    var browser = {};

    const require$$0 = /*@__PURE__*/getAugmentedNamespace(build);

    var packageInfo$1 = {};

    Object.defineProperty(packageInfo$1, "__esModule", { value: true });
    packageInfo$1.packageInfo = void 0;
    packageInfo$1.packageInfo = { name: '@polkadot/x-randomvalues', path: typeof __dirname === 'string' ? __dirname : 'auto', type: 'cjs', version: '14.0.3' };

    (function (exports) {
    	Object.defineProperty(exports, "__esModule", { value: true });
    	exports.crypto = exports.packageInfo = void 0;
    	exports.getRandomValues = getRandomValues;
    	const x_global_1 = require$$0;
    	var packageInfo_js_1 = packageInfo$1;
    	Object.defineProperty(exports, "packageInfo", { enumerable: true, get: function () { return packageInfo_js_1.packageInfo; } });
    	exports.crypto = x_global_1.xglobal.crypto;
    	function getRandomValues(arr) {
    	    return exports.crypto.getRandomValues(arr);
    	}
    } (browser));
    getDefaultExportFromCjs(browser);

    const DEFAULT_CRYPTO = { getRandomValues: browser.getRandomValues };
    const DEFAULT_SELF = { crypto: DEFAULT_CRYPTO };
    class Wbg {
        #bridge;
        constructor(bridge) {
            this.#bridge = bridge;
        }
        abort = () => {
            throw new Error('abort');
        };
        __wbindgen_is_undefined = (idx) => {
            return this.#bridge.getObject(idx) === undefined;
        };
        __wbindgen_throw = (ptr, len) => {
            throw new Error(this.#bridge.getString(ptr, len));
        };
        __wbg_self_1b7a39e3a92c949c = () => {
            return this.#bridge.addObject(DEFAULT_SELF);
        };
        __wbg_require_604837428532a733 = (ptr, len) => {
            throw new Error(`Unable to require ${this.#bridge.getString(ptr, len)}`);
        };
        __wbg_crypto_968f1772287e2df0 = (_idx) => {
            return this.#bridge.addObject(DEFAULT_CRYPTO);
        };
        __wbg_getRandomValues_a3d34b4fee3c2869 = (_idx) => {
            return this.#bridge.addObject(DEFAULT_CRYPTO.getRandomValues);
        };
        __wbg_getRandomValues_f5e14ab7ac8e995d = (_arg0, ptr, len) => {
            DEFAULT_CRYPTO.getRandomValues(this.#bridge.getU8a(ptr, len));
        };
        __wbg_randomFillSync_d5bd2d655fdf256a = (_idx, _ptr, _len) => {
            throw new Error('randomFillsync is not available');
        };
        __wbindgen_object_drop_ref = (idx) => {
            this.#bridge.takeObject(idx);
        };
    }

    class Bridge {
        #createWasm;
        #heap;
        #wbg;
        #cachegetInt32;
        #cachegetUint8;
        #heapNext;
        #wasm;
        #wasmError;
        #wasmPromise;
        #type;
        constructor(createWasm) {
            this.#createWasm = createWasm;
            this.#cachegetInt32 = null;
            this.#cachegetUint8 = null;
            this.#heap = new Array(32)
                .fill(undefined)
                .concat(undefined, null, true, false);
            this.#heapNext = this.#heap.length;
            this.#type = 'none';
            this.#wasm = null;
            this.#wasmError = null;
            this.#wasmPromise = null;
            this.#wbg = { ...new Wbg(this) };
        }
        get error() {
            return this.#wasmError;
        }
        get type() {
            return this.#type;
        }
        get wasm() {
            return this.#wasm;
        }
        async init(createWasm) {
            if (!this.#wasmPromise || createWasm) {
                this.#wasmPromise = (createWasm || this.#createWasm)(this.#wbg);
            }
            const { error, type, wasm } = await this.#wasmPromise;
            this.#type = type;
            this.#wasm = wasm;
            this.#wasmError = error;
            return this.#wasm;
        }
        getObject(idx) {
            return this.#heap[idx];
        }
        dropObject(idx) {
            if (idx < 36) {
                return;
            }
            this.#heap[idx] = this.#heapNext;
            this.#heapNext = idx;
        }
        takeObject(idx) {
            const ret = this.getObject(idx);
            this.dropObject(idx);
            return ret;
        }
        addObject(obj) {
            if (this.#heapNext === this.#heap.length) {
                this.#heap.push(this.#heap.length + 1);
            }
            const idx = this.#heapNext;
            this.#heapNext = this.#heap[idx];
            this.#heap[idx] = obj;
            return idx;
        }
        getInt32() {
            if (this.#cachegetInt32 === null || this.#cachegetInt32.buffer !== this.#wasm.memory.buffer) {
                this.#cachegetInt32 = new Int32Array(this.#wasm.memory.buffer);
            }
            return this.#cachegetInt32;
        }
        getUint8() {
            if (this.#cachegetUint8 === null || this.#cachegetUint8.buffer !== this.#wasm.memory.buffer) {
                this.#cachegetUint8 = new Uint8Array(this.#wasm.memory.buffer);
            }
            return this.#cachegetUint8;
        }
        getU8a(ptr, len) {
            return this.getUint8().subarray(ptr / 1, ptr / 1 + len);
        }
        getString(ptr, len) {
            return util.u8aToString(this.getU8a(ptr, len));
        }
        allocU8a(arg) {
            const ptr = this.#wasm.__wbindgen_malloc(arg.length * 1);
            this.getUint8().set(arg, ptr / 1);
            return [ptr, arg.length];
        }
        allocString(arg) {
            return this.allocU8a(util.stringToU8a(arg));
        }
        resultU8a() {
            const r0 = this.getInt32()[8 / 4 + 0];
            const r1 = this.getInt32()[8 / 4 + 1];
            const ret = this.getU8a(r0, r1).slice();
            this.#wasm.__wbindgen_free(r0, r1 * 1);
            return ret;
        }
        resultString() {
            return util.u8aToString(this.resultU8a());
        }
    }

    function createWasmFn(root, wasmBytes, asmFn) {
        return async (wbg) => {
            const result = {
                error: null,
                type: 'none',
                wasm: null
            };
            try {
                if (!wasmBytes?.length) {
                    throw new Error('No WebAssembly provided for initialization');
                }
                else if (typeof WebAssembly !== 'object' || typeof WebAssembly.instantiate !== 'function') {
                    throw new Error('WebAssembly is not available in your environment');
                }
                const source = await WebAssembly.instantiate(wasmBytes, { wbg });
                result.wasm = source.instance.exports;
                result.type = 'wasm';
            }
            catch (error) {
                if (typeof asmFn === 'function') {
                    result.wasm = asmFn(wbg);
                    result.type = 'asm';
                }
                else {
                    result.error = `FATAL: Unable to initialize @polkadot/wasm-${root}:: ${error.message}`;
                    console.error(result.error);
                }
            }
            return result;
        };
    }

    const CHR = 'ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/';
    const map = new Array(256);
    for (let i = 0, count = CHR.length; i < count; i++) {
        map[CHR.charCodeAt(i)] = i;
    }
    function base64Decode$1(data, out) {
        let byte = 0;
        let bits = 0;
        let pos = -1;
        for (let i = 0, last = out.length - 1; pos !== last; i++) {
            byte = (byte << 6) | map[data.charCodeAt(i)];
            if ((bits += 6) >= 8) {
                out[++pos] = (byte >>> (bits -= 8)) & 0xff;
            }
        }
        return out;
    }

    const u8 = Uint8Array, u16 = Uint16Array, u32$2 = Uint32Array;
    const clim = new u8([16, 17, 18, 0, 8, 7, 9, 6, 10, 5, 11, 4, 12, 3, 13, 2, 14, 1, 15]);
    const fleb = new u8([0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 2, 2, 2, 2, 3, 3, 3, 3, 4, 4, 4, 4, 5, 5, 5, 5, 0,  0, 0,  0]);
    const fdeb = new u8([0, 0, 0, 0, 1, 1, 2, 2, 3, 3, 4, 4, 5, 5, 6, 6, 7, 7, 8, 8, 9, 9, 10, 10, 11, 11, 12, 12, 13, 13,  0, 0]);
    const freb = (eb, start) => {
        const b = new u16(31);
        for (let i = 0; i < 31; ++i) {
            b[i] = start += 1 << eb[i - 1];
        }
        const r = new u32$2(b[30]);
        for (let i = 1; i < 30; ++i) {
            for (let j = b[i]; j < b[i + 1]; ++j) {
                r[j] = ((j - b[i]) << 5) | i;
            }
        }
        return [b, r];
    };
    const [fl, revfl] = freb(fleb, 2);
    fl[28] = 258, revfl[258] = 28;
    const [fd] = freb(fdeb, 0);
    const rev = new u16(32768);
    for (let i = 0; i < 32768; ++i) {
        let x = ((i & 0xAAAA) >>> 1) | ((i & 0x5555) << 1);
        x = ((x & 0xCCCC) >>> 2) | ((x & 0x3333) << 2);
        x = ((x & 0xF0F0) >>> 4) | ((x & 0x0F0F) << 4);
        rev[i] = (((x & 0xFF00) >>> 8) | ((x & 0x00FF) << 8)) >>> 1;
    }
    const hMap = ((cd, mb, r) => {
        const s = cd.length;
        let i = 0;
        const l = new u16(mb);
        for (; i < s; ++i) {
            if (cd[i])
                ++l[cd[i] - 1];
        }
        const le = new u16(mb);
        for (i = 1; i < mb; ++i) {
            le[i] = (le[i - 1] + l[i - 1]) << 1;
        }
        let co;
        if (r) {
            co = new u16(1 << mb);
            const rvb = 15 - mb;
            for (i = 0; i < s; ++i) {
                if (cd[i]) {
                    const sv = (i << 4) | cd[i];
                    const r = mb - cd[i];
                    let v = le[cd[i] - 1]++ << r;
                    for (const m = v | ((1 << r) - 1); v <= m; ++v) {
                        co[rev[v] >> rvb] = sv;
                    }
                }
            }
        }
        else {
            co = new u16(s);
            for (i = 0; i < s; ++i) {
                if (cd[i]) {
                    co[i] = rev[le[cd[i] - 1]++] >> (15 - cd[i]);
                }
            }
        }
        return co;
    });
    const flt = new u8(288);
    for (let i = 0; i < 144; ++i)
        flt[i] = 8;
    for (let i = 144; i < 256; ++i)
        flt[i] = 9;
    for (let i = 256; i < 280; ++i)
        flt[i] = 7;
    for (let i = 280; i < 288; ++i)
        flt[i] = 8;
    const fdt = new u8(32);
    for (let i = 0; i < 32; ++i)
        fdt[i] = 5;
    const flrm = hMap(flt, 9, 1);
    const fdrm = hMap(fdt, 5, 1);
    const bits = (d, p, m) => {
        const o = p >>> 3;
        return ((d[o] | (d[o + 1] << 8)) >>> (p & 7)) & m;
    };
    const bits16 = (d, p) => {
        const o = p >>> 3;
        return ((d[o] | (d[o + 1] << 8) | (d[o + 2] << 16)) >>> (p & 7));
    };
    const shft = (p) => (p >>> 3) + (p & 7 && 1);
    const slc = (v, s, e) => {
        if (s == null || s < 0)
            s = 0;
        if (e == null || e > v.length)
            e = v.length;
        const n = new (v instanceof u16 ? u16 : v instanceof u32$2 ? u32$2 : u8)(e - s);
        n.set(v.subarray(s, e));
        return n;
    };
    const max = (a) => {
        let m = a[0];
        for (let i = 1, count = a.length; i < count; ++i) {
            if (a[i] > m)
                m = a[i];
        }
        return m;
    };
    const inflt = (dat, buf, st) => {
        const noSt = !st || st.i;
        if (!st)
            st = {};
        const sl = dat.length;
        const noBuf = !buf || !noSt;
        if (!buf)
            buf = new u8(sl * 3);
        const cbuf = (l) => {
            let bl = buf.length;
            if (l > bl) {
                const nbuf = new u8(Math.max(bl << 1, l));
                nbuf.set(buf);
                buf = nbuf;
            }
        };
        let final = st.f || 0, pos = st.p || 0, bt = st.b || 0, lm = st.l, dm = st.d, lbt = st.m, dbt = st.n;
        if (final && !lm)
            return buf;
        const tbts = sl << 3;
        do {
            if (!lm) {
                st.f = final = bits(dat, pos, 1);
                const type = bits(dat, pos + 1, 3);
                pos += 3;
                if (!type) {
                    const s = shft(pos) + 4, l = dat[s - 4] | (dat[s - 3] << 8), t = s + l;
                    if (t > sl) {
                        if (noSt)
                            throw 'unexpected EOF';
                        break;
                    }
                    if (noBuf)
                        cbuf(bt + l);
                    buf.set(dat.subarray(s, t), bt);
                    st.b = bt += l, st.p = pos = t << 3;
                    continue;
                }
                else if (type == 1)
                    lm = flrm, dm = fdrm, lbt = 9, dbt = 5;
                else if (type == 2) {
                    const hLit = bits(dat, pos, 31) + 257, hcLen = bits(dat, pos + 10, 15) + 4;
                    const tl = hLit + bits(dat, pos + 5, 31) + 1;
                    pos += 14;
                    const ldt = new u8(tl);
                    const clt = new u8(19);
                    for (let i = 0; i < hcLen; ++i) {
                        clt[clim[i]] = bits(dat, pos + i * 3, 7);
                    }
                    pos += hcLen * 3;
                    const clb = max(clt), clbmsk = (1 << clb) - 1;
                    if (!noSt && pos + tl * (clb + 7) > tbts)
                        break;
                    const clm = hMap(clt, clb, 1);
                    for (let i = 0; i < tl;) {
                        const r = clm[bits(dat, pos, clbmsk)];
                        pos += r & 15;
                        const s = r >>> 4;
                        if (s < 16) {
                            ldt[i++] = s;
                        }
                        else {
                            let c = 0, n = 0;
                            if (s == 16)
                                n = 3 + bits(dat, pos, 3), pos += 2, c = ldt[i - 1];
                            else if (s == 17)
                                n = 3 + bits(dat, pos, 7), pos += 3;
                            else if (s == 18)
                                n = 11 + bits(dat, pos, 127), pos += 7;
                            while (n--)
                                ldt[i++] = c;
                        }
                    }
                    const lt = ldt.subarray(0, hLit), dt = ldt.subarray(hLit);
                    lbt = max(lt);
                    dbt = max(dt);
                    lm = hMap(lt, lbt, 1);
                    dm = hMap(dt, dbt, 1);
                }
                else
                    throw 'invalid block type';
                if (pos > tbts)
                    throw 'unexpected EOF';
            }
            if (noBuf)
                cbuf(bt + 131072);
            const lms = (1 << lbt) - 1, dms = (1 << dbt) - 1;
            const mxa = lbt + dbt + 18;
            while (noSt || pos + mxa < tbts) {
                const c = lm[bits16(dat, pos) & lms], sym = c >>> 4;
                pos += c & 15;
                if (pos > tbts)
                    throw 'unexpected EOF';
                if (!c)
                    throw 'invalid length/literal';
                if (sym < 256)
                    buf[bt++] = sym;
                else if (sym == 256) {
                    lm = undefined;
                    break;
                }
                else {
                    let add = sym - 254;
                    if (sym > 264) {
                        const i = sym - 257, b = fleb[i];
                        add = bits(dat, pos, (1 << b) - 1) + fl[i];
                        pos += b;
                    }
                    const d = dm[bits16(dat, pos) & dms], dsym = d >>> 4;
                    if (!d)
                        throw 'invalid distance';
                    pos += d & 15;
                    let dt = fd[dsym];
                    if (dsym > 3) {
                        const b = fdeb[dsym];
                        dt += bits16(dat, pos) & ((1 << b) - 1), pos += b;
                    }
                    if (pos > tbts)
                        throw 'unexpected EOF';
                    if (noBuf)
                        cbuf(bt + 131072);
                    const end = bt + add;
                    for (; bt < end; bt += 4) {
                        buf[bt] = buf[bt - dt];
                        buf[bt + 1] = buf[bt + 1 - dt];
                        buf[bt + 2] = buf[bt + 2 - dt];
                        buf[bt + 3] = buf[bt + 3 - dt];
                    }
                    bt = end;
                }
            }
            st.l = lm, st.p = pos, st.b = bt;
            if (lm)
                final = 1, st.m = lbt, st.d = dm, st.n = dbt;
        } while (!final);
        return bt == buf.length ? buf : slc(buf, 0, bt);
    };
    const zlv = (d) => {
        if ((d[0] & 15) != 8 || (d[0] >>> 4) > 7 || ((d[0] << 8 | d[1]) % 31))
            throw 'invalid zlib data';
        if (d[1] & 32)
            throw 'invalid zlib data: preset dictionaries not supported';
    };
    function unzlibSync(data, out) {
        return inflt((zlv(data), data.subarray(2, -4)), out);
    }

    var lenIn = 168782;
    var lenOut = 335277;
    var bytes_1 = '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';

    const wasmBytes =  unzlibSync(base64Decode$1(bytes_1, new Uint8Array(lenIn)), new Uint8Array(lenOut));

    const createWasm =  createWasmFn('crypto', wasmBytes, null);

    const bridge = new Bridge(createWasm);
    async function initBridge(createWasm) {
        return bridge.init(createWasm);
    }

    function withWasm(fn) {
        return (...params) => {
            if (!bridge.wasm) {
                throw new Error('The WASM interface has not been initialized. Ensure that you wait for the initialization Promise with waitReady() from @polkadot/wasm-crypto (or cryptoWaitReady() from @polkadot/util-crypto) before attempting to use WASM-only interfaces.');
            }
            return fn(bridge.wasm, ...params);
        };
    }
    const bip39Generate =  withWasm((wasm, words) => {
        wasm.ext_bip39_generate(8, words);
        return bridge.resultString();
    });
    const bip39ToEntropy =  withWasm((wasm, phrase) => {
        wasm.ext_bip39_to_entropy(8, ...bridge.allocString(phrase));
        return bridge.resultU8a();
    });
    const bip39ToMiniSecret =  withWasm((wasm, phrase, password) => {
        wasm.ext_bip39_to_mini_secret(8, ...bridge.allocString(phrase), ...bridge.allocString(password));
        return bridge.resultU8a();
    });
    const bip39ToSeed =  withWasm((wasm, phrase, password) => {
        wasm.ext_bip39_to_seed(8, ...bridge.allocString(phrase), ...bridge.allocString(password));
        return bridge.resultU8a();
    });
    const bip39Validate =  withWasm((wasm, phrase) => {
        const ret = wasm.ext_bip39_validate(...bridge.allocString(phrase));
        return ret !== 0;
    });
    const ed25519KeypairFromSeed =  withWasm((wasm, seed) => {
        wasm.ext_ed_from_seed(8, ...bridge.allocU8a(seed));
        return bridge.resultU8a();
    });
    const ed25519Sign$1 =  withWasm((wasm, pubkey, seckey, message) => {
        wasm.ext_ed_sign(8, ...bridge.allocU8a(pubkey), ...bridge.allocU8a(seckey), ...bridge.allocU8a(message));
        return bridge.resultU8a();
    });
    const ed25519Verify$1 =  withWasm((wasm, signature, message, pubkey) => {
        const ret = wasm.ext_ed_verify(...bridge.allocU8a(signature), ...bridge.allocU8a(message), ...bridge.allocU8a(pubkey));
        return ret !== 0;
    });
    const secp256k1FromSeed =  withWasm((wasm, seckey) => {
        wasm.ext_secp_from_seed(8, ...bridge.allocU8a(seckey));
        return bridge.resultU8a();
    });
    const secp256k1Compress$1 =  withWasm((wasm, pubkey) => {
        wasm.ext_secp_pub_compress(8, ...bridge.allocU8a(pubkey));
        return bridge.resultU8a();
    });
    const secp256k1Expand$1 =  withWasm((wasm, pubkey) => {
        wasm.ext_secp_pub_expand(8, ...bridge.allocU8a(pubkey));
        return bridge.resultU8a();
    });
    const secp256k1Recover$1 =  withWasm((wasm, msgHash, sig, recovery) => {
        wasm.ext_secp_recover(8, ...bridge.allocU8a(msgHash), ...bridge.allocU8a(sig), recovery);
        return bridge.resultU8a();
    });
    const secp256k1Sign$1 =  withWasm((wasm, msgHash, seckey) => {
        wasm.ext_secp_sign(8, ...bridge.allocU8a(msgHash), ...bridge.allocU8a(seckey));
        return bridge.resultU8a();
    });
    const blake2b$1 =  withWasm((wasm, data, key, size) => {
        wasm.ext_blake2b(8, ...bridge.allocU8a(data), ...bridge.allocU8a(key), size);
        return bridge.resultU8a();
    });
    const hmacSha256 =  withWasm((wasm, key, data) => {
        wasm.ext_hmac_sha256(8, ...bridge.allocU8a(key), ...bridge.allocU8a(data));
        return bridge.resultU8a();
    });
    const hmacSha512 =  withWasm((wasm, key, data) => {
        wasm.ext_hmac_sha512(8, ...bridge.allocU8a(key), ...bridge.allocU8a(data));
        return bridge.resultU8a();
    });
    const keccak256 =  withWasm((wasm, data) => {
        wasm.ext_keccak256(8, ...bridge.allocU8a(data));
        return bridge.resultU8a();
    });
    const keccak512 =  withWasm((wasm, data) => {
        wasm.ext_keccak512(8, ...bridge.allocU8a(data));
        return bridge.resultU8a();
    });
    const pbkdf2$1 =  withWasm((wasm, data, salt, rounds) => {
        wasm.ext_pbkdf2(8, ...bridge.allocU8a(data), ...bridge.allocU8a(salt), rounds);
        return bridge.resultU8a();
    });
    const scrypt$1 =  withWasm((wasm, password, salt, log2n, r, p) => {
        wasm.ext_scrypt(8, ...bridge.allocU8a(password), ...bridge.allocU8a(salt), log2n, r, p);
        return bridge.resultU8a();
    });
    const sha256$1 =  withWasm((wasm, data) => {
        wasm.ext_sha256(8, ...bridge.allocU8a(data));
        return bridge.resultU8a();
    });
    const sha512$3 =  withWasm((wasm, data) => {
        wasm.ext_sha512(8, ...bridge.allocU8a(data));
        return bridge.resultU8a();
    });
    const twox =  withWasm((wasm, data, rounds) => {
        wasm.ext_twox(8, ...bridge.allocU8a(data), rounds);
        return bridge.resultU8a();
    });
    function isReady() {
        return !!bridge.wasm;
    }
    async function waitReady() {
        try {
            const wasm = await initBridge();
            return !!wasm;
        }
        catch {
            return false;
        }
    }

    const cryptoIsReady = isReady;
    function cryptoWaitReady() {
        return waitReady()
            .then(() => {
            if (!isReady()) {
                throw new Error('Unable to initialize @polkadot/util-crypto');
            }
            return true;
        })
            .catch(() => false);
    }

    cryptoWaitReady().catch(() => {
    });

    const packageInfo = { name: '@polkadot/util-crypto', path: (({ url: (typeof document === 'undefined' && typeof location === 'undefined' ? require('u' + 'rl').pathToFileURL(__filename).href : typeof document === 'undefined' ? location.href : (_documentCurrentScript && _documentCurrentScript.src || new URL('bundle-polkadot-util-crypto.js', document.baseURI).href)) }) && (typeof document === 'undefined' && typeof location === 'undefined' ? require('u' + 'rl').pathToFileURL(__filename).href : typeof document === 'undefined' ? location.href : (_documentCurrentScript && _documentCurrentScript.src || new URL('bundle-polkadot-util-crypto.js', document.baseURI).href))) ? new URL((typeof document === 'undefined' && typeof location === 'undefined' ? require('u' + 'rl').pathToFileURL(__filename).href : typeof document === 'undefined' ? location.href : (_documentCurrentScript && _documentCurrentScript.src || new URL('bundle-polkadot-util-crypto.js', document.baseURI).href))).pathname.substring(0, new URL((typeof document === 'undefined' && typeof location === 'undefined' ? require('u' + 'rl').pathToFileURL(__filename).href : typeof document === 'undefined' ? location.href : (_documentCurrentScript && _documentCurrentScript.src || new URL('bundle-polkadot-util-crypto.js', document.baseURI).href))).pathname.lastIndexOf('/') + 1) : 'auto', type: 'esm', version: '14.0.3' };

    /*! scure-base - MIT License (c) 2022 Paul Miller (paulmillr.com) */
    function assertNumber(n) {
        if (!Number.isSafeInteger(n))
            throw new Error(`Wrong integer: ${n}`);
    }
    function isBytes$5(a) {
        return (a instanceof Uint8Array ||
            (a != null && typeof a === 'object' && a.constructor.name === 'Uint8Array'));
    }
    function chain(...args) {
        const id = (a) => a;
        const wrap = (a, b) => (c) => a(b(c));
        const encode = args.map((x) => x.encode).reduceRight(wrap, id);
        const decode = args.map((x) => x.decode).reduce(wrap, id);
        return { encode, decode };
    }
    function alphabet(alphabet) {
        return {
            encode: (digits) => {
                if (!Array.isArray(digits) || (digits.length && typeof digits[0] !== 'number'))
                    throw new Error('alphabet.encode input should be an array of numbers');
                return digits.map((i) => {
                    assertNumber(i);
                    if (i < 0 || i >= alphabet.length)
                        throw new Error(`Digit index outside alphabet: ${i} (alphabet: ${alphabet.length})`);
                    return alphabet[i];
                });
            },
            decode: (input) => {
                if (!Array.isArray(input) || (input.length && typeof input[0] !== 'string'))
                    throw new Error('alphabet.decode input should be array of strings');
                return input.map((letter) => {
                    if (typeof letter !== 'string')
                        throw new Error(`alphabet.decode: not string element=${letter}`);
                    const index = alphabet.indexOf(letter);
                    if (index === -1)
                        throw new Error(`Unknown letter: "${letter}". Allowed: ${alphabet}`);
                    return index;
                });
            },
        };
    }
    function join(separator = '') {
        if (typeof separator !== 'string')
            throw new Error('join separator should be string');
        return {
            encode: (from) => {
                if (!Array.isArray(from) || (from.length && typeof from[0] !== 'string'))
                    throw new Error('join.encode input should be array of strings');
                for (let i of from)
                    if (typeof i !== 'string')
                        throw new Error(`join.encode: non-string input=${i}`);
                return from.join(separator);
            },
            decode: (to) => {
                if (typeof to !== 'string')
                    throw new Error('join.decode input should be string');
                return to.split(separator);
            },
        };
    }
    function padding(bits, chr = '=') {
        assertNumber(bits);
        if (typeof chr !== 'string')
            throw new Error('padding chr should be string');
        return {
            encode(data) {
                if (!Array.isArray(data) || (data.length && typeof data[0] !== 'string'))
                    throw new Error('padding.encode input should be array of strings');
                for (let i of data)
                    if (typeof i !== 'string')
                        throw new Error(`padding.encode: non-string input=${i}`);
                while ((data.length * bits) % 8)
                    data.push(chr);
                return data;
            },
            decode(input) {
                if (!Array.isArray(input) || (input.length && typeof input[0] !== 'string'))
                    throw new Error('padding.encode input should be array of strings');
                for (let i of input)
                    if (typeof i !== 'string')
                        throw new Error(`padding.decode: non-string input=${i}`);
                let end = input.length;
                if ((end * bits) % 8)
                    throw new Error('Invalid padding: string should have whole number of bytes');
                for (; end > 0 && input[end - 1] === chr; end--) {
                    if (!(((end - 1) * bits) % 8))
                        throw new Error('Invalid padding: string has too much padding');
                }
                return input.slice(0, end);
            },
        };
    }
    function normalize$1(fn) {
        if (typeof fn !== 'function')
            throw new Error('normalize fn should be function');
        return { encode: (from) => from, decode: (to) => fn(to) };
    }
    function convertRadix(data, from, to) {
        if (from < 2)
            throw new Error(`convertRadix: wrong from=${from}, base cannot be less than 2`);
        if (to < 2)
            throw new Error(`convertRadix: wrong to=${to}, base cannot be less than 2`);
        if (!Array.isArray(data))
            throw new Error('convertRadix: data should be array');
        if (!data.length)
            return [];
        let pos = 0;
        const res = [];
        const digits = Array.from(data);
        digits.forEach((d) => {
            assertNumber(d);
            if (d < 0 || d >= from)
                throw new Error(`Wrong integer: ${d}`);
        });
        while (true) {
            let carry = 0;
            let done = true;
            for (let i = pos; i < digits.length; i++) {
                const digit = digits[i];
                const digitBase = from * carry + digit;
                if (!Number.isSafeInteger(digitBase) ||
                    (from * carry) / from !== carry ||
                    digitBase - digit !== from * carry) {
                    throw new Error('convertRadix: carry overflow');
                }
                carry = digitBase % to;
                const rounded = Math.floor(digitBase / to);
                digits[i] = rounded;
                if (!Number.isSafeInteger(rounded) || rounded * to + carry !== digitBase)
                    throw new Error('convertRadix: carry overflow');
                if (!done)
                    continue;
                else if (!rounded)
                    pos = i;
                else
                    done = false;
            }
            res.push(carry);
            if (done)
                break;
        }
        for (let i = 0; i < data.length - 1 && data[i] === 0; i++)
            res.push(0);
        return res.reverse();
    }
    const gcd =  (a, b) => (!b ? a : gcd(b, a % b));
    const radix2carry =  (from, to) => from + (to - gcd(from, to));
    function convertRadix2(data, from, to, padding) {
        if (!Array.isArray(data))
            throw new Error('convertRadix2: data should be array');
        if (from <= 0 || from > 32)
            throw new Error(`convertRadix2: wrong from=${from}`);
        if (to <= 0 || to > 32)
            throw new Error(`convertRadix2: wrong to=${to}`);
        if (radix2carry(from, to) > 32) {
            throw new Error(`convertRadix2: carry overflow from=${from} to=${to} carryBits=${radix2carry(from, to)}`);
        }
        let carry = 0;
        let pos = 0;
        const mask = 2 ** to - 1;
        const res = [];
        for (const n of data) {
            assertNumber(n);
            if (n >= 2 ** from)
                throw new Error(`convertRadix2: invalid data word=${n} from=${from}`);
            carry = (carry << from) | n;
            if (pos + from > 32)
                throw new Error(`convertRadix2: carry overflow pos=${pos} from=${from}`);
            pos += from;
            for (; pos >= to; pos -= to)
                res.push(((carry >> (pos - to)) & mask) >>> 0);
            carry &= 2 ** pos - 1;
        }
        carry = (carry << (to - pos)) & mask;
        if (!padding && pos >= from)
            throw new Error('Excess padding');
        if (!padding && carry)
            throw new Error(`Non-zero padding: ${carry}`);
        if (padding && pos > 0)
            res.push(carry >>> 0);
        return res;
    }
    function radix(num) {
        assertNumber(num);
        return {
            encode: (bytes) => {
                if (!isBytes$5(bytes))
                    throw new Error('radix.encode input should be Uint8Array');
                return convertRadix(Array.from(bytes), 2 ** 8, num);
            },
            decode: (digits) => {
                if (!Array.isArray(digits) || (digits.length && typeof digits[0] !== 'number'))
                    throw new Error('radix.decode input should be array of numbers');
                return Uint8Array.from(convertRadix(digits, num, 2 ** 8));
            },
        };
    }
    function radix2(bits, revPadding = false) {
        assertNumber(bits);
        if (bits <= 0 || bits > 32)
            throw new Error('radix2: bits should be in (0..32]');
        if (radix2carry(8, bits) > 32 || radix2carry(bits, 8) > 32)
            throw new Error('radix2: carry overflow');
        return {
            encode: (bytes) => {
                if (!isBytes$5(bytes))
                    throw new Error('radix2.encode input should be Uint8Array');
                return convertRadix2(Array.from(bytes), 8, bits, !revPadding);
            },
            decode: (digits) => {
                if (!Array.isArray(digits) || (digits.length && typeof digits[0] !== 'number'))
                    throw new Error('radix2.decode input should be array of numbers');
                return Uint8Array.from(convertRadix2(digits, bits, 8, revPadding));
            },
        };
    }
    function unsafeWrapper(fn) {
        if (typeof fn !== 'function')
            throw new Error('unsafeWrapper fn should be function');
        return function (...args) {
            try {
                return fn.apply(null, args);
            }
            catch (e) { }
        };
    }
    function checksum(len, fn) {
        assertNumber(len);
        if (typeof fn !== 'function')
            throw new Error('checksum fn should be function');
        return {
            encode(data) {
                if (!isBytes$5(data))
                    throw new Error('checksum.encode: input should be Uint8Array');
                const checksum = fn(data).slice(0, len);
                const res = new Uint8Array(data.length + len);
                res.set(data);
                res.set(checksum, data.length);
                return res;
            },
            decode(data) {
                if (!isBytes$5(data))
                    throw new Error('checksum.decode: input should be Uint8Array');
                const payload = data.slice(0, -len);
                const newChecksum = fn(payload).slice(0, len);
                const oldChecksum = data.slice(-len);
                for (let i = 0; i < len; i++)
                    if (newChecksum[i] !== oldChecksum[i])
                        throw new Error('Invalid checksum');
                return payload;
            },
        };
    }
    const utils = {
        alphabet, chain, checksum, convertRadix, convertRadix2, radix, radix2, join, padding,
    };
    chain(radix2(4), alphabet('0123456789ABCDEF'), join(''));
    chain(radix2(5), alphabet('ABCDEFGHIJKLMNOPQRSTUVWXYZ234567'), padding(5), join(''));
    chain(radix2(5), alphabet('ABCDEFGHIJKLMNOPQRSTUVWXYZ234567'), join(''));
    chain(radix2(5), alphabet('0123456789ABCDEFGHIJKLMNOPQRSTUV'), padding(5), join(''));
    chain(radix2(5), alphabet('0123456789ABCDEFGHIJKLMNOPQRSTUV'), join(''));
    chain(radix2(5), alphabet('0123456789ABCDEFGHJKMNPQRSTVWXYZ'), join(''), normalize$1((s) => s.toUpperCase().replace(/O/g, '0').replace(/[IL]/g, '1')));
    const base64 =  chain(radix2(6), alphabet('ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/'), padding(6), join(''));
    chain(radix2(6), alphabet('ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/'), join(''));
    chain(radix2(6), alphabet('ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_'), padding(6), join(''));
    chain(radix2(6), alphabet('ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_'), join(''));
    const genBase58 = (abc) => chain(radix(58), alphabet(abc), join(''));
    const base58 =  genBase58('123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz');
    genBase58('123456789abcdefghijkmnopqrstuvwxyzABCDEFGHJKLMNPQRSTUVWXYZ');
    genBase58('rpshnaf39wBUDNEGHJKLM4PQRST7VWXYZ2bcdeCg65jkm8oFqi1tuvAxyz');
    const BECH_ALPHABET =  chain(alphabet('qpzry9x8gf2tvdw0s3jn54khce6mua7l'), join(''));
    const POLYMOD_GENERATORS = [0x3b6a57b2, 0x26508e6d, 0x1ea119fa, 0x3d4233dd, 0x2a1462b3];
    function bech32Polymod(pre) {
        const b = pre >> 25;
        let chk = (pre & 0x1ffffff) << 5;
        for (let i = 0; i < POLYMOD_GENERATORS.length; i++) {
            if (((b >> i) & 1) === 1)
                chk ^= POLYMOD_GENERATORS[i];
        }
        return chk;
    }
    function bechChecksum(prefix, words, encodingConst = 1) {
        const len = prefix.length;
        let chk = 1;
        for (let i = 0; i < len; i++) {
            const c = prefix.charCodeAt(i);
            if (c < 33 || c > 126)
                throw new Error(`Invalid prefix (${prefix})`);
            chk = bech32Polymod(chk) ^ (c >> 5);
        }
        chk = bech32Polymod(chk);
        for (let i = 0; i < len; i++)
            chk = bech32Polymod(chk) ^ (prefix.charCodeAt(i) & 0x1f);
        for (let v of words)
            chk = bech32Polymod(chk) ^ v;
        for (let i = 0; i < 6; i++)
            chk = bech32Polymod(chk);
        chk ^= encodingConst;
        return BECH_ALPHABET.encode(convertRadix2([chk % 2 ** 30], 30, 5, false));
    }
    function genBech32(encoding) {
        const ENCODING_CONST = encoding === 'bech32' ? 1 : 0x2bc830a3;
        const _words = radix2(5);
        const fromWords = _words.decode;
        const toWords = _words.encode;
        const fromWordsUnsafe = unsafeWrapper(fromWords);
        function encode(prefix, words, limit = 90) {
            if (typeof prefix !== 'string')
                throw new Error(`bech32.encode prefix should be string, not ${typeof prefix}`);
            if (!Array.isArray(words) || (words.length && typeof words[0] !== 'number'))
                throw new Error(`bech32.encode words should be array of numbers, not ${typeof words}`);
            if (prefix.length === 0)
                throw new TypeError(`Invalid prefix length ${prefix.length}`);
            const actualLength = prefix.length + 7 + words.length;
            if (limit !== false && actualLength > limit)
                throw new TypeError(`Length ${actualLength} exceeds limit ${limit}`);
            const lowered = prefix.toLowerCase();
            const sum = bechChecksum(lowered, words, ENCODING_CONST);
            return `${lowered}1${BECH_ALPHABET.encode(words)}${sum}`;
        }
        function decode(str, limit = 90) {
            if (typeof str !== 'string')
                throw new Error(`bech32.decode input should be string, not ${typeof str}`);
            if (str.length < 8 || (limit !== false && str.length > limit))
                throw new TypeError(`Wrong string length: ${str.length} (${str}). Expected (8..${limit})`);
            const lowered = str.toLowerCase();
            if (str !== lowered && str !== str.toUpperCase())
                throw new Error(`String must be lowercase or uppercase`);
            const sepIndex = lowered.lastIndexOf('1');
            if (sepIndex === 0 || sepIndex === -1)
                throw new Error(`Letter "1" must be present between prefix and data only`);
            const prefix = lowered.slice(0, sepIndex);
            const data = lowered.slice(sepIndex + 1);
            if (data.length < 6)
                throw new Error('Data must be at least 6 characters long');
            const words = BECH_ALPHABET.decode(data).slice(0, -6);
            const sum = bechChecksum(prefix, words, ENCODING_CONST);
            if (!data.endsWith(sum))
                throw new Error(`Invalid checksum in ${str}: expected "${sum}"`);
            return { prefix, words };
        }
        const decodeUnsafe = unsafeWrapper(decode);
        function decodeToBytes(str) {
            const { prefix, words } = decode(str, false);
            return { prefix, words, bytes: fromWords(words) };
        }
        return { encode, decode, decodeToBytes, decodeUnsafe, fromWords, fromWordsUnsafe, toWords };
    }
    genBech32('bech32');
    genBech32('bech32m');
    chain(radix2(4), alphabet('0123456789abcdef'), join(''), normalize$1((s) => {
        if (typeof s !== 'string' || s.length % 2)
            throw new TypeError(`hex.decode: expected string, got ${typeof s} with length ${s.length}`);
        return s.toLowerCase();
    }));

    function createDecode({ coder, ipfs }, validate) {
        return (value, ipfsCompat) => {
            validate(value, ipfsCompat);
            return coder.decode(ipfs && ipfsCompat
                ? value.substring(1)
                : value);
        };
    }
    function createEncode({ coder, ipfs }) {
        return (value, ipfsCompat) => {
            const out = coder.encode(util.u8aToU8a(value));
            return ipfs && ipfsCompat
                ? `${ipfs}${out}`
                : out;
        };
    }
    function createIs(validate) {
        return (value, ipfsCompat) => {
            try {
                return validate(value, ipfsCompat);
            }
            catch {
                return false;
            }
        };
    }
    function createValidate({ chars, ipfs, type, withPadding }) {
        return (value, ipfsCompat) => {
            if (typeof value !== 'string') {
                throw new Error(`Expected ${type} string input`);
            }
            else if (ipfs && ipfsCompat && !value.startsWith(ipfs)) {
                throw new Error(`Expected ipfs-compatible ${type} to start with '${ipfs}'`);
            }
            for (let i = (ipfsCompat ? 1 : 0), count = value.length; i < count; i++) {
                if (chars.includes(value[i])) ;
                else if (withPadding && value[i] === '=') {
                    if (i === count - 1) ;
                    else if (value[i + 1] === '=') ;
                    else {
                        throw new Error(`Invalid ${type} padding sequence "${value[i]}${value[i + 1]}" at index ${i}`);
                    }
                }
                else {
                    throw new Error(`Invalid ${type} character "${value[i]}" (0x${value.charCodeAt(i).toString(16)}) at index ${i}`);
                }
            }
            return true;
        };
    }

    const config$2 = {
        chars: '123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz',
        coder: base58,
        ipfs: 'z',
        type: 'base58'
    };
    const base58Validate =  createValidate(config$2);
    const base58Decode =  createDecode(config$2, base58Validate);
    const base58Encode =  createEncode(config$2);
    const isBase58 =  createIs(base58Validate);

    function number(n) {
        if (!Number.isSafeInteger(n) || n < 0)
            throw new Error(`Wrong positive integer: ${n}`);
    }
    function isBytes$4(a) {
        return (a instanceof Uint8Array ||
            (a != null && typeof a === 'object' && a.constructor.name === 'Uint8Array'));
    }
    function bytes(b, ...lengths) {
        if (!isBytes$4(b))
            throw new Error('Expected Uint8Array');
        if (lengths.length > 0 && !lengths.includes(b.length))
            throw new Error(`Expected Uint8Array of length ${lengths}, not of length=${b.length}`);
    }
    function hash(hash) {
        if (typeof hash !== 'function' || typeof hash.create !== 'function')
            throw new Error('Hash should be wrapped by utils.wrapConstructor');
        number(hash.outputLen);
        number(hash.blockLen);
    }
    function exists(instance, checkFinished = true) {
        if (instance.destroyed)
            throw new Error('Hash instance has been destroyed');
        if (checkFinished && instance.finished)
            throw new Error('Hash#digest() has already been called');
    }
    function output(out, instance) {
        bytes(out);
        const min = instance.outputLen;
        if (out.length < min) {
            throw new Error(`digestInto() expects output buffer of length at least ${min}`);
        }
    }

    const crypto$1 = typeof globalThis === 'object' && 'crypto' in globalThis ? globalThis.crypto : undefined;

    /*! noble-hashes - MIT License (c) 2022 Paul Miller (paulmillr.com) */
    const u32$1 = (arr) => new Uint32Array(arr.buffer, arr.byteOffset, Math.floor(arr.byteLength / 4));
    function isBytes$3(a) {
        return (a instanceof Uint8Array ||
            (a != null && typeof a === 'object' && a.constructor.name === 'Uint8Array'));
    }
    const createView$1 = (arr) => new DataView(arr.buffer, arr.byteOffset, arr.byteLength);
    const rotr$1 = (word, shift) => (word << (32 - shift)) | (word >>> shift);
    const isLE$1 = new Uint8Array(new Uint32Array([0x11223344]).buffer)[0] === 0x44;
    if (!isLE$1)
        throw new Error('Non little-endian hardware is not supported');
    Array.from({ length: 256 }, (_, i) => i.toString(16).padStart(2, '0'));
    function utf8ToBytes$2(str) {
        if (typeof str !== 'string')
            throw new Error(`utf8ToBytes expected string, got ${typeof str}`);
        return new Uint8Array(new TextEncoder().encode(str));
    }
    function toBytes$1(data) {
        if (typeof data === 'string')
            data = utf8ToBytes$2(data);
        if (!isBytes$3(data))
            throw new Error(`expected Uint8Array, got ${typeof data}`);
        return data;
    }
    function concatBytes$2(...arrays) {
        let sum = 0;
        for (let i = 0; i < arrays.length; i++) {
            const a = arrays[i];
            if (!isBytes$3(a))
                throw new Error('Uint8Array expected');
            sum += a.length;
        }
        const res = new Uint8Array(sum);
        for (let i = 0, pad = 0; i < arrays.length; i++) {
            const a = arrays[i];
            res.set(a, pad);
            pad += a.length;
        }
        return res;
    }
    let Hash$1 = class Hash {
        clone() {
            return this._cloneInto();
        }
    };
    const toStr = {}.toString;
    function checkOpts(defaults, opts) {
        if (opts !== undefined && toStr.call(opts) !== '[object Object]')
            throw new Error('Options should be object or undefined');
        const merged = Object.assign(defaults, opts);
        return merged;
    }
    function wrapConstructor(hashCons) {
        const hashC = (msg) => hashCons().update(toBytes$1(msg)).digest();
        const tmp = hashCons();
        hashC.outputLen = tmp.outputLen;
        hashC.blockLen = tmp.blockLen;
        hashC.create = () => hashCons();
        return hashC;
    }
    function wrapConstructorWithOpts(hashCons) {
        const hashC = (msg, opts) => hashCons(opts).update(toBytes$1(msg)).digest();
        const tmp = hashCons({});
        hashC.outputLen = tmp.outputLen;
        hashC.blockLen = tmp.blockLen;
        hashC.create = (opts) => hashCons(opts);
        return hashC;
    }
    function wrapXOFConstructorWithOpts(hashCons) {
        const hashC = (msg, opts) => hashCons(opts).update(toBytes$1(msg)).digest();
        const tmp = hashCons({});
        hashC.outputLen = tmp.outputLen;
        hashC.blockLen = tmp.blockLen;
        hashC.create = (opts) => hashCons(opts);
        return hashC;
    }
    function randomBytes$1(bytesLength = 32) {
        if (crypto$1 && typeof crypto$1.getRandomValues === 'function') {
            return crypto$1.getRandomValues(new Uint8Array(bytesLength));
        }
        throw new Error('crypto.getRandomValues must be defined');
    }

    const SIGMA =  new Uint8Array([
        0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15,
        14, 10, 4, 8, 9, 15, 13, 6, 1, 12, 0, 2, 11, 7, 5, 3,
        11, 8, 12, 0, 5, 2, 15, 13, 10, 14, 3, 6, 7, 1, 9, 4,
        7, 9, 3, 1, 13, 12, 11, 14, 2, 6, 5, 10, 4, 0, 15, 8,
        9, 0, 5, 7, 2, 4, 10, 15, 14, 1, 11, 12, 6, 8, 3, 13,
        2, 12, 6, 10, 0, 11, 8, 3, 4, 13, 7, 5, 15, 14, 1, 9,
        12, 5, 1, 15, 14, 13, 4, 10, 0, 7, 6, 3, 9, 2, 8, 11,
        13, 11, 7, 14, 12, 1, 3, 9, 5, 0, 15, 4, 8, 6, 2, 10,
        6, 15, 14, 9, 11, 3, 0, 8, 12, 2, 13, 7, 1, 4, 10, 5,
        10, 2, 8, 4, 7, 6, 1, 5, 15, 11, 9, 14, 3, 12, 13, 0,
        0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15,
        14, 10, 4, 8, 9, 15, 13, 6, 1, 12, 0, 2, 11, 7, 5, 3,
    ]);
    class BLAKE2 extends Hash$1 {
        constructor(blockLen, outputLen, opts = {}, keyLen, saltLen, persLen) {
            super();
            this.blockLen = blockLen;
            this.outputLen = outputLen;
            this.length = 0;
            this.pos = 0;
            this.finished = false;
            this.destroyed = false;
            number(blockLen);
            number(outputLen);
            number(keyLen);
            if (outputLen < 0 || outputLen > keyLen)
                throw new Error('outputLen bigger than keyLen');
            if (opts.key !== undefined && (opts.key.length < 1 || opts.key.length > keyLen))
                throw new Error(`key must be up 1..${keyLen} byte long or undefined`);
            if (opts.salt !== undefined && opts.salt.length !== saltLen)
                throw new Error(`salt must be ${saltLen} byte long or undefined`);
            if (opts.personalization !== undefined && opts.personalization.length !== persLen)
                throw new Error(`personalization must be ${persLen} byte long or undefined`);
            this.buffer32 = u32$1((this.buffer = new Uint8Array(blockLen)));
        }
        update(data) {
            exists(this);
            const { blockLen, buffer, buffer32 } = this;
            data = toBytes$1(data);
            const len = data.length;
            const offset = data.byteOffset;
            const buf = data.buffer;
            for (let pos = 0; pos < len;) {
                if (this.pos === blockLen) {
                    this.compress(buffer32, 0, false);
                    this.pos = 0;
                }
                const take = Math.min(blockLen - this.pos, len - pos);
                const dataOffset = offset + pos;
                if (take === blockLen && !(dataOffset % 4) && pos + take < len) {
                    const data32 = new Uint32Array(buf, dataOffset, Math.floor((len - pos) / 4));
                    for (let pos32 = 0; pos + blockLen < len; pos32 += buffer32.length, pos += blockLen) {
                        this.length += blockLen;
                        this.compress(data32, pos32, false);
                    }
                    continue;
                }
                buffer.set(data.subarray(pos, pos + take), this.pos);
                this.pos += take;
                this.length += take;
                pos += take;
            }
            return this;
        }
        digestInto(out) {
            exists(this);
            output(out, this);
            const { pos, buffer32 } = this;
            this.finished = true;
            this.buffer.subarray(pos).fill(0);
            this.compress(buffer32, 0, true);
            const out32 = u32$1(out);
            this.get().forEach((v, i) => (out32[i] = v));
        }
        digest() {
            const { buffer, outputLen } = this;
            this.digestInto(buffer);
            const res = buffer.slice(0, outputLen);
            this.destroy();
            return res;
        }
        _cloneInto(to) {
            const { buffer, length, finished, destroyed, outputLen, pos } = this;
            to || (to = new this.constructor({ dkLen: outputLen }));
            to.set(...this.get());
            to.length = length;
            to.finished = finished;
            to.destroyed = destroyed;
            to.outputLen = outputLen;
            to.buffer.set(buffer);
            to.pos = pos;
            return to;
        }
    }

    const U32_MASK64$1 =  BigInt(2 ** 32 - 1);
    const _32n$2 =  BigInt(32);
    function fromBig$1(n, le = false) {
        if (le)
            return { h: Number(n & U32_MASK64$1), l: Number((n >> _32n$2) & U32_MASK64$1) };
        return { h: Number((n >> _32n$2) & U32_MASK64$1) | 0, l: Number(n & U32_MASK64$1) | 0 };
    }
    function split$1(lst, le = false) {
        let Ah = new Uint32Array(lst.length);
        let Al = new Uint32Array(lst.length);
        for (let i = 0; i < lst.length; i++) {
            const { h, l } = fromBig$1(lst[i], le);
            [Ah[i], Al[i]] = [h, l];
        }
        return [Ah, Al];
    }
    const toBig = (h, l) => (BigInt(h >>> 0) << _32n$2) | BigInt(l >>> 0);
    const shrSH$1 = (h, _l, s) => h >>> s;
    const shrSL$1 = (h, l, s) => (h << (32 - s)) | (l >>> s);
    const rotrSH$1 = (h, l, s) => (h >>> s) | (l << (32 - s));
    const rotrSL$1 = (h, l, s) => (h << (32 - s)) | (l >>> s);
    const rotrBH$1 = (h, l, s) => (h << (64 - s)) | (l >>> (s - 32));
    const rotrBL$1 = (h, l, s) => (h >>> (s - 32)) | (l << (64 - s));
    const rotr32H = (_h, l) => l;
    const rotr32L = (h, _l) => h;
    const rotlSH$1 = (h, l, s) => (h << s) | (l >>> (32 - s));
    const rotlSL$1 = (h, l, s) => (l << s) | (h >>> (32 - s));
    const rotlBH$1 = (h, l, s) => (l << (s - 32)) | (h >>> (64 - s));
    const rotlBL$1 = (h, l, s) => (h << (s - 32)) | (l >>> (64 - s));
    function add$1(Ah, Al, Bh, Bl) {
        const l = (Al >>> 0) + (Bl >>> 0);
        return { h: (Ah + Bh + ((l / 2 ** 32) | 0)) | 0, l: l | 0 };
    }
    const add3L$1 = (Al, Bl, Cl) => (Al >>> 0) + (Bl >>> 0) + (Cl >>> 0);
    const add3H$1 = (low, Ah, Bh, Ch) => (Ah + Bh + Ch + ((low / 2 ** 32) | 0)) | 0;
    const add4L$1 = (Al, Bl, Cl, Dl) => (Al >>> 0) + (Bl >>> 0) + (Cl >>> 0) + (Dl >>> 0);
    const add4H$1 = (low, Ah, Bh, Ch, Dh) => (Ah + Bh + Ch + Dh + ((low / 2 ** 32) | 0)) | 0;
    const add5L$1 = (Al, Bl, Cl, Dl, El) => (Al >>> 0) + (Bl >>> 0) + (Cl >>> 0) + (Dl >>> 0) + (El >>> 0);
    const add5H$1 = (low, Ah, Bh, Ch, Dh, Eh) => (Ah + Bh + Ch + Dh + Eh + ((low / 2 ** 32) | 0)) | 0;
    const u64 = {
        fromBig: fromBig$1, split: split$1, toBig,
        shrSH: shrSH$1, shrSL: shrSL$1,
        rotrSH: rotrSH$1, rotrSL: rotrSL$1, rotrBH: rotrBH$1, rotrBL: rotrBL$1,
        rotr32H, rotr32L,
        rotlSH: rotlSH$1, rotlSL: rotlSL$1, rotlBH: rotlBH$1, rotlBL: rotlBL$1,
        add: add$1, add3L: add3L$1, add3H: add3H$1, add4L: add4L$1, add4H: add4H$1, add5H: add5H$1, add5L: add5L$1,
    };

    const IV$1 =  new Uint32Array([
        0xf3bcc908, 0x6a09e667, 0x84caa73b, 0xbb67ae85, 0xfe94f82b, 0x3c6ef372, 0x5f1d36f1, 0xa54ff53a,
        0xade682d1, 0x510e527f, 0x2b3e6c1f, 0x9b05688c, 0xfb41bd6b, 0x1f83d9ab, 0x137e2179, 0x5be0cd19
    ]);
    const BUF =  new Uint32Array(32);
    function G1(a, b, c, d, msg, x) {
        const Xl = msg[x], Xh = msg[x + 1];
        let Al = BUF[2 * a], Ah = BUF[2 * a + 1];
        let Bl = BUF[2 * b], Bh = BUF[2 * b + 1];
        let Cl = BUF[2 * c], Ch = BUF[2 * c + 1];
        let Dl = BUF[2 * d], Dh = BUF[2 * d + 1];
        let ll = u64.add3L(Al, Bl, Xl);
        Ah = u64.add3H(ll, Ah, Bh, Xh);
        Al = ll | 0;
        ({ Dh, Dl } = { Dh: Dh ^ Ah, Dl: Dl ^ Al });
        ({ Dh, Dl } = { Dh: u64.rotr32H(Dh, Dl), Dl: u64.rotr32L(Dh, Dl) });
        ({ h: Ch, l: Cl } = u64.add(Ch, Cl, Dh, Dl));
        ({ Bh, Bl } = { Bh: Bh ^ Ch, Bl: Bl ^ Cl });
        ({ Bh, Bl } = { Bh: u64.rotrSH(Bh, Bl, 24), Bl: u64.rotrSL(Bh, Bl, 24) });
        (BUF[2 * a] = Al), (BUF[2 * a + 1] = Ah);
        (BUF[2 * b] = Bl), (BUF[2 * b + 1] = Bh);
        (BUF[2 * c] = Cl), (BUF[2 * c + 1] = Ch);
        (BUF[2 * d] = Dl), (BUF[2 * d + 1] = Dh);
    }
    function G2(a, b, c, d, msg, x) {
        const Xl = msg[x], Xh = msg[x + 1];
        let Al = BUF[2 * a], Ah = BUF[2 * a + 1];
        let Bl = BUF[2 * b], Bh = BUF[2 * b + 1];
        let Cl = BUF[2 * c], Ch = BUF[2 * c + 1];
        let Dl = BUF[2 * d], Dh = BUF[2 * d + 1];
        let ll = u64.add3L(Al, Bl, Xl);
        Ah = u64.add3H(ll, Ah, Bh, Xh);
        Al = ll | 0;
        ({ Dh, Dl } = { Dh: Dh ^ Ah, Dl: Dl ^ Al });
        ({ Dh, Dl } = { Dh: u64.rotrSH(Dh, Dl, 16), Dl: u64.rotrSL(Dh, Dl, 16) });
        ({ h: Ch, l: Cl } = u64.add(Ch, Cl, Dh, Dl));
        ({ Bh, Bl } = { Bh: Bh ^ Ch, Bl: Bl ^ Cl });
        ({ Bh, Bl } = { Bh: u64.rotrBH(Bh, Bl, 63), Bl: u64.rotrBL(Bh, Bl, 63) });
        (BUF[2 * a] = Al), (BUF[2 * a + 1] = Ah);
        (BUF[2 * b] = Bl), (BUF[2 * b + 1] = Bh);
        (BUF[2 * c] = Cl), (BUF[2 * c + 1] = Ch);
        (BUF[2 * d] = Dl), (BUF[2 * d + 1] = Dh);
    }
    class BLAKE2b extends BLAKE2 {
        constructor(opts = {}) {
            super(128, opts.dkLen === undefined ? 64 : opts.dkLen, opts, 64, 16, 16);
            this.v0l = IV$1[0] | 0;
            this.v0h = IV$1[1] | 0;
            this.v1l = IV$1[2] | 0;
            this.v1h = IV$1[3] | 0;
            this.v2l = IV$1[4] | 0;
            this.v2h = IV$1[5] | 0;
            this.v3l = IV$1[6] | 0;
            this.v3h = IV$1[7] | 0;
            this.v4l = IV$1[8] | 0;
            this.v4h = IV$1[9] | 0;
            this.v5l = IV$1[10] | 0;
            this.v5h = IV$1[11] | 0;
            this.v6l = IV$1[12] | 0;
            this.v6h = IV$1[13] | 0;
            this.v7l = IV$1[14] | 0;
            this.v7h = IV$1[15] | 0;
            const keyLength = opts.key ? opts.key.length : 0;
            this.v0l ^= this.outputLen | (keyLength << 8) | (0x01 << 16) | (0x01 << 24);
            if (opts.salt) {
                const salt = u32$1(toBytes$1(opts.salt));
                this.v4l ^= salt[0];
                this.v4h ^= salt[1];
                this.v5l ^= salt[2];
                this.v5h ^= salt[3];
            }
            if (opts.personalization) {
                const pers = u32$1(toBytes$1(opts.personalization));
                this.v6l ^= pers[0];
                this.v6h ^= pers[1];
                this.v7l ^= pers[2];
                this.v7h ^= pers[3];
            }
            if (opts.key) {
                const tmp = new Uint8Array(this.blockLen);
                tmp.set(toBytes$1(opts.key));
                this.update(tmp);
            }
        }
        get() {
            let { v0l, v0h, v1l, v1h, v2l, v2h, v3l, v3h, v4l, v4h, v5l, v5h, v6l, v6h, v7l, v7h } = this;
            return [v0l, v0h, v1l, v1h, v2l, v2h, v3l, v3h, v4l, v4h, v5l, v5h, v6l, v6h, v7l, v7h];
        }
        set(v0l, v0h, v1l, v1h, v2l, v2h, v3l, v3h, v4l, v4h, v5l, v5h, v6l, v6h, v7l, v7h) {
            this.v0l = v0l | 0;
            this.v0h = v0h | 0;
            this.v1l = v1l | 0;
            this.v1h = v1h | 0;
            this.v2l = v2l | 0;
            this.v2h = v2h | 0;
            this.v3l = v3l | 0;
            this.v3h = v3h | 0;
            this.v4l = v4l | 0;
            this.v4h = v4h | 0;
            this.v5l = v5l | 0;
            this.v5h = v5h | 0;
            this.v6l = v6l | 0;
            this.v6h = v6h | 0;
            this.v7l = v7l | 0;
            this.v7h = v7h | 0;
        }
        compress(msg, offset, isLast) {
            this.get().forEach((v, i) => (BUF[i] = v));
            BUF.set(IV$1, 16);
            let { h, l } = u64.fromBig(BigInt(this.length));
            BUF[24] = IV$1[8] ^ l;
            BUF[25] = IV$1[9] ^ h;
            if (isLast) {
                BUF[28] = ~BUF[28];
                BUF[29] = ~BUF[29];
            }
            let j = 0;
            const s = SIGMA;
            for (let i = 0; i < 12; i++) {
                G1(0, 4, 8, 12, msg, offset + 2 * s[j++]);
                G2(0, 4, 8, 12, msg, offset + 2 * s[j++]);
                G1(1, 5, 9, 13, msg, offset + 2 * s[j++]);
                G2(1, 5, 9, 13, msg, offset + 2 * s[j++]);
                G1(2, 6, 10, 14, msg, offset + 2 * s[j++]);
                G2(2, 6, 10, 14, msg, offset + 2 * s[j++]);
                G1(3, 7, 11, 15, msg, offset + 2 * s[j++]);
                G2(3, 7, 11, 15, msg, offset + 2 * s[j++]);
                G1(0, 5, 10, 15, msg, offset + 2 * s[j++]);
                G2(0, 5, 10, 15, msg, offset + 2 * s[j++]);
                G1(1, 6, 11, 12, msg, offset + 2 * s[j++]);
                G2(1, 6, 11, 12, msg, offset + 2 * s[j++]);
                G1(2, 7, 8, 13, msg, offset + 2 * s[j++]);
                G2(2, 7, 8, 13, msg, offset + 2 * s[j++]);
                G1(3, 4, 9, 14, msg, offset + 2 * s[j++]);
                G2(3, 4, 9, 14, msg, offset + 2 * s[j++]);
            }
            this.v0l ^= BUF[0] ^ BUF[16];
            this.v0h ^= BUF[1] ^ BUF[17];
            this.v1l ^= BUF[2] ^ BUF[18];
            this.v1h ^= BUF[3] ^ BUF[19];
            this.v2l ^= BUF[4] ^ BUF[20];
            this.v2h ^= BUF[5] ^ BUF[21];
            this.v3l ^= BUF[6] ^ BUF[22];
            this.v3h ^= BUF[7] ^ BUF[23];
            this.v4l ^= BUF[8] ^ BUF[24];
            this.v4h ^= BUF[9] ^ BUF[25];
            this.v5l ^= BUF[10] ^ BUF[26];
            this.v5h ^= BUF[11] ^ BUF[27];
            this.v6l ^= BUF[12] ^ BUF[28];
            this.v6h ^= BUF[13] ^ BUF[29];
            this.v7l ^= BUF[14] ^ BUF[30];
            this.v7h ^= BUF[15] ^ BUF[31];
            BUF.fill(0);
        }
        destroy() {
            this.destroyed = true;
            this.buffer32.fill(0);
            this.set(0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0);
        }
    }
    const blake2b =  wrapConstructorWithOpts((opts) => new BLAKE2b(opts));

    function createAsHex(fn) {
        return (...args) => util.u8aToHex(fn(...args));
    }
    function createBitHasher(bitLength, fn) {
        return (data, onlyJs) => fn(data, bitLength, onlyJs);
    }
    function createDualHasher(wa, js) {
        return (value, bitLength = 256, onlyJs) => {
            const u8a = util.u8aToU8a(value);
            return !util.hasBigInt || (!onlyJs && isReady())
                ? wa[bitLength](u8a)
                : js[bitLength](u8a);
        };
    }

    function blake2AsU8a(data, bitLength = 256, key, onlyJs) {
        const byteLength = Math.ceil(bitLength / 8);
        const u8a = util.u8aToU8a(data);
        return !util.hasBigInt || (!onlyJs && isReady())
            ? blake2b$1(u8a, util.u8aToU8a(key), byteLength)
            : key
                ? blake2b(u8a, { dkLen: byteLength, key })
                : blake2b(u8a, { dkLen: byteLength });
    }
    const blake2AsHex =  createAsHex(blake2AsU8a);

    const SS58_PREFIX = util.stringToU8a('SS58PRE');
    function sshash(key) {
        return blake2AsU8a(util.u8aConcat(SS58_PREFIX, key), 512);
    }

    function checkAddressChecksum(decoded) {
        const ss58Length = (decoded[0] & 0b0100_0000) ? 2 : 1;
        const ss58Decoded = ss58Length === 1
            ? decoded[0]
            : ((decoded[0] & 0b0011_1111) << 2) | (decoded[1] >> 6) | ((decoded[1] & 0b0011_1111) << 8);
        const isPublicKey = [34 + ss58Length, 35 + ss58Length].includes(decoded.length);
        const length = decoded.length - (isPublicKey ? 2 : 1);
        const hash = sshash(decoded.subarray(0, length));
        const isValid = (decoded[0] & 0b1000_0000) === 0 && ![46, 47].includes(decoded[0]) && (isPublicKey
            ? decoded[decoded.length - 2] === hash[0] && decoded[decoded.length - 1] === hash[1]
            : decoded[decoded.length - 1] === hash[0]);
        return [isValid, length, ss58Length, ss58Decoded];
    }

    const knownSubstrate = [
    	{
    		"prefix": 0,
    		"network": "polkadot",
    		"displayName": "Polkadot Relay Chain",
    		"symbols": [
    			"DOT"
    		],
    		"decimals": [
    			10
    		],
    		"standardAccount": "*25519",
    		"website": "https://polkadot.network"
    	},
    	{
    		"prefix": 1,
    		"network": "BareSr25519",
    		"displayName": "Bare 32-bit Schnorr/Ristretto (S/R 25519) public key.",
    		"symbols": [],
    		"decimals": [],
    		"standardAccount": "Sr25519",
    		"website": null
    	},
    	{
    		"prefix": 2,
    		"network": "kusama",
    		"displayName": "Kusama Relay Chain",
    		"symbols": [
    			"KSM"
    		],
    		"decimals": [
    			12
    		],
    		"standardAccount": "*25519",
    		"website": "https://kusama.network"
    	},
    	{
    		"prefix": 3,
    		"network": "BareEd25519",
    		"displayName": "Bare 32-bit Ed25519 public key.",
    		"symbols": [],
    		"decimals": [],
    		"standardAccount": "Ed25519",
    		"website": null
    	},
    	{
    		"prefix": 4,
    		"network": "katalchain",
    		"displayName": "Katal Chain",
    		"symbols": [],
    		"decimals": [],
    		"standardAccount": "*25519",
    		"website": null
    	},
    	{
    		"prefix": 5,
    		"network": "astar",
    		"displayName": "Astar Network",
    		"symbols": [
    			"ASTR"
    		],
    		"decimals": [
    			18
    		],
    		"standardAccount": "*25519",
    		"website": "https://astar.network"
    	},
    	{
    		"prefix": 6,
    		"network": "bifrost",
    		"displayName": "Bifrost",
    		"symbols": [
    			"BNC"
    		],
    		"decimals": [
    			12
    		],
    		"standardAccount": "*25519",
    		"website": "https://bifrost.finance/"
    	},
    	{
    		"prefix": 7,
    		"network": "edgeware",
    		"displayName": "Edgeware",
    		"symbols": [
    			"EDG"
    		],
    		"decimals": [
    			18
    		],
    		"standardAccount": "*25519",
    		"website": "https://edgewa.re"
    	},
    	{
    		"prefix": 8,
    		"network": "karura",
    		"displayName": "Karura",
    		"symbols": [
    			"KAR"
    		],
    		"decimals": [
    			12
    		],
    		"standardAccount": "*25519",
    		"website": "https://karura.network/"
    	},
    	{
    		"prefix": 9,
    		"network": "reynolds",
    		"displayName": "Laminar Reynolds Canary",
    		"symbols": [
    			"REY"
    		],
    		"decimals": [
    			18
    		],
    		"standardAccount": "*25519",
    		"website": "http://laminar.network/"
    	},
    	{
    		"prefix": 10,
    		"network": "acala",
    		"displayName": "Acala",
    		"symbols": [
    			"ACA"
    		],
    		"decimals": [
    			12
    		],
    		"standardAccount": "*25519",
    		"website": "https://acala.network/"
    	},
    	{
    		"prefix": 11,
    		"network": "laminar",
    		"displayName": "Laminar",
    		"symbols": [
    			"LAMI"
    		],
    		"decimals": [
    			18
    		],
    		"standardAccount": "*25519",
    		"website": "http://laminar.network/"
    	},
    	{
    		"prefix": 12,
    		"network": "polymesh",
    		"displayName": "Polymesh",
    		"symbols": [
    			"POLYX"
    		],
    		"decimals": [
    			6
    		],
    		"standardAccount": "*25519",
    		"website": "https://polymath.network/"
    	},
    	{
    		"prefix": 13,
    		"network": "integritee",
    		"displayName": "Integritee",
    		"symbols": [
    			"TEER"
    		],
    		"decimals": [
    			12
    		],
    		"standardAccount": "*25519",
    		"website": "https://integritee.network"
    	},
    	{
    		"prefix": 14,
    		"network": "totem",
    		"displayName": "Totem",
    		"symbols": [
    			"TOTEM"
    		],
    		"decimals": [
    			0
    		],
    		"standardAccount": "*25519",
    		"website": "https://totemaccounting.com"
    	},
    	{
    		"prefix": 15,
    		"network": "synesthesia",
    		"displayName": "Synesthesia",
    		"symbols": [
    			"SYN"
    		],
    		"decimals": [
    			12
    		],
    		"standardAccount": "*25519",
    		"website": "https://synesthesia.network/"
    	},
    	{
    		"prefix": 16,
    		"network": "kulupu",
    		"displayName": "Kulupu",
    		"symbols": [
    			"KLP"
    		],
    		"decimals": [
    			12
    		],
    		"standardAccount": "*25519",
    		"website": "https://kulupu.network/"
    	},
    	{
    		"prefix": 17,
    		"network": "dark",
    		"displayName": "Dark Mainnet",
    		"symbols": [],
    		"decimals": [],
    		"standardAccount": "*25519",
    		"website": null
    	},
    	{
    		"prefix": 18,
    		"network": "darwinia",
    		"displayName": "Darwinia Network",
    		"symbols": [
    			"RING"
    		],
    		"decimals": [
    			18
    		],
    		"standardAccount": "secp256k1",
    		"website": "https://darwinia.network"
    	},
    	{
    		"prefix": 19,
    		"network": "watr",
    		"displayName": "Watr Protocol",
    		"symbols": [
    			"WATR"
    		],
    		"decimals": [
    			18
    		],
    		"standardAccount": "*25519",
    		"website": "https://www.watr.org"
    	},
    	{
    		"prefix": 20,
    		"network": "stafi",
    		"displayName": "Stafi",
    		"symbols": [
    			"FIS"
    		],
    		"decimals": [
    			12
    		],
    		"standardAccount": "*25519",
    		"website": "https://stafi.io"
    	},
    	{
    		"prefix": 21,
    		"network": "karmachain",
    		"displayName": "Karmacoin",
    		"symbols": [
    			"KCOIN"
    		],
    		"decimals": [
    			6
    		],
    		"standardAccount": "*25519",
    		"website": "https://karmaco.in"
    	},
    	{
    		"prefix": 22,
    		"network": "dock-pos-mainnet",
    		"displayName": "Dock Mainnet",
    		"symbols": [
    			"DCK"
    		],
    		"decimals": [
    			6
    		],
    		"standardAccount": "*25519",
    		"website": "https://dock.io"
    	},
    	{
    		"prefix": 23,
    		"network": "shift",
    		"displayName": "ShiftNrg",
    		"symbols": [],
    		"decimals": [],
    		"standardAccount": "*25519",
    		"website": null
    	},
    	{
    		"prefix": 24,
    		"network": "zero",
    		"displayName": "ZERO",
    		"symbols": [
    			"ZERO"
    		],
    		"decimals": [
    			18
    		],
    		"standardAccount": "*25519",
    		"website": "https://zero.io"
    	},
    	{
    		"prefix": 25,
    		"network": "zero-alphaville",
    		"displayName": "ZERO Alphaville",
    		"symbols": [
    			"ZERO"
    		],
    		"decimals": [
    			18
    		],
    		"standardAccount": "*25519",
    		"website": "https://zero.io"
    	},
    	{
    		"prefix": 26,
    		"network": "jupiter",
    		"displayName": "Jupiter",
    		"symbols": [
    			"jDOT"
    		],
    		"decimals": [
    			10
    		],
    		"standardAccount": "*25519",
    		"website": "https://jupiter.patract.io"
    	},
    	{
    		"prefix": 27,
    		"network": "kabocha",
    		"displayName": "Kabocha",
    		"symbols": [
    			"KAB"
    		],
    		"decimals": [
    			12
    		],
    		"standardAccount": "*25519",
    		"website": "https://kabocha.network"
    	},
    	{
    		"prefix": 28,
    		"network": "subsocial",
    		"displayName": "Subsocial",
    		"symbols": [],
    		"decimals": [],
    		"standardAccount": "*25519",
    		"website": null
    	},
    	{
    		"prefix": 29,
    		"network": "cord",
    		"displayName": "CORD Network",
    		"symbols": [
    			"DHI",
    			"WAY"
    		],
    		"decimals": [
    			12,
    			12
    		],
    		"standardAccount": "*25519",
    		"website": "https://cord.network/"
    	},
    	{
    		"prefix": 30,
    		"network": "phala",
    		"displayName": "Phala Network",
    		"symbols": [
    			"PHA"
    		],
    		"decimals": [
    			12
    		],
    		"standardAccount": "*25519",
    		"website": "https://phala.network"
    	},
    	{
    		"prefix": 31,
    		"network": "litentry",
    		"displayName": "Litentry Network",
    		"symbols": [
    			"LIT"
    		],
    		"decimals": [
    			12
    		],
    		"standardAccount": "*25519",
    		"website": "https://litentry.com/"
    	},
    	{
    		"prefix": 32,
    		"network": "robonomics",
    		"displayName": "Robonomics",
    		"symbols": [
    			"XRT"
    		],
    		"decimals": [
    			9
    		],
    		"standardAccount": "*25519",
    		"website": "https://robonomics.network"
    	},
    	{
    		"prefix": 33,
    		"network": "datahighway",
    		"displayName": "DataHighway",
    		"symbols": [],
    		"decimals": [],
    		"standardAccount": "*25519",
    		"website": null
    	},
    	{
    		"prefix": 34,
    		"network": "ares",
    		"displayName": "Ares Protocol",
    		"symbols": [
    			"ARES"
    		],
    		"decimals": [
    			12
    		],
    		"standardAccount": "*25519",
    		"website": "https://www.aresprotocol.com/"
    	},
    	{
    		"prefix": 35,
    		"network": "vln",
    		"displayName": "Valiu Liquidity Network",
    		"symbols": [
    			"USDv"
    		],
    		"decimals": [
    			15
    		],
    		"standardAccount": "*25519",
    		"website": "https://valiu.com/"
    	},
    	{
    		"prefix": 36,
    		"network": "centrifuge",
    		"displayName": "Centrifuge Chain",
    		"symbols": [
    			"CFG"
    		],
    		"decimals": [
    			18
    		],
    		"standardAccount": "*25519",
    		"website": "https://centrifuge.io/"
    	},
    	{
    		"prefix": 37,
    		"network": "nodle",
    		"displayName": "Nodle Chain",
    		"symbols": [
    			"NODL"
    		],
    		"decimals": [
    			11
    		],
    		"standardAccount": "*25519",
    		"website": "https://nodle.io/"
    	},
    	{
    		"prefix": 38,
    		"network": "kilt",
    		"displayName": "KILT Spiritnet",
    		"symbols": [
    			"KILT"
    		],
    		"decimals": [
    			15
    		],
    		"standardAccount": "*25519",
    		"website": "https://kilt.io/"
    	},
    	{
    		"prefix": 39,
    		"network": "mathchain",
    		"displayName": "MathChain mainnet",
    		"symbols": [
    			"MATH"
    		],
    		"decimals": [
    			18
    		],
    		"standardAccount": "*25519",
    		"website": "https://mathwallet.org"
    	},
    	{
    		"prefix": 40,
    		"network": "mathchain-testnet",
    		"displayName": "MathChain testnet",
    		"symbols": [
    			"MATH"
    		],
    		"decimals": [
    			18
    		],
    		"standardAccount": "*25519",
    		"website": "https://mathwallet.org"
    	},
    	{
    		"prefix": 41,
    		"network": "polimec",
    		"displayName": "Polimec Protocol",
    		"symbols": [
    			"PLMC"
    		],
    		"decimals": [
    			10
    		],
    		"standardAccount": "*25519",
    		"website": "https://www.polimec.org/"
    	},
    	{
    		"prefix": 42,
    		"network": "substrate",
    		"displayName": "Substrate",
    		"symbols": [],
    		"decimals": [],
    		"standardAccount": "*25519",
    		"website": "https://substrate.io/"
    	},
    	{
    		"prefix": 43,
    		"network": "BareSecp256k1",
    		"displayName": "Bare 32-bit ECDSA SECP-256k1 public key.",
    		"symbols": [],
    		"decimals": [],
    		"standardAccount": "secp256k1",
    		"website": null
    	},
    	{
    		"prefix": 44,
    		"network": "chainx",
    		"displayName": "ChainX",
    		"symbols": [
    			"PCX"
    		],
    		"decimals": [
    			8
    		],
    		"standardAccount": "*25519",
    		"website": "https://chainx.org/"
    	},
    	{
    		"prefix": 45,
    		"network": "uniarts",
    		"displayName": "UniArts Network",
    		"symbols": [
    			"UART",
    			"UINK"
    		],
    		"decimals": [
    			12,
    			12
    		],
    		"standardAccount": "*25519",
    		"website": "https://uniarts.me"
    	},
    	{
    		"prefix": 46,
    		"network": "reserved46",
    		"displayName": "This prefix is reserved.",
    		"symbols": [],
    		"decimals": [],
    		"standardAccount": null,
    		"website": null
    	},
    	{
    		"prefix": 47,
    		"network": "reserved47",
    		"displayName": "This prefix is reserved.",
    		"symbols": [],
    		"decimals": [],
    		"standardAccount": null,
    		"website": null
    	},
    	{
    		"prefix": 48,
    		"network": "neatcoin",
    		"displayName": "Neatcoin Mainnet",
    		"symbols": [
    			"NEAT"
    		],
    		"decimals": [
    			12
    		],
    		"standardAccount": "*25519",
    		"website": "https://neatcoin.org"
    	},
    	{
    		"prefix": 49,
    		"network": "picasso",
    		"displayName": "Picasso",
    		"symbols": [
    			"PICA"
    		],
    		"decimals": [
    			12
    		],
    		"standardAccount": "*25519",
    		"website": "https://picasso.composable.finance"
    	},
    	{
    		"prefix": 50,
    		"network": "composable",
    		"displayName": "Composable Finance",
    		"symbols": [
    			"LAYR"
    		],
    		"decimals": [
    			12
    		],
    		"standardAccount": "*25519",
    		"website": "https://composable.finance"
    	},
    	{
    		"prefix": 51,
    		"network": "oak",
    		"displayName": "OAK Network",
    		"symbols": [
    			"OAK",
    			"TUR"
    		],
    		"decimals": [
    			10,
    			10
    		],
    		"standardAccount": "*25519",
    		"website": "https://oak.tech"
    	},
    	{
    		"prefix": 52,
    		"network": "KICO",
    		"displayName": "KICO",
    		"symbols": [
    			"KICO"
    		],
    		"decimals": [
    			14
    		],
    		"standardAccount": "*25519",
    		"website": "https://dico.io"
    	},
    	{
    		"prefix": 53,
    		"network": "DICO",
    		"displayName": "DICO",
    		"symbols": [
    			"DICO"
    		],
    		"decimals": [
    			14
    		],
    		"standardAccount": "*25519",
    		"website": "https://dico.io"
    	},
    	{
    		"prefix": 54,
    		"network": "cere",
    		"displayName": "Cere Network",
    		"symbols": [
    			"CERE"
    		],
    		"decimals": [
    			10
    		],
    		"standardAccount": "*25519",
    		"website": "https://cere.network"
    	},
    	{
    		"prefix": 55,
    		"network": "xxnetwork",
    		"displayName": "xx network",
    		"symbols": [
    			"XX"
    		],
    		"decimals": [
    			9
    		],
    		"standardAccount": "*25519",
    		"website": "https://xx.network"
    	},
    	{
    		"prefix": 56,
    		"network": "pendulum",
    		"displayName": "Pendulum chain",
    		"symbols": [
    			"PEN"
    		],
    		"decimals": [
    			12
    		],
    		"standardAccount": "*25519",
    		"website": "https://pendulumchain.org/"
    	},
    	{
    		"prefix": 57,
    		"network": "amplitude",
    		"displayName": "Amplitude chain",
    		"symbols": [
    			"AMPE"
    		],
    		"decimals": [
    			12
    		],
    		"standardAccount": "*25519",
    		"website": "https://pendulumchain.org/"
    	},
    	{
    		"prefix": 58,
    		"network": "eternal-civilization",
    		"displayName": "Eternal Civilization",
    		"symbols": [
    			"ECC"
    		],
    		"decimals": [
    			12
    		],
    		"standardAccount": "*25519",
    		"website": "http://www.ysknfr.cn/"
    	},
    	{
    		"prefix": 63,
    		"network": "hydradx",
    		"displayName": "Hydration",
    		"symbols": [
    			"HDX"
    		],
    		"decimals": [
    			12
    		],
    		"standardAccount": "*25519",
    		"website": "https://hydration.net"
    	},
    	{
    		"prefix": 65,
    		"network": "aventus",
    		"displayName": "Aventus Mainnet",
    		"symbols": [
    			"AVT"
    		],
    		"decimals": [
    			18
    		],
    		"standardAccount": "*25519",
    		"website": "https://aventus.io"
    	},
    	{
    		"prefix": 66,
    		"network": "crust",
    		"displayName": "Crust Network",
    		"symbols": [
    			"CRU"
    		],
    		"decimals": [
    			12
    		],
    		"standardAccount": "*25519",
    		"website": "https://crust.network"
    	},
    	{
    		"prefix": 67,
    		"network": "genshiro",
    		"displayName": "Genshiro Network",
    		"symbols": [
    			"GENS",
    			"EQD",
    			"LPT0"
    		],
    		"decimals": [
    			9,
    			9,
    			9
    		],
    		"standardAccount": "*25519",
    		"website": "https://genshiro.equilibrium.io"
    	},
    	{
    		"prefix": 68,
    		"network": "equilibrium",
    		"displayName": "Equilibrium Network",
    		"symbols": [
    			"EQ"
    		],
    		"decimals": [
    			9
    		],
    		"standardAccount": "*25519",
    		"website": "https://equilibrium.io"
    	},
    	{
    		"prefix": 69,
    		"network": "sora",
    		"displayName": "SORA Network",
    		"symbols": [
    			"XOR"
    		],
    		"decimals": [
    			18
    		],
    		"standardAccount": "*25519",
    		"website": "https://sora.org"
    	},
    	{
    		"prefix": 71,
    		"network": "p3d",
    		"displayName": "3DP network",
    		"symbols": [
    			"P3D"
    		],
    		"decimals": [
    			12
    		],
    		"standardAccount": "*25519",
    		"website": "https://3dpass.org"
    	},
    	{
    		"prefix": 72,
    		"network": "p3dt",
    		"displayName": "3DP test network",
    		"symbols": [
    			"P3Dt"
    		],
    		"decimals": [
    			12
    		],
    		"standardAccount": "*25519",
    		"website": "https://3dpass.org"
    	},
    	{
    		"prefix": 73,
    		"network": "zeitgeist",
    		"displayName": "Zeitgeist",
    		"symbols": [
    			"ZTG"
    		],
    		"decimals": [
    			10
    		],
    		"standardAccount": "*25519",
    		"website": "https://zeitgeist.pm"
    	},
    	{
    		"prefix": 77,
    		"network": "manta",
    		"displayName": "Manta network",
    		"symbols": [
    			"MANTA"
    		],
    		"decimals": [
    			18
    		],
    		"standardAccount": "*25519",
    		"website": "https://manta.network"
    	},
    	{
    		"prefix": 78,
    		"network": "calamari",
    		"displayName": "Calamari: Manta Canary Network",
    		"symbols": [
    			"KMA"
    		],
    		"decimals": [
    			12
    		],
    		"standardAccount": "*25519",
    		"website": "https://manta.network"
    	},
    	{
    		"prefix": 81,
    		"network": "sora_dot_para",
    		"displayName": "SORA Polkadot Parachain",
    		"symbols": [
    			"XOR"
    		],
    		"decimals": [
    			18
    		],
    		"standardAccount": "*25519",
    		"website": "https://sora.org"
    	},
    	{
    		"prefix": 88,
    		"network": "polkadex",
    		"displayName": "Polkadex Mainnet",
    		"symbols": [
    			"PDEX"
    		],
    		"decimals": [
    			12
    		],
    		"standardAccount": "*25519",
    		"website": "https://polkadex.trade"
    	},
    	{
    		"prefix": 89,
    		"network": "polkadexparachain",
    		"displayName": "Polkadex Parachain",
    		"symbols": [
    			"PDEX"
    		],
    		"decimals": [
    			12
    		],
    		"standardAccount": "*25519",
    		"website": "https://polkadex.trade"
    	},
    	{
    		"prefix": 90,
    		"network": "frequency",
    		"displayName": "Frequency",
    		"symbols": [
    			"FRQCY"
    		],
    		"decimals": [
    			8
    		],
    		"standardAccount": "*25519",
    		"website": "https://www.frequency.xyz"
    	},
    	{
    		"prefix": 92,
    		"network": "anmol",
    		"displayName": "Anmol Network",
    		"symbols": [
    			"ANML"
    		],
    		"decimals": [
    			18
    		],
    		"standardAccount": "*25519",
    		"website": "https://anmol.network/"
    	},
    	{
    		"prefix": 93,
    		"network": "fragnova",
    		"displayName": "Fragnova Network",
    		"symbols": [
    			"NOVA"
    		],
    		"decimals": [
    			12
    		],
    		"standardAccount": "*25519",
    		"website": "https://fragnova.com"
    	},
    	{
    		"prefix": 98,
    		"network": "polkasmith",
    		"displayName": "PolkaSmith Canary Network",
    		"symbols": [
    			"PKS"
    		],
    		"decimals": [
    			18
    		],
    		"standardAccount": "*25519",
    		"website": "https://polkafoundry.com"
    	},
    	{
    		"prefix": 99,
    		"network": "polkafoundry",
    		"displayName": "PolkaFoundry Network",
    		"symbols": [
    			"PKF"
    		],
    		"decimals": [
    			18
    		],
    		"standardAccount": "*25519",
    		"website": "https://polkafoundry.com"
    	},
    	{
    		"prefix": 100,
    		"network": "ibtida",
    		"displayName": "Anmol Network Ibtida Canary network",
    		"symbols": [
    			"IANML"
    		],
    		"decimals": [
    			18
    		],
    		"standardAccount": "*25519",
    		"website": "https://anmol.network/"
    	},
    	{
    		"prefix": 101,
    		"network": "origintrail-parachain",
    		"displayName": "OriginTrail Parachain",
    		"symbols": [
    			"OTP"
    		],
    		"decimals": [
    			12
    		],
    		"standardAccount": "*25519",
    		"website": "https://parachain.origintrail.io/"
    	},
    	{
    		"prefix": 105,
    		"network": "pontem-network",
    		"displayName": "Pontem Network",
    		"symbols": [
    			"PONT"
    		],
    		"decimals": [
    			10
    		],
    		"standardAccount": "*25519",
    		"website": "https://pontem.network"
    	},
    	{
    		"prefix": 110,
    		"network": "heiko",
    		"displayName": "Heiko",
    		"symbols": [
    			"HKO"
    		],
    		"decimals": [
    			12
    		],
    		"standardAccount": "*25519",
    		"website": "https://parallel.fi/"
    	},
    	{
    		"prefix": 113,
    		"network": "integritee-incognito",
    		"displayName": "Integritee Incognito",
    		"symbols": [],
    		"decimals": [],
    		"standardAccount": "*25519",
    		"website": "https://integritee.network"
    	},
    	{
    		"prefix": 117,
    		"network": "tinker",
    		"displayName": "Tinker",
    		"symbols": [
    			"TNKR"
    		],
    		"decimals": [
    			12
    		],
    		"standardAccount": "*25519",
    		"website": "https://invarch.network"
    	},
    	{
    		"prefix": 126,
    		"network": "joystream",
    		"displayName": "Joystream",
    		"symbols": [
    			"JOY"
    		],
    		"decimals": [
    			10
    		],
    		"standardAccount": "*25519",
    		"website": "https://www.joystream.org"
    	},
    	{
    		"prefix": 128,
    		"network": "clover",
    		"displayName": "Clover Finance",
    		"symbols": [
    			"CLV"
    		],
    		"decimals": [
    			18
    		],
    		"standardAccount": "*25519",
    		"website": "https://clover.finance"
    	},
    	{
    		"prefix": 129,
    		"network": "dorafactory-polkadot",
    		"displayName": "Dorafactory Polkadot Network",
    		"symbols": [
    			"DORA"
    		],
    		"decimals": [
    			12
    		],
    		"standardAccount": "*25519",
    		"website": "https://dorafactory.org"
    	},
    	{
    		"prefix": 131,
    		"network": "litmus",
    		"displayName": "Litmus Network",
    		"symbols": [
    			"LIT"
    		],
    		"decimals": [
    			12
    		],
    		"standardAccount": "*25519",
    		"website": "https://litentry.com/"
    	},
    	{
    		"prefix": 136,
    		"network": "altair",
    		"displayName": "Altair",
    		"symbols": [
    			"AIR"
    		],
    		"decimals": [
    			18
    		],
    		"standardAccount": "*25519",
    		"website": "https://centrifuge.io/"
    	},
    	{
    		"prefix": 137,
    		"network": "vara",
    		"displayName": "Vara Network",
    		"symbols": [
    			"VARA"
    		],
    		"decimals": [
    			12
    		],
    		"standardAccount": "*25519",
    		"website": "https://vara.network/"
    	},
    	{
    		"prefix": 172,
    		"network": "parallel",
    		"displayName": "Parallel",
    		"symbols": [
    			"PARA"
    		],
    		"decimals": [
    			12
    		],
    		"standardAccount": "*25519",
    		"website": "https://parallel.fi/"
    	},
    	{
    		"prefix": 252,
    		"network": "social-network",
    		"displayName": "Social Network",
    		"symbols": [
    			"NET"
    		],
    		"decimals": [
    			18
    		],
    		"standardAccount": "*25519",
    		"website": "https://social.network"
    	},
    	{
    		"prefix": 255,
    		"network": "quartz_mainnet",
    		"displayName": "QUARTZ by UNIQUE",
    		"symbols": [
    			"QTZ"
    		],
    		"decimals": [
    			18
    		],
    		"standardAccount": "*25519",
    		"website": "https://unique.network"
    	},
    	{
    		"prefix": 268,
    		"network": "pioneer_network",
    		"displayName": "Pioneer Network by Bit.Country",
    		"symbols": [
    			"NEER"
    		],
    		"decimals": [
    			18
    		],
    		"standardAccount": "*25519",
    		"website": "https://bit.country"
    	},
    	{
    		"prefix": 420,
    		"network": "sora_kusama_para",
    		"displayName": "SORA Kusama Parachain",
    		"symbols": [
    			"XOR"
    		],
    		"decimals": [
    			18
    		],
    		"standardAccount": "*25519",
    		"website": "https://sora.org"
    	},
    	{
    		"prefix": 440,
    		"network": "allfeat_network",
    		"displayName": "Allfeat Network",
    		"symbols": [
    			"AFT"
    		],
    		"decimals": [
    			12
    		],
    		"standardAccount": "*25519",
    		"website": "https://allfeat.network"
    	},
    	{
    		"prefix": 666,
    		"network": "metaquity_network",
    		"displayName": "Metaquity Network",
    		"symbols": [
    			"MQTY"
    		],
    		"decimals": [
    			18
    		],
    		"standardAccount": "*25519",
    		"website": "https://metaquity.xyz/"
    	},
    	{
    		"prefix": 777,
    		"network": "curio",
    		"displayName": "Curio",
    		"symbols": [
    			"CGT"
    		],
    		"decimals": [
    			18
    		],
    		"standardAccount": "*25519",
    		"website": "https://parachain.capitaldex.exchange/"
    	},
    	{
    		"prefix": 789,
    		"network": "geek",
    		"displayName": "GEEK Network",
    		"symbols": [
    			"GEEK"
    		],
    		"decimals": [
    			18
    		],
    		"standardAccount": "*25519",
    		"website": "https://geek.gl"
    	},
    	{
    		"prefix": 995,
    		"network": "ternoa",
    		"displayName": "Ternoa",
    		"symbols": [
    			"CAPS"
    		],
    		"decimals": [
    			18
    		],
    		"standardAccount": "*25519",
    		"website": "https://www.ternoa.network"
    	},
    	{
    		"prefix": 1110,
    		"network": "efinity",
    		"displayName": "Efinity",
    		"symbols": [
    			"EFI"
    		],
    		"decimals": [
    			18
    		],
    		"standardAccount": "*25519",
    		"website": "https://efinity.io/"
    	},
    	{
    		"prefix": 1221,
    		"network": "peaq",
    		"displayName": "Peaq Network",
    		"symbols": [
    			"PEAQ"
    		],
    		"decimals": [
    			18
    		],
    		"standardAccount": "Sr25519",
    		"website": "https://www.peaq.network/"
    	},
    	{
    		"prefix": 1222,
    		"network": "krest",
    		"displayName": "Krest Network",
    		"symbols": [
    			"KREST"
    		],
    		"decimals": [
    			18
    		],
    		"standardAccount": "Sr25519",
    		"website": "https://www.peaq.network/"
    	},
    	{
    		"prefix": 1284,
    		"network": "moonbeam",
    		"displayName": "Moonbeam",
    		"symbols": [
    			"GLMR"
    		],
    		"decimals": [
    			18
    		],
    		"standardAccount": "secp256k1",
    		"website": "https://moonbeam.network"
    	},
    	{
    		"prefix": 1285,
    		"network": "moonriver",
    		"displayName": "Moonriver",
    		"symbols": [
    			"MOVR"
    		],
    		"decimals": [
    			18
    		],
    		"standardAccount": "secp256k1",
    		"website": "https://moonbeam.network"
    	},
    	{
    		"prefix": 1328,
    		"network": "ajuna",
    		"displayName": "Ajuna Network",
    		"symbols": [
    			"AJUN"
    		],
    		"decimals": [
    			12
    		],
    		"standardAccount": "*25519",
    		"website": "https://ajuna.io"
    	},
    	{
    		"prefix": 1337,
    		"network": "bajun",
    		"displayName": "Bajun Network",
    		"symbols": [
    			"BAJU"
    		],
    		"decimals": [
    			12
    		],
    		"standardAccount": "*25519",
    		"website": "https://ajuna.io"
    	},
    	{
    		"prefix": 1516,
    		"network": "societal",
    		"displayName": "Societal",
    		"symbols": [
    			"SCTL"
    		],
    		"decimals": [
    			12
    		],
    		"standardAccount": "*25519",
    		"website": "https://www.sctl.xyz"
    	},
    	{
    		"prefix": 1985,
    		"network": "seals",
    		"displayName": "Seals Network",
    		"symbols": [
    			"SEAL"
    		],
    		"decimals": [
    			9
    		],
    		"standardAccount": "*25519",
    		"website": "https://seals.app"
    	},
    	{
    		"prefix": 2007,
    		"network": "kapex",
    		"displayName": "Kapex",
    		"symbols": [
    			"KAPEX"
    		],
    		"decimals": [
    			12
    		],
    		"standardAccount": "*25519",
    		"website": "https://totemaccounting.com"
    	},
    	{
    		"prefix": 2009,
    		"network": "cloudwalk_mainnet",
    		"displayName": "CloudWalk Network Mainnet",
    		"symbols": [
    			"CWN"
    		],
    		"decimals": [
    			18
    		],
    		"standardAccount": "*25519",
    		"website": "https://explorer.mainnet.cloudwalk.io"
    	},
    	{
    		"prefix": 2021,
    		"network": "logion",
    		"displayName": "logion network",
    		"symbols": [
    			"LGNT"
    		],
    		"decimals": [
    			18
    		],
    		"standardAccount": "*25519",
    		"website": "https://logion.network"
    	},
    	{
    		"prefix": 2024,
    		"network": "vow-chain",
    		"displayName": "Enigmatic Smile",
    		"symbols": [
    			"VOW"
    		],
    		"decimals": [
    			18
    		],
    		"standardAccount": "*25519",
    		"website": "https://www.vow.foundation/"
    	},
    	{
    		"prefix": 2032,
    		"network": "interlay",
    		"displayName": "Interlay",
    		"symbols": [
    			"INTR"
    		],
    		"decimals": [
    			10
    		],
    		"standardAccount": "*25519",
    		"website": "https://interlay.io/"
    	},
    	{
    		"prefix": 2092,
    		"network": "kintsugi",
    		"displayName": "Kintsugi",
    		"symbols": [
    			"KINT"
    		],
    		"decimals": [
    			12
    		],
    		"standardAccount": "*25519",
    		"website": "https://interlay.io/"
    	},
    	{
    		"prefix": 2106,
    		"network": "bitgreen",
    		"displayName": "Bitgreen",
    		"symbols": [
    			"BBB"
    		],
    		"decimals": [
    			18
    		],
    		"standardAccount": "*25519",
    		"website": "https://bitgreen.org/"
    	},
    	{
    		"prefix": 2112,
    		"network": "chainflip",
    		"displayName": "Chainflip",
    		"symbols": [
    			"FLIP"
    		],
    		"decimals": [
    			18
    		],
    		"standardAccount": "*25519",
    		"website": "https://chainflip.io/"
    	},
    	{
    		"prefix": 2199,
    		"network": "moonsama",
    		"displayName": "Moonsama",
    		"symbols": [
    			"SAMA"
    		],
    		"decimals": [
    			18
    		],
    		"standardAccount": "secp256k1",
    		"website": "https://moonsama.com"
    	},
    	{
    		"prefix": 2206,
    		"network": "ICE",
    		"displayName": "ICE Network",
    		"symbols": [
    			"ICY"
    		],
    		"decimals": [
    			18
    		],
    		"standardAccount": "*25519",
    		"website": "https://icenetwork.io"
    	},
    	{
    		"prefix": 2207,
    		"network": "SNOW",
    		"displayName": "SNOW: ICE Canary Network",
    		"symbols": [
    			"ICZ"
    		],
    		"decimals": [
    			18
    		],
    		"standardAccount": "*25519",
    		"website": "https://icenetwork.io"
    	},
    	{
    		"prefix": 2254,
    		"network": "subspace_testnet",
    		"displayName": "Subspace testnet",
    		"symbols": [
    			"tSSC"
    		],
    		"decimals": [
    			18
    		],
    		"standardAccount": "*25519",
    		"website": "https://subspace.network"
    	},
    	{
    		"prefix": 3333,
    		"network": "peerplays",
    		"displayName": "Peerplays",
    		"symbols": [
    			"PPY"
    		],
    		"decimals": [
    			18
    		],
    		"standardAccount": "secp256k1",
    		"website": "https://www.peerplays.com/"
    	},
    	{
    		"prefix": 4450,
    		"network": "g1",
    		"displayName": "Ğ1",
    		"symbols": [
    			"G1"
    		],
    		"decimals": [
    			2
    		],
    		"standardAccount": "*25519",
    		"website": "https://duniter.org"
    	},
    	{
    		"prefix": 5234,
    		"network": "humanode",
    		"displayName": "Humanode Network",
    		"symbols": [
    			"HMND"
    		],
    		"decimals": [
    			18
    		],
    		"standardAccount": "*25519",
    		"website": "https://humanode.io"
    	},
    	{
    		"prefix": 5845,
    		"network": "tangle",
    		"displayName": "Tangle Network",
    		"symbols": [
    			"TNT"
    		],
    		"decimals": [
    			18
    		],
    		"standardAccount": "*25519",
    		"website": "https://www.tangle.tools/"
    	},
    	{
    		"prefix": 6094,
    		"network": "autonomys",
    		"displayName": "Autonomys",
    		"symbols": [
    			"AI3"
    		],
    		"decimals": [
    			18
    		],
    		"standardAccount": "*25519",
    		"website": "https://autonomys.xyz"
    	},
    	{
    		"prefix": 7007,
    		"network": "tidefi",
    		"displayName": "Tidefi",
    		"symbols": [
    			"TDFY"
    		],
    		"decimals": [
    			12
    		],
    		"standardAccount": "*25519",
    		"website": "https://tidefi.com"
    	},
    	{
    		"prefix": 7013,
    		"network": "gm",
    		"displayName": "GM",
    		"symbols": [
    			"FREN",
    			"GM",
    			"GN"
    		],
    		"decimals": [
    			12,
    			0,
    			0
    		],
    		"standardAccount": "*25519",
    		"website": "https://gmordie.com"
    	},
    	{
    		"prefix": 7306,
    		"network": "krigan",
    		"displayName": "Krigan Network",
    		"symbols": [
    			"KRGN"
    		],
    		"decimals": [
    			9
    		],
    		"standardAccount": "*25519",
    		"website": "https://krigan.network"
    	},
    	{
    		"prefix": 7391,
    		"network": "unique_mainnet",
    		"displayName": "Unique Network",
    		"symbols": [
    			"UNQ"
    		],
    		"decimals": [
    			18
    		],
    		"standardAccount": "*25519",
    		"website": "https://unique.network"
    	},
    	{
    		"prefix": 8866,
    		"network": "golden_gate",
    		"displayName": "Golden Gate",
    		"symbols": [
    			"GGX"
    		],
    		"decimals": [
    			18
    		],
    		"standardAccount": "*25519",
    		"website": "https://ggxchain.io/"
    	},
    	{
    		"prefix": 8883,
    		"network": "sapphire_mainnet",
    		"displayName": "Sapphire by Unique",
    		"symbols": [
    			"QTZ"
    		],
    		"decimals": [
    			18
    		],
    		"standardAccount": "*25519",
    		"website": "https://unique.network"
    	},
    	{
    		"prefix": 8886,
    		"network": "golden_gate_sydney",
    		"displayName": "Golden Gate Sydney",
    		"symbols": [
    			"GGXT"
    		],
    		"decimals": [
    			18
    		],
    		"standardAccount": "*25519",
    		"website": "https://ggxchain.io/"
    	},
    	{
    		"prefix": 9072,
    		"network": "hashed",
    		"displayName": "Hashed Network",
    		"symbols": [
    			"HASH"
    		],
    		"decimals": [
    			18
    		],
    		"standardAccount": "*25519",
    		"website": "https://hashed.network"
    	},
    	{
    		"prefix": 9807,
    		"network": "dentnet",
    		"displayName": "DENTNet",
    		"symbols": [
    			"DENTX"
    		],
    		"decimals": [
    			18
    		],
    		"standardAccount": "*25519",
    		"website": "https://www.dentnet.io"
    	},
    	{
    		"prefix": 9935,
    		"network": "t3rn",
    		"displayName": "t3rn",
    		"symbols": [
    			"TRN"
    		],
    		"decimals": [
    			12
    		],
    		"standardAccount": "*25519",
    		"website": "https://t3rn.io/"
    	},
    	{
    		"prefix": 10041,
    		"network": "basilisk",
    		"displayName": "Basilisk",
    		"symbols": [
    			"BSX"
    		],
    		"decimals": [
    			12
    		],
    		"standardAccount": "*25519",
    		"website": "https://bsx.fi"
    	},
    	{
    		"prefix": 11330,
    		"network": "cess-testnet",
    		"displayName": "CESS Testnet",
    		"symbols": [
    			"TCESS"
    		],
    		"decimals": [
    			18
    		],
    		"standardAccount": "*25519",
    		"website": "https://cess.cloud"
    	},
    	{
    		"prefix": 11331,
    		"network": "cess",
    		"displayName": "CESS",
    		"symbols": [
    			"CESS"
    		],
    		"decimals": [
    			18
    		],
    		"standardAccount": "*25519",
    		"website": "https://cess.cloud"
    	},
    	{
    		"prefix": 11486,
    		"network": "luhn",
    		"displayName": "Luhn Network",
    		"symbols": [
    			"LUHN"
    		],
    		"decimals": [
    			18
    		],
    		"standardAccount": "*25519",
    		"website": "https://luhn.network"
    	},
    	{
    		"prefix": 11820,
    		"network": "contextfree",
    		"displayName": "Automata ContextFree",
    		"symbols": [
    			"CTX"
    		],
    		"decimals": [
    			18
    		],
    		"standardAccount": "*25519",
    		"website": "https://ata.network"
    	},
    	{
    		"prefix": 12155,
    		"network": "impact",
    		"displayName": "Impact Protocol Network",
    		"symbols": [
    			"BSTY"
    		],
    		"decimals": [
    			18
    		],
    		"standardAccount": "*25519",
    		"website": "https://impactprotocol.network/"
    	},
    	{
    		"prefix": 12191,
    		"network": "nftmart",
    		"displayName": "NFTMart",
    		"symbols": [
    			"NMT"
    		],
    		"decimals": [
    			12
    		],
    		"standardAccount": "*25519",
    		"website": "https://nftmart.io"
    	},
    	{
    		"prefix": 12850,
    		"network": "analog-timechain",
    		"displayName": "Analog Timechain",
    		"symbols": [
    			"ANLOG"
    		],
    		"decimals": [
    			12
    		],
    		"standardAccount": "*25519",
    		"website": "https://analog.one"
    	},
    	{
    		"prefix": 13116,
    		"network": "bittensor",
    		"displayName": "Bittensor",
    		"symbols": [
    			"TAO"
    		],
    		"decimals": [
    			9
    		],
    		"standardAccount": "*25519",
    		"website": "https://bittensor.com"
    	},
    	{
    		"prefix": 14697,
    		"network": "goro",
    		"displayName": "GORO Network",
    		"symbols": [
    			"GORO"
    		],
    		"decimals": [
    			9
    		],
    		"standardAccount": "*25519",
    		"website": "https://goro.network"
    	},
    	{
    		"prefix": 14998,
    		"network": "mosaic-chain",
    		"displayName": "Mosaic Chain",
    		"symbols": [
    			"MOS"
    		],
    		"decimals": [
    			18
    		],
    		"standardAccount": "*25519",
    		"website": "https://mosaicchain.io"
    	},
    	{
    		"prefix": 29972,
    		"network": "mythos",
    		"displayName": "Mythos",
    		"symbols": [
    			"MYTH"
    		],
    		"decimals": [
    			18
    		],
    		"standardAccount": "secp256k1",
    		"website": "https://mythos.foundation"
    	},
    	{
    		"prefix": 8888,
    		"network": "xcavate",
    		"displayName": "Xcavate Protocol",
    		"symbols": [
    			"XCAV"
    		],
    		"decimals": [
    			12
    		],
    		"standardAccount": "*25519",
    		"website": "https://xcavate.io/"
    	}
    ];

    const knownGenesis = {
        acala: [
            '0xfc41b9bd8ef8fe53d58c7ea67c794c7ec9a73daf05e6d54b14ff6342c99ba64c'
        ],
        ajuna: [
            '0xe358eb1d11b31255a286c12e44fe6780b7edb171d657905a97e39f71d9c6c3ee'
        ],
        'aleph-node': [
            '0x70255b4d28de0fc4e1a193d7e175ad1ccef431598211c55538f1018651a0344e'
        ],
        astar: [
            '0x9eb76c5184c4ab8679d2d5d819fdf90b9c001403e9e17da2e14b6d8aec4029c6'
        ],
        basilisk: [
            '0xa85cfb9b9fd4d622a5b28289a02347af987d8f73fa3108450e2b4a11c1ce5755'
        ],
        bifrost: [
            '0x262e1b2ad728475fd6fe88e62d34c200abe6fd693931ddad144059b1eb884e5b'
        ],
        'bifrost-kusama': [
            '0x9f28c6a68e0fc9646eff64935684f6eeeece527e37bbe1f213d22caa1d9d6bed'
        ],
        bittensor: [
            '0x2f0555cc76fc2840a25a6ea3b9637146806f1f44b090c175ffde2a7e5ab36c03'
        ],
        centrifuge: [
            '0xb3db41421702df9a7fcac62b53ffeac85f7853cc4e689e0b93aeb3db18c09d82',
            '0x67dddf2673b69e5f875f6f25277495834398eafd67f492e09f3f3345e003d1b5'
        ],
        cere: [
            '0x81443836a9a24caaa23f1241897d1235717535711d1d3fe24eae4fdc942c092c'
        ],
        composable: [
            '0xdaab8df776eb52ec604a5df5d388bb62a050a0aaec4556a64265b9d42755552d'
        ],
        creditcoin3: [
            '0x4436a7d64e363df85e065a894721002a86643283f9707338bf195d360ba2ee71',
            '0xfc4ec97a1c1f119c4353aecb4a17c7c0cf7b40d5d660143d8bad9117e9866572',
            '0xfc9df99a665f964aed6649f275055e54df5e3420489538ed31d7788f53d11ef6'
        ],
        darwinia: [
            '0xe71578b37a7c799b0ab4ee87ffa6f059a6b98f71f06fb8c84a8d88013a548ad6'
        ],
        dentnet: [
            '0x0313f6a011d128d22f996703cbab05162e2fdc9e031493314fe6db79979c5ca7'
        ],
        'dock-mainnet': [
            '0x6bfe24dca2a3be10f22212678ac13a6446ec764103c0f3471c71609eac384aae',
            '0xf73467c6544aa68df2ee546b135f955c46b90fa627e9b5d7935f41061bb8a5a9'
        ],
        edgeware: [
            '0x742a2ca70c2fda6cee4f8df98d64c4c670a052d9568058982dad9d5a7a135c5b'
        ],
        encointer: [
            '0x7dd99936c1e9e6d1ce7d90eb6f33bea8393b4bf87677d675aa63c9cb3e8c5b5b'
        ],
        enjin: [
            '0xd8761d3c88f26dc12875c00d3165f7d67243d56fc85b4cf19937601a7916e5a9'
        ],
        equilibrium: [
            '0x6f1a800de3daff7f5e037ddf66ab22ce03ab91874debeddb1086f5f7dbd48925'
        ],
        frequency: [
            '0x4a587bf17a404e3572747add7aab7bbe56e805a5479c6c436f07f36fcc8d3ae1'
        ],
        genshiro: [
            '0x9b8cefc0eb5c568b527998bdd76c184e2b76ae561be76e4667072230217ea243'
        ],
        hydradx: [
            '0xafdc188f45c71dacbaa0b62e16a91f726c7b8699a9748cdf715459de6b7f366d',
            '0xd2a620c27ec5cbc5621ff9a522689895074f7cca0d08e7134a7804e1a3ba86fc',
            '0x10af6e84234477d84dc572bac0789813b254aa490767ed06fb9591191d1073f9',
            '0x3d75507dd46301767e601265791da1d9cb47b6ebc94e87347b635e5bf58bd047',
            '0x0ed32bfcab4a83517fac88f2aa7cbc2f88d3ab93be9a12b6188a036bf8a943c2'
        ],
        integritee: [
            '0xcdedc8eadbfa209d3f207bba541e57c3c58a667b05a2e1d1e86353c9000758da',
            '0xe13e7af377c64e83f95e0d70d5e5c3c01d697a84538776c5b9bbe0e7d7b6034c'
        ],
        'interlay-parachain': [
            '0xbf88efe70e9e0e916416e8bed61f2b45717f517d7f3523e33c7b001e5ffcbc72'
        ],
        karura: [
            '0xbaf5aabe40646d11f0ee8abbdc64f4a4b7674925cba08e4a05ff9ebed6e2126b'
        ],
        khala: [
            '0xd43540ba6d3eb4897c28a77d48cb5b729fea37603cbbfc7a86a73b72adb3be8d'
        ],
        kulupu: [
            '0xf7a99d3cb92853d00d5275c971c132c074636256583fee53b3bbe60d7b8769ba'
        ],
        kusama: [
            '0xb0a8d493285c2df73290dfb7e61f870f17b41801197a149ca93654499ea3dafe',
            '0xe3777fa922cafbff200cadeaea1a76bd7898ad5b89f7848999058b50e715f636',
            '0x3fd7b9eb6a00376e5be61f01abb429ffb0b104be05eaff4d458da48fcd425baf'
        ],
        liberland: [
            '0x6bd89e052d67a45bb60a9a23e8581053d5e0d619f15cb9865946937e690c42d6'
        ],
        matrixchain: [
            '0x3af4ff48ec76d2efc8476730f423ac07e25ad48f5f4c9dc39c778b164d808615'
        ],
        mythos: [
            '0xf6ee56e9c5277df5b4ce6ae9983ee88f3cbed27d31beeb98f9f84f997a1ab0b9'
        ],
        nodle: [
            '0x97da7ede98d7bad4e36b4d734b6055425a3be036da2a332ea5a7037656427a21'
        ],
        origintrail: [
            '0xe7e0962324a3b86c83404dbea483f25fb5dab4c224791c81b756cfc948006174'
        ],
        p3d: [
            '0x6c5894837ad89b6d92b114a2fb3eafa8fe3d26a54848e3447015442cd6ef4e66'
        ],
        parallel: [
            '0xe61a41c53f5dcd0beb09df93b34402aada44cb05117b71059cce40a2723a4e97'
        ],
        peaq: [
            '0xd2a5d385932d1f650dae03ef8e2748983779ee342c614f80854d32b8cd8fa48c'
        ],
        pendulum: [
            '0x5d3c298622d5634ed019bf61ea4b71655030015bde9beb0d6a24743714462c86'
        ],
        phala: [
            '0x1bb969d85965e4bb5a651abbedf21a54b6b31a21f66b5401cc3f1e286268d736'
        ],
        picasso: [
            '0x6811a339673c9daa897944dcdac99c6e2939cc88245ed21951a0a3c9a2be75bc',
            '0xe8e7f0f4c4f5a00720b4821dbfddefea7490bcf0b19009961cc46957984e2c1c'
        ],
        polimec: [
            '0x7eb9354488318e7549c722669dcbdcdc526f1fef1420e7944667212f3601fdbd'
        ],
        polkadex: [
            '0x3920bcb4960a1eef5580cd5367ff3f430eef052774f78468852f7b9cb39f8a3c'
        ],
        polkadot: [
            '0x91b171bb158e2d3848fa23a9f1c25182fb8e20313b2c1eb49219da7a70ce90c3'
        ],
        polymesh: [
            '0x6fbd74e5e1d0a61d52ccfe9d4adaed16dd3a7caa37c6bc4d0c2fa12e8b2f4063'
        ],
        quartz: [
            '0xcd4d732201ebe5d6b014edda071c4203e16867305332301dc8d092044b28e554'
        ],
        rococo: [
            '0x6408de7737c59c238890533af25896a2c20608d8b380bb01029acb392781063e',
            '0xaaf2cd1b74b5f726895921259421b534124726263982522174147046b8827897',
            '0x037f5f3c8e67b314062025fc886fcd6238ea25a4a9b45dce8d246815c9ebe770',
            '0xc196f81260cf1686172b47a79cf002120735d7cb0eb1474e8adce56618456fff',
            '0xf6e9983c37baf68846fedafe21e56718790e39fb1c582abc408b81bc7b208f9a',
            '0x5fce687da39305dfe682b117f0820b319348e8bb37eb16cf34acbf6a202de9d9',
            '0xe7c3d5edde7db964317cd9b51a3a059d7cd99f81bdbce14990047354334c9779',
            '0x1611e1dbf0405379b861e2e27daa90f480b2e6d3682414a80835a52e8cb8a215',
            '0x343442f12fa715489a8714e79a7b264ea88c0d5b8c66b684a7788a516032f6b9',
            '0x78bcd530c6b3a068bc17473cf5d2aff9c287102bed9af3ae3c41c33b9d6c6147',
            '0x47381ee0697153d64404fc578392c8fd5cba9073391908f46c888498415647bd',
            '0x19c0e4fa8ab75f5ac7865e0b8f74ff91eb9a100d336f423cd013a8befba40299'
        ],
        sora: [
            '0x7e4e32d0feafd4f9c9414b0be86373f9a1efa904809b683453a9af6856d38ad5'
        ],
        stafi: [
            '0x290a4149f09ea0e402c74c1c7e96ae4239588577fe78932f94f5404c68243d80'
        ],
        statemine: [
            '0x48239ef607d7928874027a43a67689209727dfb3d3dc5e5b03a39bdc2eda771a'
        ],
        statemint: [
            '0x68d56f15f85d3136970ec16946040bc1752654e906147f7e43e9d539d7c3de2f'
        ],
        subsocial: [
            '0x0bd72c1c305172e1275278aaeb3f161e02eccb7a819e63f62d47bd53a28189f8'
        ],
        ternoa: [
            '0x6859c81ca95ef624c9dfe4dc6e3381c33e5d6509e35e147092bfbc780f777c4e'
        ],
        unique: [
            '0x84322d9cddbf35088f1e54e9a85c967a41a56a4f43445768125e61af166c7d31'
        ],
        vara: [
            '0xfe1b4c55fd4d668101126434206571a7838a8b6b93a6d1b95d607e78e6c53763'
        ],
        vtb: [
            '0x286bc8414c7000ce1d6ee6a834e29a54c1784814b76243eb77ed0b2c5573c60f',
            '0x7483b89572fb2bd687c7b9a93b242d0b237f9aba463aba07ec24503931038aaa'
        ],
        westend: [
            '0xe143f23803ac50e8f6f8e62695d1ce9e4e1d68aa36c1cd2cfd15340213f3423e'
        ],
        xxnetwork: [
            '0x50dd5d206917bf10502c68fb4d18a59fc8aa31586f4e8856b493e43544aa82aa'
        ],
        zeitgeist: [
            '0x1bf2a2ecb4a868de66ea8610f2ce7c8c43706561b6476031315f6640fe38e060'
        ]
    };

    const knownIcon = {
        centrifuge: 'polkadot',
        kusama: 'polkadot',
        polkadot: 'polkadot',
        sora: 'polkadot',
        statemine: 'polkadot',
        statemint: 'polkadot',
        westmint: 'polkadot'
    };

    const knownLedger = {
        acala: 0x00000313,
        ajuna: 0x00000162,
        'aleph-node': 0x00000283,
        astar: 0x0000032a,
        bifrost: 0x00000314,
        'bifrost-kusama': 0x00000314,
        bittensor: 0x00000162,
        centrifuge: 0x000002eb,
        composable: 0x00000162,
        creditcoin3: 0x00000162,
        darwinia: 0x00000162,
        dentnet: 0x000002de,
        'dock-mainnet': 0x00000252,
        edgeware: 0x0000020b,
        encointer: 0x000001b2,
        enjin: 0x00000483,
        equilibrium: 0x05f5e0fd,
        frequency: 0x0000082b,
        genshiro: 0x05f5e0fc,
        hydradx: 0x00000162,
        integritee: 0x000007df,
        'interlay-parachain': 0x00000162,
        karura: 0x000002ae,
        khala: 0x000001b2,
        kusama: 0x000001b2,
        liberland: 0x000002ff,
        matrixchain: 0x00000483,
        mythos: 0x0000003c,
        nodle: 0x000003eb,
        origintrail: 0x00000162,
        parallel: 0x00000162,
        peaq: 0x00000d0a,
        pendulum: 0x00000162,
        phala: 0x00000162,
        picasso: 0x000001b2,
        polimec: 0x00000d10,
        polkadex: 0x0000031f,
        polkadot: 0x00000162,
        polymesh: 0x00000253,
        quartz: 0x00000277,
        sora: 0x00000269,
        stafi: 0x0000038b,
        statemine: 0x000001b2,
        statemint: 0x00000162,
        ternoa: 0x00003e3,
        unique: 0x00000295,
        vara: 0x00001370,
        vtb: 0x000002b6,
        xxnetwork: 0x000007a3,
        zeitgeist: 0x00000162
    };

    const knownTestnet = {
        '': true,
        'cess-testnet': true,
        'dock-testnet': true,
        jupiter: true,
        'mathchain-testnet': true,
        p3dt: true,
        subspace_testnet: true,
        'zero-alphaville': true
    };

    const UNSORTED = [0, 2, 42];
    const TESTNETS = ['testnet'];
    function toExpanded(o) {
        const network = o.network || '';
        const nameParts = network.replace(/_/g, '-').split('-');
        const n = o;
        n.slip44 = knownLedger[network];
        n.hasLedgerSupport = !!n.slip44;
        n.genesisHash = knownGenesis[network] || [];
        n.icon = knownIcon[network] || 'substrate';
        n.isTestnet = !!knownTestnet[network] || TESTNETS.includes(nameParts[nameParts.length - 1]);
        n.isIgnored = n.isTestnet || (!(o.standardAccount &&
            o.decimals?.length &&
            o.symbols?.length) &&
            o.prefix !== 42);
        return n;
    }
    function filterSelectable({ genesisHash, prefix }) {
        return !!genesisHash.length || prefix === 42;
    }
    function filterAvailable(n) {
        return !n.isIgnored && !!n.network;
    }
    function sortNetworks(a, b) {
        const isUnSortedA = UNSORTED.includes(a.prefix);
        const isUnSortedB = UNSORTED.includes(b.prefix);
        return isUnSortedA === isUnSortedB
            ? isUnSortedA
                ? 0
                : a.displayName.localeCompare(b.displayName)
            : isUnSortedA
                ? -1
                : 1;
    }
    const allNetworks = knownSubstrate.map(toExpanded);
    const availableNetworks = allNetworks.filter(filterAvailable).sort(sortNetworks);
    const selectableNetworks = availableNetworks.filter(filterSelectable);

    const defaults = {
        allowedDecodedLengths: [1, 2, 4, 8, 32, 33],
        allowedEncodedLengths: [3, 4, 6, 10, 35, 36, 37, 38],
        allowedPrefix: availableNetworks.map(({ prefix }) => prefix),
        prefix: 42
    };

    function decodeAddress(encoded, ignoreChecksum, ss58Format = -1) {
        if (!encoded) {
            throw new Error('Invalid empty address passed');
        }
        if (util.isU8a(encoded) || util.isHex(encoded)) {
            return util.u8aToU8a(encoded);
        }
        try {
            const decoded = base58Decode(encoded);
            if (!defaults.allowedEncodedLengths.includes(decoded.length)) {
                throw new Error('Invalid decoded address length');
            }
            const [isValid, endPos, ss58Length, ss58Decoded] = checkAddressChecksum(decoded);
            if (!isValid && !ignoreChecksum) {
                throw new Error('Invalid decoded address checksum');
            }
            else if (ss58Format !== -1 && ss58Format !== ss58Decoded) {
                throw new Error(`Expected ss58Format ${ss58Format}, received ${ss58Decoded}`);
            }
            return decoded.slice(ss58Length, endPos);
        }
        catch (error) {
            throw new Error(`Decoding ${encoded}: ${error.message}`);
        }
    }

    function addressToEvm(address, ignoreChecksum) {
        return decodeAddress(address, ignoreChecksum).subarray(0, 20);
    }

    function checkAddress(address, prefix) {
        let decoded;
        try {
            decoded = base58Decode(address);
        }
        catch (error) {
            return [false, error.message];
        }
        const [isValid, , , ss58Decoded] = checkAddressChecksum(decoded);
        if (ss58Decoded !== prefix) {
            return [false, `Prefix mismatch, expected ${prefix}, found ${ss58Decoded}`];
        }
        else if (!defaults.allowedEncodedLengths.includes(decoded.length)) {
            return [false, 'Invalid decoded address length'];
        }
        return [isValid, isValid ? null : 'Invalid decoded address checksum'];
    }

    const BN_BE_OPTS = { isLe: false };
    const BN_LE_OPTS = { isLe: true };
    const BN_LE_16_OPTS = { bitLength: 16, isLe: true };
    const BN_BE_32_OPTS = { bitLength: 32, isLe: false };
    const BN_LE_32_OPTS = { bitLength: 32, isLe: true };
    const BN_BE_256_OPTS = { bitLength: 256, isLe: false };
    const BN_LE_256_OPTS = { bitLength: 256, isLe: true };
    const BN_LE_512_OPTS = { bitLength: 512, isLe: true };

    const RE_NUMBER = /^\d+$/;
    const JUNCTION_ID_LEN = 32;
    class DeriveJunction {
        #chainCode = new Uint8Array(32);
        #isHard = false;
        static from(value) {
            const result = new DeriveJunction();
            const [code, isHard] = value.startsWith('/')
                ? [value.substring(1), true]
                : [value, false];
            result.soft(RE_NUMBER.test(code)
                ? new util.BN(code, 10)
                : code);
            return isHard
                ? result.harden()
                : result;
        }
        get chainCode() {
            return this.#chainCode;
        }
        get isHard() {
            return this.#isHard;
        }
        get isSoft() {
            return !this.#isHard;
        }
        hard(value) {
            return this.soft(value).harden();
        }
        harden() {
            this.#isHard = true;
            return this;
        }
        soft(value) {
            if (util.isNumber(value) || util.isBn(value) || util.isBigInt(value)) {
                return this.soft(util.bnToU8a(value, BN_LE_256_OPTS));
            }
            else if (util.isHex(value)) {
                return this.soft(util.hexToU8a(value));
            }
            else if (util.isString(value)) {
                return this.soft(util.compactAddLength(util.stringToU8a(value)));
            }
            else if (value.length > JUNCTION_ID_LEN) {
                return this.soft(blake2AsU8a(value));
            }
            this.#chainCode.fill(0);
            this.#chainCode.set(value, 0);
            return this;
        }
        soften() {
            this.#isHard = false;
            return this;
        }
    }

    const RE_JUNCTION = /\/(\/?)([^/]+)/g;
    function keyExtractPath(derivePath) {
        const parts = derivePath.match(RE_JUNCTION);
        const path = [];
        let constructed = '';
        if (parts) {
            constructed = parts.join('');
            for (const p of parts) {
                path.push(DeriveJunction.from(p.substring(1)));
            }
        }
        if (constructed !== derivePath) {
            throw new Error(`Re-constructed path "${constructed}" does not match input`);
        }
        return {
            parts,
            path
        };
    }

    const RE_CAPTURE = /^((0x[a-fA-F0-9]+|[\p{L}\d]+(?: [\p{L}\d]+)*))((\/\/?[^/]+)*)(\/\/\/(.*))?$/u;
    function keyExtractSuri(suri) {
        const normalizedSuri = suri.normalize('NFC');
        const matches = normalizedSuri.match(RE_CAPTURE);
        if (matches === null) {
            throw new Error('Unable to match provided value to a secret URI');
        }
        const [, phrase, , derivePath, , , password] = matches;
        const { path } = keyExtractPath(derivePath);
        return {
            derivePath,
            password,
            path,
            phrase
        };
    }

    const HDKD$2 = util.compactAddLength(util.stringToU8a('Secp256k1HDKD'));
    function secp256k1DeriveHard(seed, chainCode) {
        if (!util.isU8a(chainCode) || chainCode.length !== 32) {
            throw new Error('Invalid chainCode passed to derive');
        }
        return blake2AsU8a(util.u8aConcat(HDKD$2, seed, chainCode), 256);
    }

    function setBigUint64$1(view, byteOffset, value, isLE) {
        if (typeof view.setBigUint64 === 'function')
            return view.setBigUint64(byteOffset, value, isLE);
        const _32n = BigInt(32);
        const _u32_max = BigInt(0xffffffff);
        const wh = Number((value >> _32n) & _u32_max);
        const wl = Number(value & _u32_max);
        const h = isLE ? 4 : 0;
        const l = isLE ? 0 : 4;
        view.setUint32(byteOffset + h, wh, isLE);
        view.setUint32(byteOffset + l, wl, isLE);
    }
    class SHA2 extends Hash$1 {
        constructor(blockLen, outputLen, padOffset, isLE) {
            super();
            this.blockLen = blockLen;
            this.outputLen = outputLen;
            this.padOffset = padOffset;
            this.isLE = isLE;
            this.finished = false;
            this.length = 0;
            this.pos = 0;
            this.destroyed = false;
            this.buffer = new Uint8Array(blockLen);
            this.view = createView$1(this.buffer);
        }
        update(data) {
            exists(this);
            const { view, buffer, blockLen } = this;
            data = toBytes$1(data);
            const len = data.length;
            for (let pos = 0; pos < len;) {
                const take = Math.min(blockLen - this.pos, len - pos);
                if (take === blockLen) {
                    const dataView = createView$1(data);
                    for (; blockLen <= len - pos; pos += blockLen)
                        this.process(dataView, pos);
                    continue;
                }
                buffer.set(data.subarray(pos, pos + take), this.pos);
                this.pos += take;
                pos += take;
                if (this.pos === blockLen) {
                    this.process(view, 0);
                    this.pos = 0;
                }
            }
            this.length += data.length;
            this.roundClean();
            return this;
        }
        digestInto(out) {
            exists(this);
            output(out, this);
            this.finished = true;
            const { buffer, view, blockLen, isLE } = this;
            let { pos } = this;
            buffer[pos++] = 0b10000000;
            this.buffer.subarray(pos).fill(0);
            if (this.padOffset > blockLen - pos) {
                this.process(view, 0);
                pos = 0;
            }
            for (let i = pos; i < blockLen; i++)
                buffer[i] = 0;
            setBigUint64$1(view, blockLen - 8, BigInt(this.length * 8), isLE);
            this.process(view, 0);
            const oview = createView$1(out);
            const len = this.outputLen;
            if (len % 4)
                throw new Error('_sha2: outputLen should be aligned to 32bit');
            const outLen = len / 4;
            const state = this.get();
            if (outLen > state.length)
                throw new Error('_sha2: outputLen bigger than state');
            for (let i = 0; i < outLen; i++)
                oview.setUint32(4 * i, state[i], isLE);
        }
        digest() {
            const { buffer, outputLen } = this;
            this.digestInto(buffer);
            const res = buffer.slice(0, outputLen);
            this.destroy();
            return res;
        }
        _cloneInto(to) {
            to || (to = new this.constructor());
            to.set(...this.get());
            const { blockLen, buffer, length, finished, destroyed, pos } = this;
            to.length = length;
            to.pos = pos;
            to.finished = finished;
            to.destroyed = destroyed;
            if (length % blockLen)
                to.buffer.set(buffer);
            return to;
        }
    }

    const Chi$1 = (a, b, c) => (a & b) ^ (~a & c);
    const Maj$1 = (a, b, c) => (a & b) ^ (a & c) ^ (b & c);
    const SHA256_K$1 =  new Uint32Array([
        0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4, 0xab1c5ed5,
        0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe, 0x9bdc06a7, 0xc19bf174,
        0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f, 0x4a7484aa, 0x5cb0a9dc, 0x76f988da,
        0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7, 0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967,
        0x27b70a85, 0x2e1b2138, 0x4d2c6dfc, 0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85,
        0xa2bfe8a1, 0xa81a664b, 0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070,
        0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
        0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7, 0xc67178f2
    ]);
    const IV =  new Uint32Array([
        0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab, 0x5be0cd19
    ]);
    const SHA256_W$1 =  new Uint32Array(64);
    let SHA256$1 = class SHA256 extends SHA2 {
        constructor() {
            super(64, 32, 8, false);
            this.A = IV[0] | 0;
            this.B = IV[1] | 0;
            this.C = IV[2] | 0;
            this.D = IV[3] | 0;
            this.E = IV[4] | 0;
            this.F = IV[5] | 0;
            this.G = IV[6] | 0;
            this.H = IV[7] | 0;
        }
        get() {
            const { A, B, C, D, E, F, G, H } = this;
            return [A, B, C, D, E, F, G, H];
        }
        set(A, B, C, D, E, F, G, H) {
            this.A = A | 0;
            this.B = B | 0;
            this.C = C | 0;
            this.D = D | 0;
            this.E = E | 0;
            this.F = F | 0;
            this.G = G | 0;
            this.H = H | 0;
        }
        process(view, offset) {
            for (let i = 0; i < 16; i++, offset += 4)
                SHA256_W$1[i] = view.getUint32(offset, false);
            for (let i = 16; i < 64; i++) {
                const W15 = SHA256_W$1[i - 15];
                const W2 = SHA256_W$1[i - 2];
                const s0 = rotr$1(W15, 7) ^ rotr$1(W15, 18) ^ (W15 >>> 3);
                const s1 = rotr$1(W2, 17) ^ rotr$1(W2, 19) ^ (W2 >>> 10);
                SHA256_W$1[i] = (s1 + SHA256_W$1[i - 7] + s0 + SHA256_W$1[i - 16]) | 0;
            }
            let { A, B, C, D, E, F, G, H } = this;
            for (let i = 0; i < 64; i++) {
                const sigma1 = rotr$1(E, 6) ^ rotr$1(E, 11) ^ rotr$1(E, 25);
                const T1 = (H + sigma1 + Chi$1(E, F, G) + SHA256_K$1[i] + SHA256_W$1[i]) | 0;
                const sigma0 = rotr$1(A, 2) ^ rotr$1(A, 13) ^ rotr$1(A, 22);
                const T2 = (sigma0 + Maj$1(A, B, C)) | 0;
                H = G;
                G = F;
                F = E;
                E = (D + T1) | 0;
                D = C;
                C = B;
                B = A;
                A = (T1 + T2) | 0;
            }
            A = (A + this.A) | 0;
            B = (B + this.B) | 0;
            C = (C + this.C) | 0;
            D = (D + this.D) | 0;
            E = (E + this.E) | 0;
            F = (F + this.F) | 0;
            G = (G + this.G) | 0;
            H = (H + this.H) | 0;
            this.set(A, B, C, D, E, F, G, H);
        }
        roundClean() {
            SHA256_W$1.fill(0);
        }
        destroy() {
            this.set(0, 0, 0, 0, 0, 0, 0, 0);
            this.buffer.fill(0);
        }
    };
    let SHA224$1 = class SHA224 extends SHA256$1 {
        constructor() {
            super();
            this.A = 0xc1059ed8 | 0;
            this.B = 0x367cd507 | 0;
            this.C = 0x3070dd17 | 0;
            this.D = 0xf70e5939 | 0;
            this.E = 0xffc00b31 | 0;
            this.F = 0x68581511 | 0;
            this.G = 0x64f98fa7 | 0;
            this.H = 0xbefa4fa4 | 0;
            this.outputLen = 28;
        }
    };
    const sha256 =  wrapConstructor(() => new SHA256$1());
    wrapConstructor(() => new SHA224$1());

    /*! noble-curves - MIT License (c) 2022 Paul Miller (paulmillr.com) */
    const _0n$g = BigInt(0);
    const _1n$f = BigInt(1);
    const _2n$b = BigInt(2);
    function isBytes$2(a) {
        return (a instanceof Uint8Array ||
            (a != null && typeof a === 'object' && a.constructor.name === 'Uint8Array'));
    }
    const hexes$1 =  Array.from({ length: 256 }, (_, i) => i.toString(16).padStart(2, '0'));
    function bytesToHex$1(bytes) {
        if (!isBytes$2(bytes))
            throw new Error('Uint8Array expected');
        let hex = '';
        for (let i = 0; i < bytes.length; i++) {
            hex += hexes$1[bytes[i]];
        }
        return hex;
    }
    function numberToHexUnpadded(num) {
        const hex = num.toString(16);
        return hex.length & 1 ? `0${hex}` : hex;
    }
    function hexToNumber$1(hex) {
        if (typeof hex !== 'string')
            throw new Error('hex string expected, got ' + typeof hex);
        return BigInt(hex === '' ? '0' : `0x${hex}`);
    }
    const asciis$1 = { _0: 48, _9: 57, _A: 65, _F: 70, _a: 97, _f: 102 };
    function asciiToBase16$1(char) {
        if (char >= asciis$1._0 && char <= asciis$1._9)
            return char - asciis$1._0;
        if (char >= asciis$1._A && char <= asciis$1._F)
            return char - (asciis$1._A - 10);
        if (char >= asciis$1._a && char <= asciis$1._f)
            return char - (asciis$1._a - 10);
        return;
    }
    function hexToBytes$1(hex) {
        if (typeof hex !== 'string')
            throw new Error('hex string expected, got ' + typeof hex);
        const hl = hex.length;
        const al = hl / 2;
        if (hl % 2)
            throw new Error('padded hex string expected, got unpadded hex of length ' + hl);
        const array = new Uint8Array(al);
        for (let ai = 0, hi = 0; ai < al; ai++, hi += 2) {
            const n1 = asciiToBase16$1(hex.charCodeAt(hi));
            const n2 = asciiToBase16$1(hex.charCodeAt(hi + 1));
            if (n1 === undefined || n2 === undefined) {
                const char = hex[hi] + hex[hi + 1];
                throw new Error('hex string expected, got non-hex character "' + char + '" at index ' + hi);
            }
            array[ai] = n1 * 16 + n2;
        }
        return array;
    }
    function bytesToNumberBE$1(bytes) {
        return hexToNumber$1(bytesToHex$1(bytes));
    }
    function bytesToNumberLE$2(bytes) {
        if (!isBytes$2(bytes))
            throw new Error('Uint8Array expected');
        return hexToNumber$1(bytesToHex$1(Uint8Array.from(bytes).reverse()));
    }
    function numberToBytesBE$1(n, len) {
        return hexToBytes$1(n.toString(16).padStart(len * 2, '0'));
    }
    function numberToBytesLE$2(n, len) {
        return numberToBytesBE$1(n, len).reverse();
    }
    function numberToVarBytesBE(n) {
        return hexToBytes$1(numberToHexUnpadded(n));
    }
    function ensureBytes$1(title, hex, expectedLength) {
        let res;
        if (typeof hex === 'string') {
            try {
                res = hexToBytes$1(hex);
            }
            catch (e) {
                throw new Error(`${title} must be valid hex string, got "${hex}". Cause: ${e}`);
            }
        }
        else if (isBytes$2(hex)) {
            res = Uint8Array.from(hex);
        }
        else {
            throw new Error(`${title} must be hex string or Uint8Array`);
        }
        const len = res.length;
        if (typeof expectedLength === 'number' && len !== expectedLength)
            throw new Error(`${title} expected ${expectedLength} bytes, got ${len}`);
        return res;
    }
    function concatBytes$1(...arrays) {
        let sum = 0;
        for (let i = 0; i < arrays.length; i++) {
            const a = arrays[i];
            if (!isBytes$2(a))
                throw new Error('Uint8Array expected');
            sum += a.length;
        }
        let res = new Uint8Array(sum);
        let pad = 0;
        for (let i = 0; i < arrays.length; i++) {
            const a = arrays[i];
            res.set(a, pad);
            pad += a.length;
        }
        return res;
    }
    function equalBytes$1(a, b) {
        if (a.length !== b.length)
            return false;
        let diff = 0;
        for (let i = 0; i < a.length; i++)
            diff |= a[i] ^ b[i];
        return diff === 0;
    }
    function utf8ToBytes$1(str) {
        if (typeof str !== 'string')
            throw new Error(`utf8ToBytes expected string, got ${typeof str}`);
        return new Uint8Array(new TextEncoder().encode(str));
    }
    function bitLen$1(n) {
        let len;
        for (len = 0; n > _0n$g; n >>= _1n$f, len += 1)
            ;
        return len;
    }
    function bitGet(n, pos) {
        return (n >> BigInt(pos)) & _1n$f;
    }
    const bitSet = (n, pos, value) => {
        return n | ((value ? _1n$f : _0n$g) << BigInt(pos));
    };
    const bitMask$2 = (n) => (_2n$b << BigInt(n - 1)) - _1n$f;
    const u8n = (data) => new Uint8Array(data);
    const u8fr = (arr) => Uint8Array.from(arr);
    function createHmacDrbg(hashLen, qByteLen, hmacFn) {
        if (typeof hashLen !== 'number' || hashLen < 2)
            throw new Error('hashLen must be a number');
        if (typeof qByteLen !== 'number' || qByteLen < 2)
            throw new Error('qByteLen must be a number');
        if (typeof hmacFn !== 'function')
            throw new Error('hmacFn must be a function');
        let v = u8n(hashLen);
        let k = u8n(hashLen);
        let i = 0;
        const reset = () => {
            v.fill(1);
            k.fill(0);
            i = 0;
        };
        const h = (...b) => hmacFn(k, v, ...b);
        const reseed = (seed = u8n()) => {
            k = h(u8fr([0x00]), seed);
            v = h();
            if (seed.length === 0)
                return;
            k = h(u8fr([0x01]), seed);
            v = h();
        };
        const gen = () => {
            if (i++ >= 1000)
                throw new Error('drbg: tried 1000 values');
            let len = 0;
            const out = [];
            while (len < qByteLen) {
                v = h();
                const sl = v.slice();
                out.push(sl);
                len += v.length;
            }
            return concatBytes$1(...out);
        };
        const genUntil = (seed, pred) => {
            reset();
            reseed(seed);
            let res = undefined;
            while (!(res = pred(gen())))
                reseed();
            reset();
            return res;
        };
        return genUntil;
    }
    const validatorFns = {
        bigint: (val) => typeof val === 'bigint',
        function: (val) => typeof val === 'function',
        boolean: (val) => typeof val === 'boolean',
        string: (val) => typeof val === 'string',
        stringOrUint8Array: (val) => typeof val === 'string' || isBytes$2(val),
        isSafeInteger: (val) => Number.isSafeInteger(val),
        array: (val) => Array.isArray(val),
        field: (val, object) => object.Fp.isValid(val),
        hash: (val) => typeof val === 'function' && Number.isSafeInteger(val.outputLen),
    };
    function validateObject(object, validators, optValidators = {}) {
        const checkField = (fieldName, type, isOptional) => {
            const checkVal = validatorFns[type];
            if (typeof checkVal !== 'function')
                throw new Error(`Invalid validator "${type}", expected function`);
            const val = object[fieldName];
            if (isOptional && val === undefined)
                return;
            if (!checkVal(val, object)) {
                throw new Error(`Invalid param ${String(fieldName)}=${val} (${typeof val}), expected ${type}`);
            }
        };
        for (const [fieldName, type] of Object.entries(validators))
            checkField(fieldName, type, false);
        for (const [fieldName, type] of Object.entries(optValidators))
            checkField(fieldName, type, true);
        return object;
    }

    const ut = /*#__PURE__*/Object.freeze({
        __proto__: null,
        bitGet: bitGet,
        bitLen: bitLen$1,
        bitMask: bitMask$2,
        bitSet: bitSet,
        bytesToHex: bytesToHex$1,
        bytesToNumberBE: bytesToNumberBE$1,
        bytesToNumberLE: bytesToNumberLE$2,
        concatBytes: concatBytes$1,
        createHmacDrbg: createHmacDrbg,
        ensureBytes: ensureBytes$1,
        equalBytes: equalBytes$1,
        hexToBytes: hexToBytes$1,
        hexToNumber: hexToNumber$1,
        isBytes: isBytes$2,
        numberToBytesBE: numberToBytesBE$1,
        numberToBytesLE: numberToBytesLE$2,
        numberToHexUnpadded: numberToHexUnpadded,
        numberToVarBytesBE: numberToVarBytesBE,
        utf8ToBytes: utf8ToBytes$1,
        validateObject: validateObject
    });

    /*! noble-curves - MIT License (c) 2022 Paul Miller (paulmillr.com) */
    const _0n$f = BigInt(0), _1n$e = BigInt(1), _2n$a = BigInt(2), _3n$4 = BigInt(3);
    const _4n$2 = BigInt(4), _5n$3 = BigInt(5), _8n$4 = BigInt(8);
    BigInt(9); BigInt(16);
    function mod$1(a, b) {
        const result = a % b;
        return result >= _0n$f ? result : b + result;
    }
    function pow(num, power, modulo) {
        if (modulo <= _0n$f || power < _0n$f)
            throw new Error('Expected power/modulo > 0');
        if (modulo === _1n$e)
            return _0n$f;
        let res = _1n$e;
        while (power > _0n$f) {
            if (power & _1n$e)
                res = (res * num) % modulo;
            num = (num * num) % modulo;
            power >>= _1n$e;
        }
        return res;
    }
    function pow2$1(x, power, modulo) {
        let res = x;
        while (power-- > _0n$f) {
            res *= res;
            res %= modulo;
        }
        return res;
    }
    function invert$1(number, modulo) {
        if (number === _0n$f || modulo <= _0n$f) {
            throw new Error(`invert: expected positive integers, got n=${number} mod=${modulo}`);
        }
        let a = mod$1(number, modulo);
        let b = modulo;
        let x = _0n$f, u = _1n$e;
        while (a !== _0n$f) {
            const q = b / a;
            const r = b % a;
            const m = x - u * q;
            b = a, a = r, x = u, u = m;
        }
        const gcd = b;
        if (gcd !== _1n$e)
            throw new Error('invert: does not exist');
        return mod$1(x, modulo);
    }
    function tonelliShanks$1(P) {
        const legendreC = (P - _1n$e) / _2n$a;
        let Q, S, Z;
        for (Q = P - _1n$e, S = 0; Q % _2n$a === _0n$f; Q /= _2n$a, S++)
            ;
        for (Z = _2n$a; Z < P && pow(Z, legendreC, P) !== P - _1n$e; Z++)
            ;
        if (S === 1) {
            const p1div4 = (P + _1n$e) / _4n$2;
            return function tonelliFast(Fp, n) {
                const root = Fp.pow(n, p1div4);
                if (!Fp.eql(Fp.sqr(root), n))
                    throw new Error('Cannot find square root');
                return root;
            };
        }
        const Q1div2 = (Q + _1n$e) / _2n$a;
        return function tonelliSlow(Fp, n) {
            if (Fp.pow(n, legendreC) === Fp.neg(Fp.ONE))
                throw new Error('Cannot find square root');
            let r = S;
            let g = Fp.pow(Fp.mul(Fp.ONE, Z), Q);
            let x = Fp.pow(n, Q1div2);
            let b = Fp.pow(n, Q);
            while (!Fp.eql(b, Fp.ONE)) {
                if (Fp.eql(b, Fp.ZERO))
                    return Fp.ZERO;
                let m = 1;
                for (let t2 = Fp.sqr(b); m < r; m++) {
                    if (Fp.eql(t2, Fp.ONE))
                        break;
                    t2 = Fp.sqr(t2);
                }
                const ge = Fp.pow(g, _1n$e << BigInt(r - m - 1));
                g = Fp.sqr(ge);
                x = Fp.mul(x, ge);
                b = Fp.mul(b, g);
                r = m;
            }
            return x;
        };
    }
    function FpSqrt$1(P) {
        if (P % _4n$2 === _3n$4) {
            const p1div4 = (P + _1n$e) / _4n$2;
            return function sqrt3mod4(Fp, n) {
                const root = Fp.pow(n, p1div4);
                if (!Fp.eql(Fp.sqr(root), n))
                    throw new Error('Cannot find square root');
                return root;
            };
        }
        if (P % _8n$4 === _5n$3) {
            const c1 = (P - _5n$3) / _8n$4;
            return function sqrt5mod8(Fp, n) {
                const n2 = Fp.mul(n, _2n$a);
                const v = Fp.pow(n2, c1);
                const nv = Fp.mul(n, v);
                const i = Fp.mul(Fp.mul(nv, _2n$a), v);
                const root = Fp.mul(nv, Fp.sub(i, Fp.ONE));
                if (!Fp.eql(Fp.sqr(root), n))
                    throw new Error('Cannot find square root');
                return root;
            };
        }
        return tonelliShanks$1(P);
    }
    const isNegativeLE$1 = (num, modulo) => (mod$1(num, modulo) & _1n$e) === _1n$e;
    const FIELD_FIELDS$1 = [
        'create', 'isValid', 'is0', 'neg', 'inv', 'sqrt', 'sqr',
        'eql', 'add', 'sub', 'mul', 'pow', 'div',
        'addN', 'subN', 'mulN', 'sqrN'
    ];
    function validateField$1(field) {
        const initial = {
            ORDER: 'bigint',
            MASK: 'bigint',
            BYTES: 'isSafeInteger',
            BITS: 'isSafeInteger',
        };
        const opts = FIELD_FIELDS$1.reduce((map, val) => {
            map[val] = 'function';
            return map;
        }, initial);
        return validateObject(field, opts);
    }
    function FpPow$1(f, num, power) {
        if (power < _0n$f)
            throw new Error('Expected power > 0');
        if (power === _0n$f)
            return f.ONE;
        if (power === _1n$e)
            return num;
        let p = f.ONE;
        let d = num;
        while (power > _0n$f) {
            if (power & _1n$e)
                p = f.mul(p, d);
            d = f.sqr(d);
            power >>= _1n$e;
        }
        return p;
    }
    function FpInvertBatch$1(f, nums) {
        const tmp = new Array(nums.length);
        const lastMultiplied = nums.reduce((acc, num, i) => {
            if (f.is0(num))
                return acc;
            tmp[i] = acc;
            return f.mul(acc, num);
        }, f.ONE);
        const inverted = f.inv(lastMultiplied);
        nums.reduceRight((acc, num, i) => {
            if (f.is0(num))
                return acc;
            tmp[i] = f.mul(acc, tmp[i]);
            return f.mul(acc, num);
        }, inverted);
        return tmp;
    }
    function nLength$1(n, nBitLength) {
        const _nBitLength = nBitLength !== undefined ? nBitLength : n.toString(2).length;
        const nByteLength = Math.ceil(_nBitLength / 8);
        return { nBitLength: _nBitLength, nByteLength };
    }
    function Field$1(ORDER, bitLen, isLE = false, redef = {}) {
        if (ORDER <= _0n$f)
            throw new Error(`Expected Field ORDER > 0, got ${ORDER}`);
        const { nBitLength: BITS, nByteLength: BYTES } = nLength$1(ORDER, bitLen);
        if (BYTES > 2048)
            throw new Error('Field lengths over 2048 bytes are not supported');
        const sqrtP = FpSqrt$1(ORDER);
        const f = Object.freeze({
            ORDER,
            BITS,
            BYTES,
            MASK: bitMask$2(BITS),
            ZERO: _0n$f,
            ONE: _1n$e,
            create: (num) => mod$1(num, ORDER),
            isValid: (num) => {
                if (typeof num !== 'bigint')
                    throw new Error(`Invalid field element: expected bigint, got ${typeof num}`);
                return _0n$f <= num && num < ORDER;
            },
            is0: (num) => num === _0n$f,
            isOdd: (num) => (num & _1n$e) === _1n$e,
            neg: (num) => mod$1(-num, ORDER),
            eql: (lhs, rhs) => lhs === rhs,
            sqr: (num) => mod$1(num * num, ORDER),
            add: (lhs, rhs) => mod$1(lhs + rhs, ORDER),
            sub: (lhs, rhs) => mod$1(lhs - rhs, ORDER),
            mul: (lhs, rhs) => mod$1(lhs * rhs, ORDER),
            pow: (num, power) => FpPow$1(f, num, power),
            div: (lhs, rhs) => mod$1(lhs * invert$1(rhs, ORDER), ORDER),
            sqrN: (num) => num * num,
            addN: (lhs, rhs) => lhs + rhs,
            subN: (lhs, rhs) => lhs - rhs,
            mulN: (lhs, rhs) => lhs * rhs,
            inv: (num) => invert$1(num, ORDER),
            sqrt: redef.sqrt || ((n) => sqrtP(f, n)),
            invertBatch: (lst) => FpInvertBatch$1(f, lst),
            cmov: (a, b, c) => (c ? b : a),
            toBytes: (num) => (isLE ? numberToBytesLE$2(num, BYTES) : numberToBytesBE$1(num, BYTES)),
            fromBytes: (bytes) => {
                if (bytes.length !== BYTES)
                    throw new Error(`Fp.fromBytes: expected ${BYTES}, got ${bytes.length}`);
                return isLE ? bytesToNumberLE$2(bytes) : bytesToNumberBE$1(bytes);
            },
        });
        return Object.freeze(f);
    }
    function FpSqrtEven$1(Fp, elm) {
        if (!Fp.isOdd)
            throw new Error(`Field doesn't have isOdd`);
        const root = Fp.sqrt(elm);
        return Fp.isOdd(root) ? Fp.neg(root) : root;
    }
    function getFieldBytesLength(fieldOrder) {
        if (typeof fieldOrder !== 'bigint')
            throw new Error('field order must be bigint');
        const bitLength = fieldOrder.toString(2).length;
        return Math.ceil(bitLength / 8);
    }
    function getMinHashLength(fieldOrder) {
        const length = getFieldBytesLength(fieldOrder);
        return length + Math.ceil(length / 2);
    }
    function mapHashToField(key, fieldOrder, isLE = false) {
        const len = key.length;
        const fieldLen = getFieldBytesLength(fieldOrder);
        const minLen = getMinHashLength(fieldOrder);
        if (len < 16 || len < minLen || len > 1024)
            throw new Error(`expected ${minLen}-1024 bytes of input, got ${len}`);
        const num = isLE ? bytesToNumberBE$1(key) : bytesToNumberLE$2(key);
        const reduced = mod$1(num, fieldOrder - _1n$e) + _1n$e;
        return isLE ? numberToBytesLE$2(reduced, fieldLen) : numberToBytesBE$1(reduced, fieldLen);
    }

    /*! noble-curves - MIT License (c) 2022 Paul Miller (paulmillr.com) */
    const _0n$e = BigInt(0);
    const _1n$d = BigInt(1);
    function wNAF$1(c, bits) {
        const constTimeNegate = (condition, item) => {
            const neg = item.negate();
            return condition ? neg : item;
        };
        const opts = (W) => {
            const windows = Math.ceil(bits / W) + 1;
            const windowSize = 2 ** (W - 1);
            return { windows, windowSize };
        };
        return {
            constTimeNegate,
            unsafeLadder(elm, n) {
                let p = c.ZERO;
                let d = elm;
                while (n > _0n$e) {
                    if (n & _1n$d)
                        p = p.add(d);
                    d = d.double();
                    n >>= _1n$d;
                }
                return p;
            },
            precomputeWindow(elm, W) {
                const { windows, windowSize } = opts(W);
                const points = [];
                let p = elm;
                let base = p;
                for (let window = 0; window < windows; window++) {
                    base = p;
                    points.push(base);
                    for (let i = 1; i < windowSize; i++) {
                        base = base.add(p);
                        points.push(base);
                    }
                    p = base.double();
                }
                return points;
            },
            wNAF(W, precomputes, n) {
                const { windows, windowSize } = opts(W);
                let p = c.ZERO;
                let f = c.BASE;
                const mask = BigInt(2 ** W - 1);
                const maxNumber = 2 ** W;
                const shiftBy = BigInt(W);
                for (let window = 0; window < windows; window++) {
                    const offset = window * windowSize;
                    let wbits = Number(n & mask);
                    n >>= shiftBy;
                    if (wbits > windowSize) {
                        wbits -= maxNumber;
                        n += _1n$d;
                    }
                    const offset1 = offset;
                    const offset2 = offset + Math.abs(wbits) - 1;
                    const cond1 = window % 2 !== 0;
                    const cond2 = wbits < 0;
                    if (wbits === 0) {
                        f = f.add(constTimeNegate(cond1, precomputes[offset1]));
                    }
                    else {
                        p = p.add(constTimeNegate(cond2, precomputes[offset2]));
                    }
                }
                return { p, f };
            },
            wNAFCached(P, precomputesMap, n, transform) {
                const W = P._WINDOW_SIZE || 1;
                let comp = precomputesMap.get(P);
                if (!comp) {
                    comp = this.precomputeWindow(P, W);
                    if (W !== 1) {
                        precomputesMap.set(P, transform(comp));
                    }
                }
                return this.wNAF(W, comp, n);
            },
        };
    }
    function validateBasic(curve) {
        validateField$1(curve.Fp);
        validateObject(curve, {
            n: 'bigint',
            h: 'bigint',
            Gx: 'field',
            Gy: 'field',
        }, {
            nBitLength: 'isSafeInteger',
            nByteLength: 'isSafeInteger',
        });
        return Object.freeze({
            ...nLength$1(curve.n, curve.nBitLength),
            ...curve,
            ...{ p: curve.Fp.ORDER },
        });
    }

    /*! noble-curves - MIT License (c) 2022 Paul Miller (paulmillr.com) */
    function validatePointOpts(curve) {
        const opts = validateBasic(curve);
        validateObject(opts, {
            a: 'field',
            b: 'field',
        }, {
            allowedPrivateKeyLengths: 'array',
            wrapPrivateKey: 'boolean',
            isTorsionFree: 'function',
            clearCofactor: 'function',
            allowInfinityPoint: 'boolean',
            fromBytes: 'function',
            toBytes: 'function',
        });
        const { endo, Fp, a } = opts;
        if (endo) {
            if (!Fp.eql(a, Fp.ZERO)) {
                throw new Error('Endomorphism can only be defined for Koblitz curves that have a=0');
            }
            if (typeof endo !== 'object' ||
                typeof endo.beta !== 'bigint' ||
                typeof endo.splitScalar !== 'function') {
                throw new Error('Expected endomorphism with beta: bigint and splitScalar: function');
            }
        }
        return Object.freeze({ ...opts });
    }
    const { bytesToNumberBE: b2n, hexToBytes: h2b } = ut;
    const DER = {
        Err: class DERErr extends Error {
            constructor(m = '') {
                super(m);
            }
        },
        _parseInt(data) {
            const { Err: E } = DER;
            if (data.length < 2 || data[0] !== 0x02)
                throw new E('Invalid signature integer tag');
            const len = data[1];
            const res = data.subarray(2, len + 2);
            if (!len || res.length !== len)
                throw new E('Invalid signature integer: wrong length');
            if (res[0] & 0b10000000)
                throw new E('Invalid signature integer: negative');
            if (res[0] === 0x00 && !(res[1] & 0b10000000))
                throw new E('Invalid signature integer: unnecessary leading zero');
            return { d: b2n(res), l: data.subarray(len + 2) };
        },
        toSig(hex) {
            const { Err: E } = DER;
            const data = typeof hex === 'string' ? h2b(hex) : hex;
            if (!isBytes$2(data))
                throw new Error('ui8a expected');
            let l = data.length;
            if (l < 2 || data[0] != 0x30)
                throw new E('Invalid signature tag');
            if (data[1] !== l - 2)
                throw new E('Invalid signature: incorrect length');
            const { d: r, l: sBytes } = DER._parseInt(data.subarray(2));
            const { d: s, l: rBytesLeft } = DER._parseInt(sBytes);
            if (rBytesLeft.length)
                throw new E('Invalid signature: left bytes after parsing');
            return { r, s };
        },
        hexFromSig(sig) {
            const slice = (s) => (Number.parseInt(s[0], 16) & 0b1000 ? '00' + s : s);
            const h = (num) => {
                const hex = num.toString(16);
                return hex.length & 1 ? `0${hex}` : hex;
            };
            const s = slice(h(sig.s));
            const r = slice(h(sig.r));
            const shl = s.length / 2;
            const rhl = r.length / 2;
            const sl = h(shl);
            const rl = h(rhl);
            return `30${h(rhl + shl + 4)}02${rl}${r}02${sl}${s}`;
        },
    };
    const _0n$d = BigInt(0), _1n$c = BigInt(1), _2n$9 = BigInt(2), _3n$3 = BigInt(3), _4n$1 = BigInt(4);
    function weierstrassPoints(opts) {
        const CURVE = validatePointOpts(opts);
        const { Fp } = CURVE;
        const toBytes = CURVE.toBytes ||
            ((_c, point, _isCompressed) => {
                const a = point.toAffine();
                return concatBytes$1(Uint8Array.from([0x04]), Fp.toBytes(a.x), Fp.toBytes(a.y));
            });
        const fromBytes = CURVE.fromBytes ||
            ((bytes) => {
                const tail = bytes.subarray(1);
                const x = Fp.fromBytes(tail.subarray(0, Fp.BYTES));
                const y = Fp.fromBytes(tail.subarray(Fp.BYTES, 2 * Fp.BYTES));
                return { x, y };
            });
        function weierstrassEquation(x) {
            const { a, b } = CURVE;
            const x2 = Fp.sqr(x);
            const x3 = Fp.mul(x2, x);
            return Fp.add(Fp.add(x3, Fp.mul(x, a)), b);
        }
        if (!Fp.eql(Fp.sqr(CURVE.Gy), weierstrassEquation(CURVE.Gx)))
            throw new Error('bad generator point: equation left != right');
        function isWithinCurveOrder(num) {
            return typeof num === 'bigint' && _0n$d < num && num < CURVE.n;
        }
        function assertGE(num) {
            if (!isWithinCurveOrder(num))
                throw new Error('Expected valid bigint: 0 < bigint < curve.n');
        }
        function normPrivateKeyToScalar(key) {
            const { allowedPrivateKeyLengths: lengths, nByteLength, wrapPrivateKey, n } = CURVE;
            if (lengths && typeof key !== 'bigint') {
                if (isBytes$2(key))
                    key = bytesToHex$1(key);
                if (typeof key !== 'string' || !lengths.includes(key.length))
                    throw new Error('Invalid key');
                key = key.padStart(nByteLength * 2, '0');
            }
            let num;
            try {
                num =
                    typeof key === 'bigint'
                        ? key
                        : bytesToNumberBE$1(ensureBytes$1('private key', key, nByteLength));
            }
            catch (error) {
                throw new Error(`private key must be ${nByteLength} bytes, hex or bigint, not ${typeof key}`);
            }
            if (wrapPrivateKey)
                num = mod$1(num, n);
            assertGE(num);
            return num;
        }
        const pointPrecomputes = new Map();
        function assertPrjPoint(other) {
            if (!(other instanceof Point))
                throw new Error('ProjectivePoint expected');
        }
        class Point {
            constructor(px, py, pz) {
                this.px = px;
                this.py = py;
                this.pz = pz;
                if (px == null || !Fp.isValid(px))
                    throw new Error('x required');
                if (py == null || !Fp.isValid(py))
                    throw new Error('y required');
                if (pz == null || !Fp.isValid(pz))
                    throw new Error('z required');
            }
            static fromAffine(p) {
                const { x, y } = p || {};
                if (!p || !Fp.isValid(x) || !Fp.isValid(y))
                    throw new Error('invalid affine point');
                if (p instanceof Point)
                    throw new Error('projective point not allowed');
                const is0 = (i) => Fp.eql(i, Fp.ZERO);
                if (is0(x) && is0(y))
                    return Point.ZERO;
                return new Point(x, y, Fp.ONE);
            }
            get x() {
                return this.toAffine().x;
            }
            get y() {
                return this.toAffine().y;
            }
            static normalizeZ(points) {
                const toInv = Fp.invertBatch(points.map((p) => p.pz));
                return points.map((p, i) => p.toAffine(toInv[i])).map(Point.fromAffine);
            }
            static fromHex(hex) {
                const P = Point.fromAffine(fromBytes(ensureBytes$1('pointHex', hex)));
                P.assertValidity();
                return P;
            }
            static fromPrivateKey(privateKey) {
                return Point.BASE.multiply(normPrivateKeyToScalar(privateKey));
            }
            _setWindowSize(windowSize) {
                this._WINDOW_SIZE = windowSize;
                pointPrecomputes.delete(this);
            }
            assertValidity() {
                if (this.is0()) {
                    if (CURVE.allowInfinityPoint && !Fp.is0(this.py))
                        return;
                    throw new Error('bad point: ZERO');
                }
                const { x, y } = this.toAffine();
                if (!Fp.isValid(x) || !Fp.isValid(y))
                    throw new Error('bad point: x or y not FE');
                const left = Fp.sqr(y);
                const right = weierstrassEquation(x);
                if (!Fp.eql(left, right))
                    throw new Error('bad point: equation left != right');
                if (!this.isTorsionFree())
                    throw new Error('bad point: not in prime-order subgroup');
            }
            hasEvenY() {
                const { y } = this.toAffine();
                if (Fp.isOdd)
                    return !Fp.isOdd(y);
                throw new Error("Field doesn't support isOdd");
            }
            equals(other) {
                assertPrjPoint(other);
                const { px: X1, py: Y1, pz: Z1 } = this;
                const { px: X2, py: Y2, pz: Z2 } = other;
                const U1 = Fp.eql(Fp.mul(X1, Z2), Fp.mul(X2, Z1));
                const U2 = Fp.eql(Fp.mul(Y1, Z2), Fp.mul(Y2, Z1));
                return U1 && U2;
            }
            negate() {
                return new Point(this.px, Fp.neg(this.py), this.pz);
            }
            double() {
                const { a, b } = CURVE;
                const b3 = Fp.mul(b, _3n$3);
                const { px: X1, py: Y1, pz: Z1 } = this;
                let X3 = Fp.ZERO, Y3 = Fp.ZERO, Z3 = Fp.ZERO;
                let t0 = Fp.mul(X1, X1);
                let t1 = Fp.mul(Y1, Y1);
                let t2 = Fp.mul(Z1, Z1);
                let t3 = Fp.mul(X1, Y1);
                t3 = Fp.add(t3, t3);
                Z3 = Fp.mul(X1, Z1);
                Z3 = Fp.add(Z3, Z3);
                X3 = Fp.mul(a, Z3);
                Y3 = Fp.mul(b3, t2);
                Y3 = Fp.add(X3, Y3);
                X3 = Fp.sub(t1, Y3);
                Y3 = Fp.add(t1, Y3);
                Y3 = Fp.mul(X3, Y3);
                X3 = Fp.mul(t3, X3);
                Z3 = Fp.mul(b3, Z3);
                t2 = Fp.mul(a, t2);
                t3 = Fp.sub(t0, t2);
                t3 = Fp.mul(a, t3);
                t3 = Fp.add(t3, Z3);
                Z3 = Fp.add(t0, t0);
                t0 = Fp.add(Z3, t0);
                t0 = Fp.add(t0, t2);
                t0 = Fp.mul(t0, t3);
                Y3 = Fp.add(Y3, t0);
                t2 = Fp.mul(Y1, Z1);
                t2 = Fp.add(t2, t2);
                t0 = Fp.mul(t2, t3);
                X3 = Fp.sub(X3, t0);
                Z3 = Fp.mul(t2, t1);
                Z3 = Fp.add(Z3, Z3);
                Z3 = Fp.add(Z3, Z3);
                return new Point(X3, Y3, Z3);
            }
            add(other) {
                assertPrjPoint(other);
                const { px: X1, py: Y1, pz: Z1 } = this;
                const { px: X2, py: Y2, pz: Z2 } = other;
                let X3 = Fp.ZERO, Y3 = Fp.ZERO, Z3 = Fp.ZERO;
                const a = CURVE.a;
                const b3 = Fp.mul(CURVE.b, _3n$3);
                let t0 = Fp.mul(X1, X2);
                let t1 = Fp.mul(Y1, Y2);
                let t2 = Fp.mul(Z1, Z2);
                let t3 = Fp.add(X1, Y1);
                let t4 = Fp.add(X2, Y2);
                t3 = Fp.mul(t3, t4);
                t4 = Fp.add(t0, t1);
                t3 = Fp.sub(t3, t4);
                t4 = Fp.add(X1, Z1);
                let t5 = Fp.add(X2, Z2);
                t4 = Fp.mul(t4, t5);
                t5 = Fp.add(t0, t2);
                t4 = Fp.sub(t4, t5);
                t5 = Fp.add(Y1, Z1);
                X3 = Fp.add(Y2, Z2);
                t5 = Fp.mul(t5, X3);
                X3 = Fp.add(t1, t2);
                t5 = Fp.sub(t5, X3);
                Z3 = Fp.mul(a, t4);
                X3 = Fp.mul(b3, t2);
                Z3 = Fp.add(X3, Z3);
                X3 = Fp.sub(t1, Z3);
                Z3 = Fp.add(t1, Z3);
                Y3 = Fp.mul(X3, Z3);
                t1 = Fp.add(t0, t0);
                t1 = Fp.add(t1, t0);
                t2 = Fp.mul(a, t2);
                t4 = Fp.mul(b3, t4);
                t1 = Fp.add(t1, t2);
                t2 = Fp.sub(t0, t2);
                t2 = Fp.mul(a, t2);
                t4 = Fp.add(t4, t2);
                t0 = Fp.mul(t1, t4);
                Y3 = Fp.add(Y3, t0);
                t0 = Fp.mul(t5, t4);
                X3 = Fp.mul(t3, X3);
                X3 = Fp.sub(X3, t0);
                t0 = Fp.mul(t3, t1);
                Z3 = Fp.mul(t5, Z3);
                Z3 = Fp.add(Z3, t0);
                return new Point(X3, Y3, Z3);
            }
            subtract(other) {
                return this.add(other.negate());
            }
            is0() {
                return this.equals(Point.ZERO);
            }
            wNAF(n) {
                return wnaf.wNAFCached(this, pointPrecomputes, n, (comp) => {
                    const toInv = Fp.invertBatch(comp.map((p) => p.pz));
                    return comp.map((p, i) => p.toAffine(toInv[i])).map(Point.fromAffine);
                });
            }
            multiplyUnsafe(n) {
                const I = Point.ZERO;
                if (n === _0n$d)
                    return I;
                assertGE(n);
                if (n === _1n$c)
                    return this;
                const { endo } = CURVE;
                if (!endo)
                    return wnaf.unsafeLadder(this, n);
                let { k1neg, k1, k2neg, k2 } = endo.splitScalar(n);
                let k1p = I;
                let k2p = I;
                let d = this;
                while (k1 > _0n$d || k2 > _0n$d) {
                    if (k1 & _1n$c)
                        k1p = k1p.add(d);
                    if (k2 & _1n$c)
                        k2p = k2p.add(d);
                    d = d.double();
                    k1 >>= _1n$c;
                    k2 >>= _1n$c;
                }
                if (k1neg)
                    k1p = k1p.negate();
                if (k2neg)
                    k2p = k2p.negate();
                k2p = new Point(Fp.mul(k2p.px, endo.beta), k2p.py, k2p.pz);
                return k1p.add(k2p);
            }
            multiply(scalar) {
                assertGE(scalar);
                let n = scalar;
                let point, fake;
                const { endo } = CURVE;
                if (endo) {
                    const { k1neg, k1, k2neg, k2 } = endo.splitScalar(n);
                    let { p: k1p, f: f1p } = this.wNAF(k1);
                    let { p: k2p, f: f2p } = this.wNAF(k2);
                    k1p = wnaf.constTimeNegate(k1neg, k1p);
                    k2p = wnaf.constTimeNegate(k2neg, k2p);
                    k2p = new Point(Fp.mul(k2p.px, endo.beta), k2p.py, k2p.pz);
                    point = k1p.add(k2p);
                    fake = f1p.add(f2p);
                }
                else {
                    const { p, f } = this.wNAF(n);
                    point = p;
                    fake = f;
                }
                return Point.normalizeZ([point, fake])[0];
            }
            multiplyAndAddUnsafe(Q, a, b) {
                const G = Point.BASE;
                const mul = (P, a
                ) => (a === _0n$d || a === _1n$c || !P.equals(G) ? P.multiplyUnsafe(a) : P.multiply(a));
                const sum = mul(this, a).add(mul(Q, b));
                return sum.is0() ? undefined : sum;
            }
            toAffine(iz) {
                const { px: x, py: y, pz: z } = this;
                const is0 = this.is0();
                if (iz == null)
                    iz = is0 ? Fp.ONE : Fp.inv(z);
                const ax = Fp.mul(x, iz);
                const ay = Fp.mul(y, iz);
                const zz = Fp.mul(z, iz);
                if (is0)
                    return { x: Fp.ZERO, y: Fp.ZERO };
                if (!Fp.eql(zz, Fp.ONE))
                    throw new Error('invZ was invalid');
                return { x: ax, y: ay };
            }
            isTorsionFree() {
                const { h: cofactor, isTorsionFree } = CURVE;
                if (cofactor === _1n$c)
                    return true;
                if (isTorsionFree)
                    return isTorsionFree(Point, this);
                throw new Error('isTorsionFree() has not been declared for the elliptic curve');
            }
            clearCofactor() {
                const { h: cofactor, clearCofactor } = CURVE;
                if (cofactor === _1n$c)
                    return this;
                if (clearCofactor)
                    return clearCofactor(Point, this);
                return this.multiplyUnsafe(CURVE.h);
            }
            toRawBytes(isCompressed = true) {
                this.assertValidity();
                return toBytes(Point, this, isCompressed);
            }
            toHex(isCompressed = true) {
                return bytesToHex$1(this.toRawBytes(isCompressed));
            }
        }
        Point.BASE = new Point(CURVE.Gx, CURVE.Gy, Fp.ONE);
        Point.ZERO = new Point(Fp.ZERO, Fp.ONE, Fp.ZERO);
        const _bits = CURVE.nBitLength;
        const wnaf = wNAF$1(Point, CURVE.endo ? Math.ceil(_bits / 2) : _bits);
        return {
            CURVE,
            ProjectivePoint: Point,
            normPrivateKeyToScalar,
            weierstrassEquation,
            isWithinCurveOrder,
        };
    }
    function validateOpts$3(curve) {
        const opts = validateBasic(curve);
        validateObject(opts, {
            hash: 'hash',
            hmac: 'function',
            randomBytes: 'function',
        }, {
            bits2int: 'function',
            bits2int_modN: 'function',
            lowS: 'boolean',
        });
        return Object.freeze({ lowS: true, ...opts });
    }
    function weierstrass(curveDef) {
        const CURVE = validateOpts$3(curveDef);
        const { Fp, n: CURVE_ORDER } = CURVE;
        const compressedLen = Fp.BYTES + 1;
        const uncompressedLen = 2 * Fp.BYTES + 1;
        function isValidFieldElement(num) {
            return _0n$d < num && num < Fp.ORDER;
        }
        function modN(a) {
            return mod$1(a, CURVE_ORDER);
        }
        function invN(a) {
            return invert$1(a, CURVE_ORDER);
        }
        const { ProjectivePoint: Point, normPrivateKeyToScalar, weierstrassEquation, isWithinCurveOrder, } = weierstrassPoints({
            ...CURVE,
            toBytes(_c, point, isCompressed) {
                const a = point.toAffine();
                const x = Fp.toBytes(a.x);
                const cat = concatBytes$1;
                if (isCompressed) {
                    return cat(Uint8Array.from([point.hasEvenY() ? 0x02 : 0x03]), x);
                }
                else {
                    return cat(Uint8Array.from([0x04]), x, Fp.toBytes(a.y));
                }
            },
            fromBytes(bytes) {
                const len = bytes.length;
                const head = bytes[0];
                const tail = bytes.subarray(1);
                if (len === compressedLen && (head === 0x02 || head === 0x03)) {
                    const x = bytesToNumberBE$1(tail);
                    if (!isValidFieldElement(x))
                        throw new Error('Point is not on curve');
                    const y2 = weierstrassEquation(x);
                    let y = Fp.sqrt(y2);
                    const isYOdd = (y & _1n$c) === _1n$c;
                    const isHeadOdd = (head & 1) === 1;
                    if (isHeadOdd !== isYOdd)
                        y = Fp.neg(y);
                    return { x, y };
                }
                else if (len === uncompressedLen && head === 0x04) {
                    const x = Fp.fromBytes(tail.subarray(0, Fp.BYTES));
                    const y = Fp.fromBytes(tail.subarray(Fp.BYTES, 2 * Fp.BYTES));
                    return { x, y };
                }
                else {
                    throw new Error(`Point of length ${len} was invalid. Expected ${compressedLen} compressed bytes or ${uncompressedLen} uncompressed bytes`);
                }
            },
        });
        const numToNByteStr = (num) => bytesToHex$1(numberToBytesBE$1(num, CURVE.nByteLength));
        function isBiggerThanHalfOrder(number) {
            const HALF = CURVE_ORDER >> _1n$c;
            return number > HALF;
        }
        function normalizeS(s) {
            return isBiggerThanHalfOrder(s) ? modN(-s) : s;
        }
        const slcNum = (b, from, to) => bytesToNumberBE$1(b.slice(from, to));
        class Signature {
            constructor(r, s, recovery) {
                this.r = r;
                this.s = s;
                this.recovery = recovery;
                this.assertValidity();
            }
            static fromCompact(hex) {
                const l = CURVE.nByteLength;
                hex = ensureBytes$1('compactSignature', hex, l * 2);
                return new Signature(slcNum(hex, 0, l), slcNum(hex, l, 2 * l));
            }
            static fromDER(hex) {
                const { r, s } = DER.toSig(ensureBytes$1('DER', hex));
                return new Signature(r, s);
            }
            assertValidity() {
                if (!isWithinCurveOrder(this.r))
                    throw new Error('r must be 0 < r < CURVE.n');
                if (!isWithinCurveOrder(this.s))
                    throw new Error('s must be 0 < s < CURVE.n');
            }
            addRecoveryBit(recovery) {
                return new Signature(this.r, this.s, recovery);
            }
            recoverPublicKey(msgHash) {
                const { r, s, recovery: rec } = this;
                const h = bits2int_modN(ensureBytes$1('msgHash', msgHash));
                if (rec == null || ![0, 1, 2, 3].includes(rec))
                    throw new Error('recovery id invalid');
                const radj = rec === 2 || rec === 3 ? r + CURVE.n : r;
                if (radj >= Fp.ORDER)
                    throw new Error('recovery id 2 or 3 invalid');
                const prefix = (rec & 1) === 0 ? '02' : '03';
                const R = Point.fromHex(prefix + numToNByteStr(radj));
                const ir = invN(radj);
                const u1 = modN(-h * ir);
                const u2 = modN(s * ir);
                const Q = Point.BASE.multiplyAndAddUnsafe(R, u1, u2);
                if (!Q)
                    throw new Error('point at infinify');
                Q.assertValidity();
                return Q;
            }
            hasHighS() {
                return isBiggerThanHalfOrder(this.s);
            }
            normalizeS() {
                return this.hasHighS() ? new Signature(this.r, modN(-this.s), this.recovery) : this;
            }
            toDERRawBytes() {
                return hexToBytes$1(this.toDERHex());
            }
            toDERHex() {
                return DER.hexFromSig({ r: this.r, s: this.s });
            }
            toCompactRawBytes() {
                return hexToBytes$1(this.toCompactHex());
            }
            toCompactHex() {
                return numToNByteStr(this.r) + numToNByteStr(this.s);
            }
        }
        const utils = {
            isValidPrivateKey(privateKey) {
                try {
                    normPrivateKeyToScalar(privateKey);
                    return true;
                }
                catch (error) {
                    return false;
                }
            },
            normPrivateKeyToScalar: normPrivateKeyToScalar,
            randomPrivateKey: () => {
                const length = getMinHashLength(CURVE.n);
                return mapHashToField(CURVE.randomBytes(length), CURVE.n);
            },
            precompute(windowSize = 8, point = Point.BASE) {
                point._setWindowSize(windowSize);
                point.multiply(BigInt(3));
                return point;
            },
        };
        function getPublicKey(privateKey, isCompressed = true) {
            return Point.fromPrivateKey(privateKey).toRawBytes(isCompressed);
        }
        function isProbPub(item) {
            const arr = isBytes$2(item);
            const str = typeof item === 'string';
            const len = (arr || str) && item.length;
            if (arr)
                return len === compressedLen || len === uncompressedLen;
            if (str)
                return len === 2 * compressedLen || len === 2 * uncompressedLen;
            if (item instanceof Point)
                return true;
            return false;
        }
        function getSharedSecret(privateA, publicB, isCompressed = true) {
            if (isProbPub(privateA))
                throw new Error('first arg must be private key');
            if (!isProbPub(publicB))
                throw new Error('second arg must be public key');
            const b = Point.fromHex(publicB);
            return b.multiply(normPrivateKeyToScalar(privateA)).toRawBytes(isCompressed);
        }
        const bits2int = CURVE.bits2int ||
            function (bytes) {
                const num = bytesToNumberBE$1(bytes);
                const delta = bytes.length * 8 - CURVE.nBitLength;
                return delta > 0 ? num >> BigInt(delta) : num;
            };
        const bits2int_modN = CURVE.bits2int_modN ||
            function (bytes) {
                return modN(bits2int(bytes));
            };
        const ORDER_MASK = bitMask$2(CURVE.nBitLength);
        function int2octets(num) {
            if (typeof num !== 'bigint')
                throw new Error('bigint expected');
            if (!(_0n$d <= num && num < ORDER_MASK))
                throw new Error(`bigint expected < 2^${CURVE.nBitLength}`);
            return numberToBytesBE$1(num, CURVE.nByteLength);
        }
        function prepSig(msgHash, privateKey, opts = defaultSigOpts) {
            if (['recovered', 'canonical'].some((k) => k in opts))
                throw new Error('sign() legacy options not supported');
            const { hash, randomBytes } = CURVE;
            let { lowS, prehash, extraEntropy: ent } = opts;
            if (lowS == null)
                lowS = true;
            msgHash = ensureBytes$1('msgHash', msgHash);
            if (prehash)
                msgHash = ensureBytes$1('prehashed msgHash', hash(msgHash));
            const h1int = bits2int_modN(msgHash);
            const d = normPrivateKeyToScalar(privateKey);
            const seedArgs = [int2octets(d), int2octets(h1int)];
            if (ent != null) {
                const e = ent === true ? randomBytes(Fp.BYTES) : ent;
                seedArgs.push(ensureBytes$1('extraEntropy', e));
            }
            const seed = concatBytes$1(...seedArgs);
            const m = h1int;
            function k2sig(kBytes) {
                const k = bits2int(kBytes);
                if (!isWithinCurveOrder(k))
                    return;
                const ik = invN(k);
                const q = Point.BASE.multiply(k).toAffine();
                const r = modN(q.x);
                if (r === _0n$d)
                    return;
                const s = modN(ik * modN(m + r * d));
                if (s === _0n$d)
                    return;
                let recovery = (q.x === r ? 0 : 2) | Number(q.y & _1n$c);
                let normS = s;
                if (lowS && isBiggerThanHalfOrder(s)) {
                    normS = normalizeS(s);
                    recovery ^= 1;
                }
                return new Signature(r, normS, recovery);
            }
            return { seed, k2sig };
        }
        const defaultSigOpts = { lowS: CURVE.lowS, prehash: false };
        const defaultVerOpts = { lowS: CURVE.lowS, prehash: false };
        function sign(msgHash, privKey, opts = defaultSigOpts) {
            const { seed, k2sig } = prepSig(msgHash, privKey, opts);
            const C = CURVE;
            const drbg = createHmacDrbg(C.hash.outputLen, C.nByteLength, C.hmac);
            return drbg(seed, k2sig);
        }
        Point.BASE._setWindowSize(8);
        function verify(signature, msgHash, publicKey, opts = defaultVerOpts) {
            const sg = signature;
            msgHash = ensureBytes$1('msgHash', msgHash);
            publicKey = ensureBytes$1('publicKey', publicKey);
            if ('strict' in opts)
                throw new Error('options.strict was renamed to lowS');
            const { lowS, prehash } = opts;
            let _sig = undefined;
            let P;
            try {
                if (typeof sg === 'string' || isBytes$2(sg)) {
                    try {
                        _sig = Signature.fromDER(sg);
                    }
                    catch (derError) {
                        if (!(derError instanceof DER.Err))
                            throw derError;
                        _sig = Signature.fromCompact(sg);
                    }
                }
                else if (typeof sg === 'object' && typeof sg.r === 'bigint' && typeof sg.s === 'bigint') {
                    const { r, s } = sg;
                    _sig = new Signature(r, s);
                }
                else {
                    throw new Error('PARSE');
                }
                P = Point.fromHex(publicKey);
            }
            catch (error) {
                if (error.message === 'PARSE')
                    throw new Error(`signature must be Signature instance, Uint8Array or hex string`);
                return false;
            }
            if (lowS && _sig.hasHighS())
                return false;
            if (prehash)
                msgHash = CURVE.hash(msgHash);
            const { r, s } = _sig;
            const h = bits2int_modN(msgHash);
            const is = invN(s);
            const u1 = modN(h * is);
            const u2 = modN(r * is);
            const R = Point.BASE.multiplyAndAddUnsafe(P, u1, u2)?.toAffine();
            if (!R)
                return false;
            const v = modN(R.x);
            return v === r;
        }
        return {
            CURVE,
            getPublicKey,
            getSharedSecret,
            sign,
            verify,
            ProjectivePoint: Point,
            Signature,
            utils,
        };
    }
    function SWUFpSqrtRatio(Fp, Z) {
        const q = Fp.ORDER;
        let l = _0n$d;
        for (let o = q - _1n$c; o % _2n$9 === _0n$d; o /= _2n$9)
            l += _1n$c;
        const c1 = l;
        const _2n_pow_c1_1 = _2n$9 << (c1 - _1n$c - _1n$c);
        const _2n_pow_c1 = _2n_pow_c1_1 * _2n$9;
        const c2 = (q - _1n$c) / _2n_pow_c1;
        const c3 = (c2 - _1n$c) / _2n$9;
        const c4 = _2n_pow_c1 - _1n$c;
        const c5 = _2n_pow_c1_1;
        const c6 = Fp.pow(Z, c2);
        const c7 = Fp.pow(Z, (c2 + _1n$c) / _2n$9);
        let sqrtRatio = (u, v) => {
            let tv1 = c6;
            let tv2 = Fp.pow(v, c4);
            let tv3 = Fp.sqr(tv2);
            tv3 = Fp.mul(tv3, v);
            let tv5 = Fp.mul(u, tv3);
            tv5 = Fp.pow(tv5, c3);
            tv5 = Fp.mul(tv5, tv2);
            tv2 = Fp.mul(tv5, v);
            tv3 = Fp.mul(tv5, u);
            let tv4 = Fp.mul(tv3, tv2);
            tv5 = Fp.pow(tv4, c5);
            let isQR = Fp.eql(tv5, Fp.ONE);
            tv2 = Fp.mul(tv3, c7);
            tv5 = Fp.mul(tv4, tv1);
            tv3 = Fp.cmov(tv2, tv3, isQR);
            tv4 = Fp.cmov(tv5, tv4, isQR);
            for (let i = c1; i > _1n$c; i--) {
                let tv5 = i - _2n$9;
                tv5 = _2n$9 << (tv5 - _1n$c);
                let tvv5 = Fp.pow(tv4, tv5);
                const e1 = Fp.eql(tvv5, Fp.ONE);
                tv2 = Fp.mul(tv3, tv1);
                tv1 = Fp.mul(tv1, tv1);
                tvv5 = Fp.mul(tv4, tv1);
                tv3 = Fp.cmov(tv2, tv3, e1);
                tv4 = Fp.cmov(tvv5, tv4, e1);
            }
            return { isValid: isQR, value: tv3 };
        };
        if (Fp.ORDER % _4n$1 === _3n$3) {
            const c1 = (Fp.ORDER - _3n$3) / _4n$1;
            const c2 = Fp.sqrt(Fp.neg(Z));
            sqrtRatio = (u, v) => {
                let tv1 = Fp.sqr(v);
                const tv2 = Fp.mul(u, v);
                tv1 = Fp.mul(tv1, tv2);
                let y1 = Fp.pow(tv1, c1);
                y1 = Fp.mul(y1, tv2);
                const y2 = Fp.mul(y1, c2);
                const tv3 = Fp.mul(Fp.sqr(y1), v);
                const isQR = Fp.eql(tv3, u);
                let y = Fp.cmov(y2, y1, isQR);
                return { isValid: isQR, value: y };
            };
        }
        return sqrtRatio;
    }
    function mapToCurveSimpleSWU(Fp, opts) {
        validateField$1(Fp);
        if (!Fp.isValid(opts.A) || !Fp.isValid(opts.B) || !Fp.isValid(opts.Z))
            throw new Error('mapToCurveSimpleSWU: invalid opts');
        const sqrtRatio = SWUFpSqrtRatio(Fp, opts.Z);
        if (!Fp.isOdd)
            throw new Error('Fp.isOdd is not implemented!');
        return (u) => {
            let tv1, tv2, tv3, tv4, tv5, tv6, x, y;
            tv1 = Fp.sqr(u);
            tv1 = Fp.mul(tv1, opts.Z);
            tv2 = Fp.sqr(tv1);
            tv2 = Fp.add(tv2, tv1);
            tv3 = Fp.add(tv2, Fp.ONE);
            tv3 = Fp.mul(tv3, opts.B);
            tv4 = Fp.cmov(opts.Z, Fp.neg(tv2), !Fp.eql(tv2, Fp.ZERO));
            tv4 = Fp.mul(tv4, opts.A);
            tv2 = Fp.sqr(tv3);
            tv6 = Fp.sqr(tv4);
            tv5 = Fp.mul(tv6, opts.A);
            tv2 = Fp.add(tv2, tv5);
            tv2 = Fp.mul(tv2, tv3);
            tv6 = Fp.mul(tv6, tv4);
            tv5 = Fp.mul(tv6, opts.B);
            tv2 = Fp.add(tv2, tv5);
            x = Fp.mul(tv1, tv3);
            const { isValid, value } = sqrtRatio(tv2, tv6);
            y = Fp.mul(tv1, u);
            y = Fp.mul(y, value);
            x = Fp.cmov(x, tv3, isValid);
            y = Fp.cmov(y, value, isValid);
            const e1 = Fp.isOdd(u) === Fp.isOdd(y);
            y = Fp.cmov(Fp.neg(y), y, e1);
            x = Fp.div(x, tv4);
            return { x, y };
        };
    }

    function validateDST(dst) {
        if (isBytes$2(dst))
            return dst;
        if (typeof dst === 'string')
            return utf8ToBytes$1(dst);
        throw new Error('DST must be Uint8Array or string');
    }
    const os2ip$1 = bytesToNumberBE$1;
    function i2osp$1(value, length) {
        if (value < 0 || value >= 1 << (8 * length)) {
            throw new Error(`bad I2OSP call: value=${value} length=${length}`);
        }
        const res = Array.from({ length }).fill(0);
        for (let i = length - 1; i >= 0; i--) {
            res[i] = value & 0xff;
            value >>>= 8;
        }
        return new Uint8Array(res);
    }
    function strxor$1(a, b) {
        const arr = new Uint8Array(a.length);
        for (let i = 0; i < a.length; i++) {
            arr[i] = a[i] ^ b[i];
        }
        return arr;
    }
    function abytes$2(item) {
        if (!isBytes$2(item))
            throw new Error('Uint8Array expected');
    }
    function isNum(item) {
        if (!Number.isSafeInteger(item))
            throw new Error('number expected');
    }
    function expand_message_xmd$1(msg, DST, lenInBytes, H) {
        abytes$2(msg);
        abytes$2(DST);
        isNum(lenInBytes);
        if (DST.length > 255)
            DST = H(concatBytes$1(utf8ToBytes$1('H2C-OVERSIZE-DST-'), DST));
        const { outputLen: b_in_bytes, blockLen: r_in_bytes } = H;
        const ell = Math.ceil(lenInBytes / b_in_bytes);
        if (ell > 255)
            throw new Error('Invalid xmd length');
        const DST_prime = concatBytes$1(DST, i2osp$1(DST.length, 1));
        const Z_pad = i2osp$1(0, r_in_bytes);
        const l_i_b_str = i2osp$1(lenInBytes, 2);
        const b = new Array(ell);
        const b_0 = H(concatBytes$1(Z_pad, msg, l_i_b_str, i2osp$1(0, 1), DST_prime));
        b[0] = H(concatBytes$1(b_0, i2osp$1(1, 1), DST_prime));
        for (let i = 1; i <= ell; i++) {
            const args = [strxor$1(b_0, b[i - 1]), i2osp$1(i + 1, 1), DST_prime];
            b[i] = H(concatBytes$1(...args));
        }
        const pseudo_random_bytes = concatBytes$1(...b);
        return pseudo_random_bytes.slice(0, lenInBytes);
    }
    function expand_message_xof$1(msg, DST, lenInBytes, k, H) {
        abytes$2(msg);
        abytes$2(DST);
        isNum(lenInBytes);
        if (DST.length > 255) {
            const dkLen = Math.ceil((2 * k) / 8);
            DST = H.create({ dkLen }).update(utf8ToBytes$1('H2C-OVERSIZE-DST-')).update(DST).digest();
        }
        if (lenInBytes > 65535 || DST.length > 255)
            throw new Error('expand_message_xof: invalid lenInBytes');
        return (H.create({ dkLen: lenInBytes })
            .update(msg)
            .update(i2osp$1(lenInBytes, 2))
            .update(DST)
            .update(i2osp$1(DST.length, 1))
            .digest());
    }
    function hash_to_field$1(msg, count, options) {
        validateObject(options, {
            DST: 'stringOrUint8Array',
            p: 'bigint',
            m: 'isSafeInteger',
            k: 'isSafeInteger',
            hash: 'hash',
        });
        const { p, k, m, hash, expand, DST: _DST } = options;
        abytes$2(msg);
        isNum(count);
        const DST = validateDST(_DST);
        const log2p = p.toString(2).length;
        const L = Math.ceil((log2p + k) / 8);
        const len_in_bytes = count * m * L;
        let prb;
        if (expand === 'xmd') {
            prb = expand_message_xmd$1(msg, DST, len_in_bytes, hash);
        }
        else if (expand === 'xof') {
            prb = expand_message_xof$1(msg, DST, len_in_bytes, k, hash);
        }
        else if (expand === '_internal_pass') {
            prb = msg;
        }
        else {
            throw new Error('expand must be "xmd" or "xof"');
        }
        const u = new Array(count);
        for (let i = 0; i < count; i++) {
            const e = new Array(m);
            for (let j = 0; j < m; j++) {
                const elm_offset = L * (j + i * m);
                const tv = prb.subarray(elm_offset, elm_offset + L);
                e[j] = mod$1(os2ip$1(tv), p);
            }
            u[i] = e;
        }
        return u;
    }
    function isogenyMap(field, map) {
        const COEFF = map.map((i) => Array.from(i).reverse());
        return (x, y) => {
            const [xNum, xDen, yNum, yDen] = COEFF.map((val) => val.reduce((acc, i) => field.add(field.mul(acc, x), i)));
            x = field.div(xNum, xDen);
            y = field.mul(y, field.div(yNum, yDen));
            return { x, y };
        };
    }
    function createHasher$2(Point, mapToCurve, def) {
        if (typeof mapToCurve !== 'function')
            throw new Error('mapToCurve() must be defined');
        return {
            hashToCurve(msg, options) {
                const u = hash_to_field$1(msg, 2, { ...def, DST: def.DST, ...options });
                const u0 = Point.fromAffine(mapToCurve(u[0]));
                const u1 = Point.fromAffine(mapToCurve(u[1]));
                const P = u0.add(u1).clearCofactor();
                P.assertValidity();
                return P;
            },
            encodeToCurve(msg, options) {
                const u = hash_to_field$1(msg, 1, { ...def, DST: def.encodeDST, ...options });
                const P = Point.fromAffine(mapToCurve(u[0])).clearCofactor();
                P.assertValidity();
                return P;
            },
        };
    }

    class HMAC extends Hash$1 {
        constructor(hash$1, _key) {
            super();
            this.finished = false;
            this.destroyed = false;
            hash(hash$1);
            const key = toBytes$1(_key);
            this.iHash = hash$1.create();
            if (typeof this.iHash.update !== 'function')
                throw new Error('Expected instance of class which extends utils.Hash');
            this.blockLen = this.iHash.blockLen;
            this.outputLen = this.iHash.outputLen;
            const blockLen = this.blockLen;
            const pad = new Uint8Array(blockLen);
            pad.set(key.length > blockLen ? hash$1.create().update(key).digest() : key);
            for (let i = 0; i < pad.length; i++)
                pad[i] ^= 0x36;
            this.iHash.update(pad);
            this.oHash = hash$1.create();
            for (let i = 0; i < pad.length; i++)
                pad[i] ^= 0x36 ^ 0x5c;
            this.oHash.update(pad);
            pad.fill(0);
        }
        update(buf) {
            exists(this);
            this.iHash.update(buf);
            return this;
        }
        digestInto(out) {
            exists(this);
            bytes(out, this.outputLen);
            this.finished = true;
            this.iHash.digestInto(out);
            this.oHash.update(out);
            this.oHash.digestInto(out);
            this.destroy();
        }
        digest() {
            const out = new Uint8Array(this.oHash.outputLen);
            this.digestInto(out);
            return out;
        }
        _cloneInto(to) {
            to || (to = Object.create(Object.getPrototypeOf(this), {}));
            const { oHash, iHash, finished, destroyed, blockLen, outputLen } = this;
            to = to;
            to.finished = finished;
            to.destroyed = destroyed;
            to.blockLen = blockLen;
            to.outputLen = outputLen;
            to.oHash = oHash._cloneInto(to.oHash);
            to.iHash = iHash._cloneInto(to.iHash);
            return to;
        }
        destroy() {
            this.destroyed = true;
            this.oHash.destroy();
            this.iHash.destroy();
        }
    }
    const hmac = (hash, key, message) => new HMAC(hash, key).update(message).digest();
    hmac.create = (hash, key) => new HMAC(hash, key);

    /*! noble-curves - MIT License (c) 2022 Paul Miller (paulmillr.com) */
    function getHash(hash) {
        return {
            hash,
            hmac: (key, ...msgs) => hmac(hash, key, concatBytes$2(...msgs)),
            randomBytes: randomBytes$1,
        };
    }
    function createCurve(curveDef, defHash) {
        const create = (hash) => weierstrass({ ...curveDef, ...getHash(hash) });
        return Object.freeze({ ...create(defHash), create });
    }

    /*! noble-curves - MIT License (c) 2022 Paul Miller (paulmillr.com) */
    const secp256k1P = BigInt('0xfffffffffffffffffffffffffffffffffffffffffffffffffffffffefffffc2f');
    const secp256k1N = BigInt('0xfffffffffffffffffffffffffffffffebaaedce6af48a03bbfd25e8cd0364141');
    const _1n$b = BigInt(1);
    const _2n$8 = BigInt(2);
    const divNearest = (a, b) => (a + b / _2n$8) / b;
    function sqrtMod(y) {
        const P = secp256k1P;
        const _3n = BigInt(3), _6n = BigInt(6), _11n = BigInt(11), _22n = BigInt(22);
        const _23n = BigInt(23), _44n = BigInt(44), _88n = BigInt(88);
        const b2 = (y * y * y) % P;
        const b3 = (b2 * b2 * y) % P;
        const b6 = (pow2$1(b3, _3n, P) * b3) % P;
        const b9 = (pow2$1(b6, _3n, P) * b3) % P;
        const b11 = (pow2$1(b9, _2n$8, P) * b2) % P;
        const b22 = (pow2$1(b11, _11n, P) * b11) % P;
        const b44 = (pow2$1(b22, _22n, P) * b22) % P;
        const b88 = (pow2$1(b44, _44n, P) * b44) % P;
        const b176 = (pow2$1(b88, _88n, P) * b88) % P;
        const b220 = (pow2$1(b176, _44n, P) * b44) % P;
        const b223 = (pow2$1(b220, _3n, P) * b3) % P;
        const t1 = (pow2$1(b223, _23n, P) * b22) % P;
        const t2 = (pow2$1(t1, _6n, P) * b2) % P;
        const root = pow2$1(t2, _2n$8, P);
        if (!Fp$2.eql(Fp$2.sqr(root), y))
            throw new Error('Cannot find square root');
        return root;
    }
    const Fp$2 = Field$1(secp256k1P, undefined, undefined, { sqrt: sqrtMod });
    const secp256k1 = createCurve({
        a: BigInt(0),
        b: BigInt(7),
        Fp: Fp$2,
        n: secp256k1N,
        Gx: BigInt('55066263022277343669578718895168534326250603453777594175500187360389116729240'),
        Gy: BigInt('32670510020758816978083085130507043184471273380659243275938904335757337482424'),
        h: BigInt(1),
        lowS: true,
        endo: {
            beta: BigInt('0x7ae96a2b657c07106e64479eac3434e99cf0497512f58995c1396c28719501ee'),
            splitScalar: (k) => {
                const n = secp256k1N;
                const a1 = BigInt('0x3086d221a7d46bcde86c90e49284eb15');
                const b1 = -_1n$b * BigInt('0xe4437ed6010e88286f547fa90abfe4c3');
                const a2 = BigInt('0x114ca50f7a8e2f3f657c1108d9d44cfd8');
                const b2 = a1;
                const POW_2_128 = BigInt('0x100000000000000000000000000000000');
                const c1 = divNearest(b2 * k, n);
                const c2 = divNearest(-b1 * k, n);
                let k1 = mod$1(k - c1 * a1 - c2 * a2, n);
                let k2 = mod$1(-c1 * b1 - c2 * b2, n);
                const k1neg = k1 > POW_2_128;
                const k2neg = k2 > POW_2_128;
                if (k1neg)
                    k1 = n - k1;
                if (k2neg)
                    k2 = n - k2;
                if (k1 > POW_2_128 || k2 > POW_2_128) {
                    throw new Error('splitScalar: Endomorphism failed, k=' + k);
                }
                return { k1neg, k1, k2neg, k2 };
            },
        },
    }, sha256);
    const _0n$c = BigInt(0);
    const fe = (x) => typeof x === 'bigint' && _0n$c < x && x < secp256k1P;
    const ge = (x) => typeof x === 'bigint' && _0n$c < x && x < secp256k1N;
    const TAGGED_HASH_PREFIXES = {};
    function taggedHash(tag, ...messages) {
        let tagP = TAGGED_HASH_PREFIXES[tag];
        if (tagP === undefined) {
            const tagH = sha256(Uint8Array.from(tag, (c) => c.charCodeAt(0)));
            tagP = concatBytes$1(tagH, tagH);
            TAGGED_HASH_PREFIXES[tag] = tagP;
        }
        return sha256(concatBytes$1(tagP, ...messages));
    }
    const pointToBytes = (point) => point.toRawBytes(true).slice(1);
    const numTo32b = (n) => numberToBytesBE$1(n, 32);
    const modP = (x) => mod$1(x, secp256k1P);
    const modN$1 = (x) => mod$1(x, secp256k1N);
    const Point = secp256k1.ProjectivePoint;
    const GmulAdd = (Q, a, b) => Point.BASE.multiplyAndAddUnsafe(Q, a, b);
    function schnorrGetExtPubKey(priv) {
        let d_ = secp256k1.utils.normPrivateKeyToScalar(priv);
        let p = Point.fromPrivateKey(d_);
        const scalar = p.hasEvenY() ? d_ : modN$1(-d_);
        return { scalar: scalar, bytes: pointToBytes(p) };
    }
    function lift_x(x) {
        if (!fe(x))
            throw new Error('bad x: need 0 < x < p');
        const xx = modP(x * x);
        const c = modP(xx * x + BigInt(7));
        let y = sqrtMod(c);
        if (y % _2n$8 !== _0n$c)
            y = modP(-y);
        const p = new Point(x, y, _1n$b);
        p.assertValidity();
        return p;
    }
    function challenge(...args) {
        return modN$1(bytesToNumberBE$1(taggedHash('BIP0340/challenge', ...args)));
    }
    function schnorrGetPublicKey(privateKey) {
        return schnorrGetExtPubKey(privateKey).bytes;
    }
    function schnorrSign(message, privateKey, auxRand = randomBytes$1(32)) {
        const m = ensureBytes$1('message', message);
        const { bytes: px, scalar: d } = schnorrGetExtPubKey(privateKey);
        const a = ensureBytes$1('auxRand', auxRand, 32);
        const t = numTo32b(d ^ bytesToNumberBE$1(taggedHash('BIP0340/aux', a)));
        const rand = taggedHash('BIP0340/nonce', t, px, m);
        const k_ = modN$1(bytesToNumberBE$1(rand));
        if (k_ === _0n$c)
            throw new Error('sign failed: k is zero');
        const { bytes: rx, scalar: k } = schnorrGetExtPubKey(k_);
        const e = challenge(rx, px, m);
        const sig = new Uint8Array(64);
        sig.set(rx, 0);
        sig.set(numTo32b(modN$1(k + e * d)), 32);
        if (!schnorrVerify(sig, m, px))
            throw new Error('sign: Invalid signature produced');
        return sig;
    }
    function schnorrVerify(signature, message, publicKey) {
        const sig = ensureBytes$1('signature', signature, 64);
        const m = ensureBytes$1('message', message);
        const pub = ensureBytes$1('publicKey', publicKey, 32);
        try {
            const P = lift_x(bytesToNumberBE$1(pub));
            const r = bytesToNumberBE$1(sig.subarray(0, 32));
            if (!fe(r))
                return false;
            const s = bytesToNumberBE$1(sig.subarray(32, 64));
            if (!ge(s))
                return false;
            const e = challenge(numTo32b(r), pointToBytes(P), m);
            const R = GmulAdd(P, s, modN$1(-e));
            if (!R || !R.hasEvenY() || R.toAffine().x !== r)
                return false;
            return true;
        }
        catch (error) {
            return false;
        }
    }
    (() => ({
        getPublicKey: schnorrGetPublicKey,
        sign: schnorrSign,
        verify: schnorrVerify,
        utils: {
            randomPrivateKey: secp256k1.utils.randomPrivateKey,
            lift_x,
            pointToBytes,
            numberToBytesBE: numberToBytesBE$1,
            bytesToNumberBE: bytesToNumberBE$1,
            taggedHash,
            mod: mod$1,
        },
    }))();
    const isoMap =  (() => isogenyMap(Fp$2, [
        [
            '0x8e38e38e38e38e38e38e38e38e38e38e38e38e38e38e38e38e38e38daaaaa8c7',
            '0x7d3d4c80bc321d5b9f315cea7fd44c5d595d2fc0bf63b92dfff1044f17c6581',
            '0x534c328d23f234e6e2a413deca25caece4506144037c40314ecbd0b53d9dd262',
            '0x8e38e38e38e38e38e38e38e38e38e38e38e38e38e38e38e38e38e38daaaaa88c',
        ],
        [
            '0xd35771193d94918a9ca34ccbb7b640dd86cd409542f8487d9fe6b745781eb49b',
            '0xedadc6f64383dc1df7c4b2d51b54225406d36b641f5e41bbc52a56612a8c6d14',
            '0x0000000000000000000000000000000000000000000000000000000000000001',
        ],
        [
            '0x4bda12f684bda12f684bda12f684bda12f684bda12f684bda12f684b8e38e23c',
            '0xc75e0c32d5cb7c0fa9d0a54b12a0a6d5647ab046d686da6fdffc90fc201d71a3',
            '0x29a6194691f91a73715209ef6512e576722830a201be2018a765e85a9ecee931',
            '0x2f684bda12f684bda12f684bda12f684bda12f684bda12f684bda12f38e38d84',
        ],
        [
            '0xfffffffffffffffffffffffffffffffffffffffffffffffffffffffefffff93b',
            '0x7a06534bb8bdb49fd5e9e6632722c2989467c1bfc8e8d978dfb425d2685c2573',
            '0x6484aa716545ca2cf3a70c3fa8fe337e0a3d21162f0d6299a7bf8192bfd2a76f',
            '0x0000000000000000000000000000000000000000000000000000000000000001',
        ],
    ].map((i) => i.map((j) => BigInt(j)))))();
    const mapSWU =  (() => mapToCurveSimpleSWU(Fp$2, {
        A: BigInt('0x3f8731abdd661adca08a5558f0f5d272e953d363cb6f0e5d405447c01a444533'),
        B: BigInt('1771'),
        Z: Fp$2.create(BigInt('-11')),
    }))();
    (() => createHasher$2(secp256k1.ProjectivePoint, (scalars) => {
        const { x, y } = mapSWU(Fp$2.create(scalars[0]));
        return isoMap(x, y);
    }, {
        DST: 'secp256k1_XMD:SHA-256_SSWU_RO_',
        encodeDST: 'secp256k1_XMD:SHA-256_SSWU_NU_',
        p: Fp$2.ORDER,
        m: 1,
        k: 128,
        expand: 'xmd',
        hash: sha256,
    }))();

    function secp256k1PairFromSeed(seed, onlyJs) {
        if (seed.length !== 32) {
            throw new Error('Expected valid 32-byte private key as a seed');
        }
        if (!util.hasBigInt || (!onlyJs && isReady())) {
            const full = secp256k1FromSeed(seed);
            const publicKey = full.slice(32);
            if (util.u8aEmpty(publicKey)) {
                throw new Error('Invalid publicKey generated from WASM interface');
            }
            return {
                publicKey,
                secretKey: full.slice(0, 32)
            };
        }
        return {
            publicKey: secp256k1.getPublicKey(seed, true),
            secretKey: seed
        };
    }

    function createSeedDeriveFn(fromSeed, derive) {
        return (keypair, { chainCode, isHard }) => {
            if (!isHard) {
                throw new Error('A soft key was found in the path and is not supported');
            }
            return fromSeed(derive(keypair.secretKey.subarray(0, 32), chainCode));
        };
    }

    const keyHdkdEcdsa =  createSeedDeriveFn(secp256k1PairFromSeed, secp256k1DeriveHard);

    const HDKD$1 = util.compactAddLength(util.stringToU8a('Ed25519HDKD'));
    function ed25519DeriveHard(seed, chainCode) {
        if (!util.isU8a(chainCode) || chainCode.length !== 32) {
            throw new Error('Invalid chainCode passed to derive');
        }
        return blake2AsU8a(util.u8aConcat(HDKD$1, seed, chainCode));
    }

    function randomAsU8a(length = 32) {
        return browser.getRandomValues(new Uint8Array(length));
    }
    const randomAsHex =  createAsHex(randomAsU8a);

    const BN_53 = new util.BN(0b11111111111111111111111111111111111111111111111111111);
    function randomAsNumber() {
        return util.hexToBn(randomAsHex(8)).and(BN_53).toNumber();
    }

    const [SHA512_Kh$1, SHA512_Kl$1] =  (() => u64.split([
        '0x428a2f98d728ae22', '0x7137449123ef65cd', '0xb5c0fbcfec4d3b2f', '0xe9b5dba58189dbbc',
        '0x3956c25bf348b538', '0x59f111f1b605d019', '0x923f82a4af194f9b', '0xab1c5ed5da6d8118',
        '0xd807aa98a3030242', '0x12835b0145706fbe', '0x243185be4ee4b28c', '0x550c7dc3d5ffb4e2',
        '0x72be5d74f27b896f', '0x80deb1fe3b1696b1', '0x9bdc06a725c71235', '0xc19bf174cf692694',
        '0xe49b69c19ef14ad2', '0xefbe4786384f25e3', '0x0fc19dc68b8cd5b5', '0x240ca1cc77ac9c65',
        '0x2de92c6f592b0275', '0x4a7484aa6ea6e483', '0x5cb0a9dcbd41fbd4', '0x76f988da831153b5',
        '0x983e5152ee66dfab', '0xa831c66d2db43210', '0xb00327c898fb213f', '0xbf597fc7beef0ee4',
        '0xc6e00bf33da88fc2', '0xd5a79147930aa725', '0x06ca6351e003826f', '0x142929670a0e6e70',
        '0x27b70a8546d22ffc', '0x2e1b21385c26c926', '0x4d2c6dfc5ac42aed', '0x53380d139d95b3df',
        '0x650a73548baf63de', '0x766a0abb3c77b2a8', '0x81c2c92e47edaee6', '0x92722c851482353b',
        '0xa2bfe8a14cf10364', '0xa81a664bbc423001', '0xc24b8b70d0f89791', '0xc76c51a30654be30',
        '0xd192e819d6ef5218', '0xd69906245565a910', '0xf40e35855771202a', '0x106aa07032bbd1b8',
        '0x19a4c116b8d2d0c8', '0x1e376c085141ab53', '0x2748774cdf8eeb99', '0x34b0bcb5e19b48a8',
        '0x391c0cb3c5c95a63', '0x4ed8aa4ae3418acb', '0x5b9cca4f7763e373', '0x682e6ff3d6b2b8a3',
        '0x748f82ee5defb2fc', '0x78a5636f43172f60', '0x84c87814a1f0ab72', '0x8cc702081a6439ec',
        '0x90befffa23631e28', '0xa4506cebde82bde9', '0xbef9a3f7b2c67915', '0xc67178f2e372532b',
        '0xca273eceea26619c', '0xd186b8c721c0c207', '0xeada7dd6cde0eb1e', '0xf57d4f7fee6ed178',
        '0x06f067aa72176fba', '0x0a637dc5a2c898a6', '0x113f9804bef90dae', '0x1b710b35131c471b',
        '0x28db77f523047d84', '0x32caab7b40c72493', '0x3c9ebe0a15c9bebc', '0x431d67c49c100d4c',
        '0x4cc5d4becb3e42b6', '0x597f299cfc657e2a', '0x5fcb6fab3ad6faec', '0x6c44198c4a475817'
    ].map(n => BigInt(n))))();
    const SHA512_W_H$1 =  new Uint32Array(80);
    const SHA512_W_L$1 =  new Uint32Array(80);
    let SHA512$1 = class SHA512 extends SHA2 {
        constructor() {
            super(128, 64, 16, false);
            this.Ah = 0x6a09e667 | 0;
            this.Al = 0xf3bcc908 | 0;
            this.Bh = 0xbb67ae85 | 0;
            this.Bl = 0x84caa73b | 0;
            this.Ch = 0x3c6ef372 | 0;
            this.Cl = 0xfe94f82b | 0;
            this.Dh = 0xa54ff53a | 0;
            this.Dl = 0x5f1d36f1 | 0;
            this.Eh = 0x510e527f | 0;
            this.El = 0xade682d1 | 0;
            this.Fh = 0x9b05688c | 0;
            this.Fl = 0x2b3e6c1f | 0;
            this.Gh = 0x1f83d9ab | 0;
            this.Gl = 0xfb41bd6b | 0;
            this.Hh = 0x5be0cd19 | 0;
            this.Hl = 0x137e2179 | 0;
        }
        get() {
            const { Ah, Al, Bh, Bl, Ch, Cl, Dh, Dl, Eh, El, Fh, Fl, Gh, Gl, Hh, Hl } = this;
            return [Ah, Al, Bh, Bl, Ch, Cl, Dh, Dl, Eh, El, Fh, Fl, Gh, Gl, Hh, Hl];
        }
        set(Ah, Al, Bh, Bl, Ch, Cl, Dh, Dl, Eh, El, Fh, Fl, Gh, Gl, Hh, Hl) {
            this.Ah = Ah | 0;
            this.Al = Al | 0;
            this.Bh = Bh | 0;
            this.Bl = Bl | 0;
            this.Ch = Ch | 0;
            this.Cl = Cl | 0;
            this.Dh = Dh | 0;
            this.Dl = Dl | 0;
            this.Eh = Eh | 0;
            this.El = El | 0;
            this.Fh = Fh | 0;
            this.Fl = Fl | 0;
            this.Gh = Gh | 0;
            this.Gl = Gl | 0;
            this.Hh = Hh | 0;
            this.Hl = Hl | 0;
        }
        process(view, offset) {
            for (let i = 0; i < 16; i++, offset += 4) {
                SHA512_W_H$1[i] = view.getUint32(offset);
                SHA512_W_L$1[i] = view.getUint32((offset += 4));
            }
            for (let i = 16; i < 80; i++) {
                const W15h = SHA512_W_H$1[i - 15] | 0;
                const W15l = SHA512_W_L$1[i - 15] | 0;
                const s0h = u64.rotrSH(W15h, W15l, 1) ^ u64.rotrSH(W15h, W15l, 8) ^ u64.shrSH(W15h, W15l, 7);
                const s0l = u64.rotrSL(W15h, W15l, 1) ^ u64.rotrSL(W15h, W15l, 8) ^ u64.shrSL(W15h, W15l, 7);
                const W2h = SHA512_W_H$1[i - 2] | 0;
                const W2l = SHA512_W_L$1[i - 2] | 0;
                const s1h = u64.rotrSH(W2h, W2l, 19) ^ u64.rotrBH(W2h, W2l, 61) ^ u64.shrSH(W2h, W2l, 6);
                const s1l = u64.rotrSL(W2h, W2l, 19) ^ u64.rotrBL(W2h, W2l, 61) ^ u64.shrSL(W2h, W2l, 6);
                const SUMl = u64.add4L(s0l, s1l, SHA512_W_L$1[i - 7], SHA512_W_L$1[i - 16]);
                const SUMh = u64.add4H(SUMl, s0h, s1h, SHA512_W_H$1[i - 7], SHA512_W_H$1[i - 16]);
                SHA512_W_H$1[i] = SUMh | 0;
                SHA512_W_L$1[i] = SUMl | 0;
            }
            let { Ah, Al, Bh, Bl, Ch, Cl, Dh, Dl, Eh, El, Fh, Fl, Gh, Gl, Hh, Hl } = this;
            for (let i = 0; i < 80; i++) {
                const sigma1h = u64.rotrSH(Eh, El, 14) ^ u64.rotrSH(Eh, El, 18) ^ u64.rotrBH(Eh, El, 41);
                const sigma1l = u64.rotrSL(Eh, El, 14) ^ u64.rotrSL(Eh, El, 18) ^ u64.rotrBL(Eh, El, 41);
                const CHIh = (Eh & Fh) ^ (~Eh & Gh);
                const CHIl = (El & Fl) ^ (~El & Gl);
                const T1ll = u64.add5L(Hl, sigma1l, CHIl, SHA512_Kl$1[i], SHA512_W_L$1[i]);
                const T1h = u64.add5H(T1ll, Hh, sigma1h, CHIh, SHA512_Kh$1[i], SHA512_W_H$1[i]);
                const T1l = T1ll | 0;
                const sigma0h = u64.rotrSH(Ah, Al, 28) ^ u64.rotrBH(Ah, Al, 34) ^ u64.rotrBH(Ah, Al, 39);
                const sigma0l = u64.rotrSL(Ah, Al, 28) ^ u64.rotrBL(Ah, Al, 34) ^ u64.rotrBL(Ah, Al, 39);
                const MAJh = (Ah & Bh) ^ (Ah & Ch) ^ (Bh & Ch);
                const MAJl = (Al & Bl) ^ (Al & Cl) ^ (Bl & Cl);
                Hh = Gh | 0;
                Hl = Gl | 0;
                Gh = Fh | 0;
                Gl = Fl | 0;
                Fh = Eh | 0;
                Fl = El | 0;
                ({ h: Eh, l: El } = u64.add(Dh | 0, Dl | 0, T1h | 0, T1l | 0));
                Dh = Ch | 0;
                Dl = Cl | 0;
                Ch = Bh | 0;
                Cl = Bl | 0;
                Bh = Ah | 0;
                Bl = Al | 0;
                const All = u64.add3L(T1l, sigma0l, MAJl);
                Ah = u64.add3H(All, T1h, sigma0h, MAJh);
                Al = All | 0;
            }
            ({ h: Ah, l: Al } = u64.add(this.Ah | 0, this.Al | 0, Ah | 0, Al | 0));
            ({ h: Bh, l: Bl } = u64.add(this.Bh | 0, this.Bl | 0, Bh | 0, Bl | 0));
            ({ h: Ch, l: Cl } = u64.add(this.Ch | 0, this.Cl | 0, Ch | 0, Cl | 0));
            ({ h: Dh, l: Dl } = u64.add(this.Dh | 0, this.Dl | 0, Dh | 0, Dl | 0));
            ({ h: Eh, l: El } = u64.add(this.Eh | 0, this.El | 0, Eh | 0, El | 0));
            ({ h: Fh, l: Fl } = u64.add(this.Fh | 0, this.Fl | 0, Fh | 0, Fl | 0));
            ({ h: Gh, l: Gl } = u64.add(this.Gh | 0, this.Gl | 0, Gh | 0, Gl | 0));
            ({ h: Hh, l: Hl } = u64.add(this.Hh | 0, this.Hl | 0, Hh | 0, Hl | 0));
            this.set(Ah, Al, Bh, Bl, Ch, Cl, Dh, Dl, Eh, El, Fh, Fl, Gh, Gl, Hh, Hl);
        }
        roundClean() {
            SHA512_W_H$1.fill(0);
            SHA512_W_L$1.fill(0);
        }
        destroy() {
            this.buffer.fill(0);
            this.set(0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0);
        }
    };
    let SHA512_224$1 = class SHA512_224 extends SHA512$1 {
        constructor() {
            super();
            this.Ah = 0x8c3d37c8 | 0;
            this.Al = 0x19544da2 | 0;
            this.Bh = 0x73e19966 | 0;
            this.Bl = 0x89dcd4d6 | 0;
            this.Ch = 0x1dfab7ae | 0;
            this.Cl = 0x32ff9c82 | 0;
            this.Dh = 0x679dd514 | 0;
            this.Dl = 0x582f9fcf | 0;
            this.Eh = 0x0f6d2b69 | 0;
            this.El = 0x7bd44da8 | 0;
            this.Fh = 0x77e36f73 | 0;
            this.Fl = 0x04c48942 | 0;
            this.Gh = 0x3f9d85a8 | 0;
            this.Gl = 0x6a1d36c8 | 0;
            this.Hh = 0x1112e6ad | 0;
            this.Hl = 0x91d692a1 | 0;
            this.outputLen = 28;
        }
    };
    let SHA512_256$1 = class SHA512_256 extends SHA512$1 {
        constructor() {
            super();
            this.Ah = 0x22312194 | 0;
            this.Al = 0xfc2bf72c | 0;
            this.Bh = 0x9f555fa3 | 0;
            this.Bl = 0xc84c64c2 | 0;
            this.Ch = 0x2393b86b | 0;
            this.Cl = 0x6f53b151 | 0;
            this.Dh = 0x96387719 | 0;
            this.Dl = 0x5940eabd | 0;
            this.Eh = 0x96283ee2 | 0;
            this.El = 0xa88effe3 | 0;
            this.Fh = 0xbe5e1e25 | 0;
            this.Fl = 0x53863992 | 0;
            this.Gh = 0x2b0199fc | 0;
            this.Gl = 0x2c85b8aa | 0;
            this.Hh = 0x0eb72ddc | 0;
            this.Hl = 0x81c52ca2 | 0;
            this.outputLen = 32;
        }
    };
    let SHA384$1 = class SHA384 extends SHA512$1 {
        constructor() {
            super();
            this.Ah = 0xcbbb9d5d | 0;
            this.Al = 0xc1059ed8 | 0;
            this.Bh = 0x629a292a | 0;
            this.Bl = 0x367cd507 | 0;
            this.Ch = 0x9159015a | 0;
            this.Cl = 0x3070dd17 | 0;
            this.Dh = 0x152fecd8 | 0;
            this.Dl = 0xf70e5939 | 0;
            this.Eh = 0x67332667 | 0;
            this.El = 0xffc00b31 | 0;
            this.Fh = 0x8eb44a87 | 0;
            this.Fl = 0x68581511 | 0;
            this.Gh = 0xdb0c2e0d | 0;
            this.Gl = 0x64f98fa7 | 0;
            this.Hh = 0x47b5481d | 0;
            this.Hl = 0xbefa4fa4 | 0;
            this.outputLen = 48;
        }
    };
    const sha512$2 =  wrapConstructor(() => new SHA512$1());
    wrapConstructor(() => new SHA512_224$1());
    wrapConstructor(() => new SHA512_256$1());
    wrapConstructor(() => new SHA384$1());

    /*! noble-curves - MIT License (c) 2022 Paul Miller (paulmillr.com) */
    const _0n$b = BigInt(0), _1n$a = BigInt(1), _2n$7 = BigInt(2), _8n$3 = BigInt(8);
    const VERIFY_DEFAULT = { zip215: true };
    function validateOpts$2(curve) {
        const opts = validateBasic(curve);
        validateObject(curve, {
            hash: 'function',
            a: 'bigint',
            d: 'bigint',
            randomBytes: 'function',
        }, {
            adjustScalarBytes: 'function',
            domain: 'function',
            uvRatio: 'function',
            mapToCurve: 'function',
        });
        return Object.freeze({ ...opts });
    }
    function twistedEdwards$1(curveDef) {
        const CURVE = validateOpts$2(curveDef);
        const { Fp, n: CURVE_ORDER, prehash: prehash, hash: cHash, randomBytes, nByteLength, h: cofactor, } = CURVE;
        const MASK = _2n$7 << (BigInt(nByteLength * 8) - _1n$a);
        const modP = Fp.create;
        const uvRatio = CURVE.uvRatio ||
            ((u, v) => {
                try {
                    return { isValid: true, value: Fp.sqrt(u * Fp.inv(v)) };
                }
                catch (e) {
                    return { isValid: false, value: _0n$b };
                }
            });
        const adjustScalarBytes = CURVE.adjustScalarBytes || ((bytes) => bytes);
        const domain = CURVE.domain ||
            ((data, ctx, phflag) => {
                if (ctx.length || phflag)
                    throw new Error('Contexts/pre-hash are not supported');
                return data;
            });
        const inBig = (n) => typeof n === 'bigint' && _0n$b < n;
        const inRange = (n, max) => inBig(n) && inBig(max) && n < max;
        const in0MaskRange = (n) => n === _0n$b || inRange(n, MASK);
        function assertInRange(n, max) {
            if (inRange(n, max))
                return n;
            throw new Error(`Expected valid scalar < ${max}, got ${typeof n} ${n}`);
        }
        function assertGE0(n) {
            return n === _0n$b ? n : assertInRange(n, CURVE_ORDER);
        }
        const pointPrecomputes = new Map();
        function isPoint(other) {
            if (!(other instanceof Point))
                throw new Error('ExtendedPoint expected');
        }
        class Point {
            constructor(ex, ey, ez, et) {
                this.ex = ex;
                this.ey = ey;
                this.ez = ez;
                this.et = et;
                if (!in0MaskRange(ex))
                    throw new Error('x required');
                if (!in0MaskRange(ey))
                    throw new Error('y required');
                if (!in0MaskRange(ez))
                    throw new Error('z required');
                if (!in0MaskRange(et))
                    throw new Error('t required');
            }
            get x() {
                return this.toAffine().x;
            }
            get y() {
                return this.toAffine().y;
            }
            static fromAffine(p) {
                if (p instanceof Point)
                    throw new Error('extended point not allowed');
                const { x, y } = p || {};
                if (!in0MaskRange(x) || !in0MaskRange(y))
                    throw new Error('invalid affine point');
                return new Point(x, y, _1n$a, modP(x * y));
            }
            static normalizeZ(points) {
                const toInv = Fp.invertBatch(points.map((p) => p.ez));
                return points.map((p, i) => p.toAffine(toInv[i])).map(Point.fromAffine);
            }
            _setWindowSize(windowSize) {
                this._WINDOW_SIZE = windowSize;
                pointPrecomputes.delete(this);
            }
            assertValidity() {
                const { a, d } = CURVE;
                if (this.is0())
                    throw new Error('bad point: ZERO');
                const { ex: X, ey: Y, ez: Z, et: T } = this;
                const X2 = modP(X * X);
                const Y2 = modP(Y * Y);
                const Z2 = modP(Z * Z);
                const Z4 = modP(Z2 * Z2);
                const aX2 = modP(X2 * a);
                const left = modP(Z2 * modP(aX2 + Y2));
                const right = modP(Z4 + modP(d * modP(X2 * Y2)));
                if (left !== right)
                    throw new Error('bad point: equation left != right (1)');
                const XY = modP(X * Y);
                const ZT = modP(Z * T);
                if (XY !== ZT)
                    throw new Error('bad point: equation left != right (2)');
            }
            equals(other) {
                isPoint(other);
                const { ex: X1, ey: Y1, ez: Z1 } = this;
                const { ex: X2, ey: Y2, ez: Z2 } = other;
                const X1Z2 = modP(X1 * Z2);
                const X2Z1 = modP(X2 * Z1);
                const Y1Z2 = modP(Y1 * Z2);
                const Y2Z1 = modP(Y2 * Z1);
                return X1Z2 === X2Z1 && Y1Z2 === Y2Z1;
            }
            is0() {
                return this.equals(Point.ZERO);
            }
            negate() {
                return new Point(modP(-this.ex), this.ey, this.ez, modP(-this.et));
            }
            double() {
                const { a } = CURVE;
                const { ex: X1, ey: Y1, ez: Z1 } = this;
                const A = modP(X1 * X1);
                const B = modP(Y1 * Y1);
                const C = modP(_2n$7 * modP(Z1 * Z1));
                const D = modP(a * A);
                const x1y1 = X1 + Y1;
                const E = modP(modP(x1y1 * x1y1) - A - B);
                const G = D + B;
                const F = G - C;
                const H = D - B;
                const X3 = modP(E * F);
                const Y3 = modP(G * H);
                const T3 = modP(E * H);
                const Z3 = modP(F * G);
                return new Point(X3, Y3, Z3, T3);
            }
            add(other) {
                isPoint(other);
                const { a, d } = CURVE;
                const { ex: X1, ey: Y1, ez: Z1, et: T1 } = this;
                const { ex: X2, ey: Y2, ez: Z2, et: T2 } = other;
                if (a === BigInt(-1)) {
                    const A = modP((Y1 - X1) * (Y2 + X2));
                    const B = modP((Y1 + X1) * (Y2 - X2));
                    const F = modP(B - A);
                    if (F === _0n$b)
                        return this.double();
                    const C = modP(Z1 * _2n$7 * T2);
                    const D = modP(T1 * _2n$7 * Z2);
                    const E = D + C;
                    const G = B + A;
                    const H = D - C;
                    const X3 = modP(E * F);
                    const Y3 = modP(G * H);
                    const T3 = modP(E * H);
                    const Z3 = modP(F * G);
                    return new Point(X3, Y3, Z3, T3);
                }
                const A = modP(X1 * X2);
                const B = modP(Y1 * Y2);
                const C = modP(T1 * d * T2);
                const D = modP(Z1 * Z2);
                const E = modP((X1 + Y1) * (X2 + Y2) - A - B);
                const F = D - C;
                const G = D + C;
                const H = modP(B - a * A);
                const X3 = modP(E * F);
                const Y3 = modP(G * H);
                const T3 = modP(E * H);
                const Z3 = modP(F * G);
                return new Point(X3, Y3, Z3, T3);
            }
            subtract(other) {
                return this.add(other.negate());
            }
            wNAF(n) {
                return wnaf.wNAFCached(this, pointPrecomputes, n, Point.normalizeZ);
            }
            multiply(scalar) {
                const { p, f } = this.wNAF(assertInRange(scalar, CURVE_ORDER));
                return Point.normalizeZ([p, f])[0];
            }
            multiplyUnsafe(scalar) {
                let n = assertGE0(scalar);
                if (n === _0n$b)
                    return I;
                if (this.equals(I) || n === _1n$a)
                    return this;
                if (this.equals(G))
                    return this.wNAF(n).p;
                return wnaf.unsafeLadder(this, n);
            }
            isSmallOrder() {
                return this.multiplyUnsafe(cofactor).is0();
            }
            isTorsionFree() {
                return wnaf.unsafeLadder(this, CURVE_ORDER).is0();
            }
            toAffine(iz) {
                const { ex: x, ey: y, ez: z } = this;
                const is0 = this.is0();
                if (iz == null)
                    iz = is0 ? _8n$3 : Fp.inv(z);
                const ax = modP(x * iz);
                const ay = modP(y * iz);
                const zz = modP(z * iz);
                if (is0)
                    return { x: _0n$b, y: _1n$a };
                if (zz !== _1n$a)
                    throw new Error('invZ was invalid');
                return { x: ax, y: ay };
            }
            clearCofactor() {
                const { h: cofactor } = CURVE;
                if (cofactor === _1n$a)
                    return this;
                return this.multiplyUnsafe(cofactor);
            }
            static fromHex(hex, zip215 = false) {
                const { d, a } = CURVE;
                const len = Fp.BYTES;
                hex = ensureBytes$1('pointHex', hex, len);
                const normed = hex.slice();
                const lastByte = hex[len - 1];
                normed[len - 1] = lastByte & ~0x80;
                const y = bytesToNumberLE$2(normed);
                if (y === _0n$b) ;
                else {
                    if (zip215)
                        assertInRange(y, MASK);
                    else
                        assertInRange(y, Fp.ORDER);
                }
                const y2 = modP(y * y);
                const u = modP(y2 - _1n$a);
                const v = modP(d * y2 - a);
                let { isValid, value: x } = uvRatio(u, v);
                if (!isValid)
                    throw new Error('Point.fromHex: invalid y coordinate');
                const isXOdd = (x & _1n$a) === _1n$a;
                const isLastByteOdd = (lastByte & 0x80) !== 0;
                if (!zip215 && x === _0n$b && isLastByteOdd)
                    throw new Error('Point.fromHex: x=0 and x_0=1');
                if (isLastByteOdd !== isXOdd)
                    x = modP(-x);
                return Point.fromAffine({ x, y });
            }
            static fromPrivateKey(privKey) {
                return getExtendedPublicKey(privKey).point;
            }
            toRawBytes() {
                const { x, y } = this.toAffine();
                const bytes = numberToBytesLE$2(y, Fp.BYTES);
                bytes[bytes.length - 1] |= x & _1n$a ? 0x80 : 0;
                return bytes;
            }
            toHex() {
                return bytesToHex$1(this.toRawBytes());
            }
        }
        Point.BASE = new Point(CURVE.Gx, CURVE.Gy, _1n$a, modP(CURVE.Gx * CURVE.Gy));
        Point.ZERO = new Point(_0n$b, _1n$a, _1n$a, _0n$b);
        const { BASE: G, ZERO: I } = Point;
        const wnaf = wNAF$1(Point, nByteLength * 8);
        function modN(a) {
            return mod$1(a, CURVE_ORDER);
        }
        function modN_LE(hash) {
            return modN(bytesToNumberLE$2(hash));
        }
        function getExtendedPublicKey(key) {
            const len = nByteLength;
            key = ensureBytes$1('private key', key, len);
            const hashed = ensureBytes$1('hashed private key', cHash(key), 2 * len);
            const head = adjustScalarBytes(hashed.slice(0, len));
            const prefix = hashed.slice(len, 2 * len);
            const scalar = modN_LE(head);
            const point = G.multiply(scalar);
            const pointBytes = point.toRawBytes();
            return { head, prefix, scalar, point, pointBytes };
        }
        function getPublicKey(privKey) {
            return getExtendedPublicKey(privKey).pointBytes;
        }
        function hashDomainToScalar(context = new Uint8Array(), ...msgs) {
            const msg = concatBytes$1(...msgs);
            return modN_LE(cHash(domain(msg, ensureBytes$1('context', context), !!prehash)));
        }
        function sign(msg, privKey, options = {}) {
            msg = ensureBytes$1('message', msg);
            if (prehash)
                msg = prehash(msg);
            const { prefix, scalar, pointBytes } = getExtendedPublicKey(privKey);
            const r = hashDomainToScalar(options.context, prefix, msg);
            const R = G.multiply(r).toRawBytes();
            const k = hashDomainToScalar(options.context, R, pointBytes, msg);
            const s = modN(r + k * scalar);
            assertGE0(s);
            const res = concatBytes$1(R, numberToBytesLE$2(s, Fp.BYTES));
            return ensureBytes$1('result', res, nByteLength * 2);
        }
        const verifyOpts = VERIFY_DEFAULT;
        function verify(sig, msg, publicKey, options = verifyOpts) {
            const { context, zip215 } = options;
            const len = Fp.BYTES;
            sig = ensureBytes$1('signature', sig, 2 * len);
            msg = ensureBytes$1('message', msg);
            if (prehash)
                msg = prehash(msg);
            const s = bytesToNumberLE$2(sig.slice(len, 2 * len));
            let A, R, SB;
            try {
                A = Point.fromHex(publicKey, zip215);
                R = Point.fromHex(sig.slice(0, len), zip215);
                SB = G.multiplyUnsafe(s);
            }
            catch (error) {
                return false;
            }
            if (!zip215 && A.isSmallOrder())
                return false;
            const k = hashDomainToScalar(context, R.toRawBytes(), A.toRawBytes(), msg);
            const RkA = R.add(A.multiplyUnsafe(k));
            return RkA.subtract(SB).clearCofactor().equals(Point.ZERO);
        }
        G._setWindowSize(8);
        const utils = {
            getExtendedPublicKey,
            randomPrivateKey: () => randomBytes(Fp.BYTES),
            precompute(windowSize = 8, point = Point.BASE) {
                point._setWindowSize(windowSize);
                point.multiply(BigInt(3));
                return point;
            },
        };
        return {
            CURVE,
            getPublicKey,
            sign,
            verify,
            ExtendedPoint: Point,
            utils,
        };
    }

    /*! noble-curves - MIT License (c) 2022 Paul Miller (paulmillr.com) */
    const _0n$a = BigInt(0);
    const _1n$9 = BigInt(1);
    function validateOpts$1(curve) {
        validateObject(curve, {
            a: 'bigint',
        }, {
            montgomeryBits: 'isSafeInteger',
            nByteLength: 'isSafeInteger',
            adjustScalarBytes: 'function',
            domain: 'function',
            powPminus2: 'function',
            Gu: 'bigint',
        });
        return Object.freeze({ ...curve });
    }
    function montgomery$1(curveDef) {
        const CURVE = validateOpts$1(curveDef);
        const { P } = CURVE;
        const modP = (n) => mod$1(n, P);
        const montgomeryBits = CURVE.montgomeryBits;
        const montgomeryBytes = Math.ceil(montgomeryBits / 8);
        const fieldLen = CURVE.nByteLength;
        const adjustScalarBytes = CURVE.adjustScalarBytes || ((bytes) => bytes);
        const powPminus2 = CURVE.powPminus2 || ((x) => pow(x, P - BigInt(2), P));
        function cswap(swap, x_2, x_3) {
            const dummy = modP(swap * (x_2 - x_3));
            x_2 = modP(x_2 - dummy);
            x_3 = modP(x_3 + dummy);
            return [x_2, x_3];
        }
        function assertFieldElement(n) {
            if (typeof n === 'bigint' && _0n$a <= n && n < P)
                return n;
            throw new Error('Expected valid scalar 0 < scalar < CURVE.P');
        }
        const a24 = (CURVE.a - BigInt(2)) / BigInt(4);
        function montgomeryLadder(pointU, scalar) {
            const u = assertFieldElement(pointU);
            const k = assertFieldElement(scalar);
            const x_1 = u;
            let x_2 = _1n$9;
            let z_2 = _0n$a;
            let x_3 = u;
            let z_3 = _1n$9;
            let swap = _0n$a;
            let sw;
            for (let t = BigInt(montgomeryBits - 1); t >= _0n$a; t--) {
                const k_t = (k >> t) & _1n$9;
                swap ^= k_t;
                sw = cswap(swap, x_2, x_3);
                x_2 = sw[0];
                x_3 = sw[1];
                sw = cswap(swap, z_2, z_3);
                z_2 = sw[0];
                z_3 = sw[1];
                swap = k_t;
                const A = x_2 + z_2;
                const AA = modP(A * A);
                const B = x_2 - z_2;
                const BB = modP(B * B);
                const E = AA - BB;
                const C = x_3 + z_3;
                const D = x_3 - z_3;
                const DA = modP(D * A);
                const CB = modP(C * B);
                const dacb = DA + CB;
                const da_cb = DA - CB;
                x_3 = modP(dacb * dacb);
                z_3 = modP(x_1 * modP(da_cb * da_cb));
                x_2 = modP(AA * BB);
                z_2 = modP(E * (AA + modP(a24 * E)));
            }
            sw = cswap(swap, x_2, x_3);
            x_2 = sw[0];
            x_3 = sw[1];
            sw = cswap(swap, z_2, z_3);
            z_2 = sw[0];
            z_3 = sw[1];
            const z2 = powPminus2(z_2);
            return modP(x_2 * z2);
        }
        function encodeUCoordinate(u) {
            return numberToBytesLE$2(modP(u), montgomeryBytes);
        }
        function decodeUCoordinate(uEnc) {
            const u = ensureBytes$1('u coordinate', uEnc, montgomeryBytes);
            if (fieldLen === 32)
                u[31] &= 127;
            return bytesToNumberLE$2(u);
        }
        function decodeScalar(n) {
            const bytes = ensureBytes$1('scalar', n);
            const len = bytes.length;
            if (len !== montgomeryBytes && len !== fieldLen)
                throw new Error(`Expected ${montgomeryBytes} or ${fieldLen} bytes, got ${len}`);
            return bytesToNumberLE$2(adjustScalarBytes(bytes));
        }
        function scalarMult(scalar, u) {
            const pointU = decodeUCoordinate(u);
            const _scalar = decodeScalar(scalar);
            const pu = montgomeryLadder(pointU, _scalar);
            if (pu === _0n$a)
                throw new Error('Invalid private or public key received');
            return encodeUCoordinate(pu);
        }
        const GuBytes = encodeUCoordinate(CURVE.Gu);
        function scalarMultBase(scalar) {
            return scalarMult(scalar, GuBytes);
        }
        return {
            scalarMult,
            scalarMultBase,
            getSharedSecret: (privateKey, publicKey) => scalarMult(privateKey, publicKey),
            getPublicKey: (privateKey) => scalarMultBase(privateKey),
            utils: { randomPrivateKey: () => CURVE.randomBytes(CURVE.nByteLength) },
            GuBytes: GuBytes,
        };
    }

    /*! noble-curves - MIT License (c) 2022 Paul Miller (paulmillr.com) */
    const ED25519_P = BigInt('57896044618658097711785492504343953926634992332820282019728792003956564819949');
    const ED25519_SQRT_M1$1 = BigInt('19681161376707505956807079304988542015446066515923890162744021073123829784752');
    const _0n$9 = BigInt(0), _1n$8 = BigInt(1), _2n$6 = BigInt(2), _5n$2 = BigInt(5);
    const _10n = BigInt(10), _20n = BigInt(20), _40n = BigInt(40), _80n = BigInt(80);
    function ed25519_pow_2_252_3$1(x) {
        const P = ED25519_P;
        const x2 = (x * x) % P;
        const b2 = (x2 * x) % P;
        const b4 = (pow2$1(b2, _2n$6, P) * b2) % P;
        const b5 = (pow2$1(b4, _1n$8, P) * x) % P;
        const b10 = (pow2$1(b5, _5n$2, P) * b5) % P;
        const b20 = (pow2$1(b10, _10n, P) * b10) % P;
        const b40 = (pow2$1(b20, _20n, P) * b20) % P;
        const b80 = (pow2$1(b40, _40n, P) * b40) % P;
        const b160 = (pow2$1(b80, _80n, P) * b80) % P;
        const b240 = (pow2$1(b160, _80n, P) * b80) % P;
        const b250 = (pow2$1(b240, _10n, P) * b10) % P;
        const pow_p_5_8 = (pow2$1(b250, _2n$6, P) * x) % P;
        return { pow_p_5_8, b2 };
    }
    function adjustScalarBytes$1(bytes) {
        bytes[0] &= 248;
        bytes[31] &= 127;
        bytes[31] |= 64;
        return bytes;
    }
    function uvRatio$1(u, v) {
        const P = ED25519_P;
        const v3 = mod$1(v * v * v, P);
        const v7 = mod$1(v3 * v3 * v, P);
        const pow = ed25519_pow_2_252_3$1(u * v7).pow_p_5_8;
        let x = mod$1(u * v3 * pow, P);
        const vx2 = mod$1(v * x * x, P);
        const root1 = x;
        const root2 = mod$1(x * ED25519_SQRT_M1$1, P);
        const useRoot1 = vx2 === u;
        const useRoot2 = vx2 === mod$1(-u, P);
        const noRoot = vx2 === mod$1(-u * ED25519_SQRT_M1$1, P);
        if (useRoot1)
            x = root1;
        if (useRoot2 || noRoot)
            x = root2;
        if (isNegativeLE$1(x, P))
            x = mod$1(-x, P);
        return { isValid: useRoot1 || useRoot2, value: x };
    }
    const Fp$1 = Field$1(ED25519_P, undefined, true);
    const ed25519Defaults$1 = {
        a: BigInt(-1),
        d: BigInt('37095705934669439343138083508754565189542113879843219016388785533085940283555'),
        Fp: Fp$1,
        n: BigInt('7237005577332262213973186563042994240857116359379907606001950938285454250989'),
        h: BigInt(8),
        Gx: BigInt('15112221349535400772501151409588531511454012693041857206046113283949847762202'),
        Gy: BigInt('46316835694926478169428394003475163141307993866256225615783033603165251855960'),
        hash: sha512$2,
        randomBytes: randomBytes$1,
        adjustScalarBytes: adjustScalarBytes$1,
        uvRatio: uvRatio$1,
    };
    const ed25519$1 =  twistedEdwards$1(ed25519Defaults$1);
    function ed25519_domain$1(data, ctx, phflag) {
        if (ctx.length > 255)
            throw new Error('Context is too big');
        return concatBytes$2(utf8ToBytes$2('SigEd25519 no Ed25519 collisions'), new Uint8Array([phflag ? 1 : 0, ctx.length]), ctx, data);
    }
    twistedEdwards$1({
        ...ed25519Defaults$1,
        domain: ed25519_domain$1,
    });
    twistedEdwards$1({
        ...ed25519Defaults$1,
        domain: ed25519_domain$1,
        prehash: sha512$2,
    });
    (() => montgomery$1({
        P: ED25519_P,
        a: BigInt(486662),
        montgomeryBits: 255,
        nByteLength: 32,
        Gu: BigInt(9),
        powPminus2: (x) => {
            const P = ED25519_P;
            const { pow_p_5_8, b2 } = ed25519_pow_2_252_3$1(x);
            return mod$1(pow2$1(pow_p_5_8, BigInt(3), P) * b2, P);
        },
        adjustScalarBytes: adjustScalarBytes$1,
        randomBytes: randomBytes$1,
    }))();
    const ELL2_C1$1 = (Fp$1.ORDER + BigInt(3)) / BigInt(8);
    const ELL2_C2$1 = Fp$1.pow(_2n$6, ELL2_C1$1);
    const ELL2_C3$1 = Fp$1.sqrt(Fp$1.neg(Fp$1.ONE));
    const ELL2_C4 = (Fp$1.ORDER - BigInt(5)) / BigInt(8);
    const ELL2_J = BigInt(486662);
    function map_to_curve_elligator2_curve25519$1(u) {
        let tv1 = Fp$1.sqr(u);
        tv1 = Fp$1.mul(tv1, _2n$6);
        let xd = Fp$1.add(tv1, Fp$1.ONE);
        let x1n = Fp$1.neg(ELL2_J);
        let tv2 = Fp$1.sqr(xd);
        let gxd = Fp$1.mul(tv2, xd);
        let gx1 = Fp$1.mul(tv1, ELL2_J);
        gx1 = Fp$1.mul(gx1, x1n);
        gx1 = Fp$1.add(gx1, tv2);
        gx1 = Fp$1.mul(gx1, x1n);
        let tv3 = Fp$1.sqr(gxd);
        tv2 = Fp$1.sqr(tv3);
        tv3 = Fp$1.mul(tv3, gxd);
        tv3 = Fp$1.mul(tv3, gx1);
        tv2 = Fp$1.mul(tv2, tv3);
        let y11 = Fp$1.pow(tv2, ELL2_C4);
        y11 = Fp$1.mul(y11, tv3);
        let y12 = Fp$1.mul(y11, ELL2_C3$1);
        tv2 = Fp$1.sqr(y11);
        tv2 = Fp$1.mul(tv2, gxd);
        let e1 = Fp$1.eql(tv2, gx1);
        let y1 = Fp$1.cmov(y12, y11, e1);
        let x2n = Fp$1.mul(x1n, tv1);
        let y21 = Fp$1.mul(y11, u);
        y21 = Fp$1.mul(y21, ELL2_C2$1);
        let y22 = Fp$1.mul(y21, ELL2_C3$1);
        let gx2 = Fp$1.mul(gx1, tv1);
        tv2 = Fp$1.sqr(y21);
        tv2 = Fp$1.mul(tv2, gxd);
        let e2 = Fp$1.eql(tv2, gx2);
        let y2 = Fp$1.cmov(y22, y21, e2);
        tv2 = Fp$1.sqr(y1);
        tv2 = Fp$1.mul(tv2, gxd);
        let e3 = Fp$1.eql(tv2, gx1);
        let xn = Fp$1.cmov(x2n, x1n, e3);
        let y = Fp$1.cmov(y2, y1, e3);
        let e4 = Fp$1.isOdd(y);
        y = Fp$1.cmov(y, Fp$1.neg(y), e3 !== e4);
        return { xMn: xn, xMd: xd, yMn: y, yMd: _1n$8 };
    }
    const ELL2_C1_EDWARDS$1 = FpSqrtEven$1(Fp$1, Fp$1.neg(BigInt(486664)));
    function map_to_curve_elligator2_edwards25519$1(u) {
        const { xMn, xMd, yMn, yMd } = map_to_curve_elligator2_curve25519$1(u);
        let xn = Fp$1.mul(xMn, yMd);
        xn = Fp$1.mul(xn, ELL2_C1_EDWARDS$1);
        let xd = Fp$1.mul(xMd, yMn);
        let yn = Fp$1.sub(xMn, xMd);
        let yd = Fp$1.add(xMn, xMd);
        let tv1 = Fp$1.mul(xd, yd);
        let e = Fp$1.eql(tv1, Fp$1.ZERO);
        xn = Fp$1.cmov(xn, Fp$1.ZERO, e);
        xd = Fp$1.cmov(xd, Fp$1.ONE, e);
        yn = Fp$1.cmov(yn, Fp$1.ONE, e);
        yd = Fp$1.cmov(yd, Fp$1.ONE, e);
        const inv = Fp$1.invertBatch([xd, yd]);
        return { x: Fp$1.mul(xn, inv[0]), y: Fp$1.mul(yn, inv[1]) };
    }
    (() => createHasher$2(ed25519$1.ExtendedPoint, (scalars) => map_to_curve_elligator2_edwards25519$1(scalars[0]), {
        DST: 'edwards25519_XMD:SHA-512_ELL2_RO_',
        encodeDST: 'edwards25519_XMD:SHA-512_ELL2_NU_',
        p: Fp$1.ORDER,
        m: 1,
        k: 128,
        expand: 'xmd',
        hash: sha512$2,
    }))();
    function assertRstPoint(other) {
        if (!(other instanceof RistPoint))
            throw new Error('RistrettoPoint expected');
    }
    const SQRT_M1$1 = ED25519_SQRT_M1$1;
    const SQRT_AD_MINUS_ONE$1 = BigInt('25063068953384623474111414158702152701244531502492656460079210482610430750235');
    const INVSQRT_A_MINUS_D$1 = BigInt('54469307008909316920995813868745141605393597292927456921205312896311721017578');
    const ONE_MINUS_D_SQ$1 = BigInt('1159843021668779879193775521855586647937357759715417654439879720876111806838');
    const D_MINUS_ONE_SQ$1 = BigInt('40440834346308536858101042469323190826248399146238708352240133220865137265952');
    const invertSqrt$1 = (number) => uvRatio$1(_1n$8, number);
    const MAX_255B$1 = BigInt('0x7fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff');
    const bytes255ToNumberLE$1 = (bytes) => ed25519$1.CURVE.Fp.create(bytesToNumberLE$2(bytes) & MAX_255B$1);
    function calcElligatorRistrettoMap$1(r0) {
        const { d } = ed25519$1.CURVE;
        const P = ed25519$1.CURVE.Fp.ORDER;
        const mod = ed25519$1.CURVE.Fp.create;
        const r = mod(SQRT_M1$1 * r0 * r0);
        const Ns = mod((r + _1n$8) * ONE_MINUS_D_SQ$1);
        let c = BigInt(-1);
        const D = mod((c - d * r) * mod(r + d));
        let { isValid: Ns_D_is_sq, value: s } = uvRatio$1(Ns, D);
        let s_ = mod(s * r0);
        if (!isNegativeLE$1(s_, P))
            s_ = mod(-s_);
        if (!Ns_D_is_sq)
            s = s_;
        if (!Ns_D_is_sq)
            c = r;
        const Nt = mod(c * (r - _1n$8) * D_MINUS_ONE_SQ$1 - D);
        const s2 = s * s;
        const W0 = mod((s + s) * D);
        const W1 = mod(Nt * SQRT_AD_MINUS_ONE$1);
        const W2 = mod(_1n$8 - s2);
        const W3 = mod(_1n$8 + s2);
        return new ed25519$1.ExtendedPoint(mod(W0 * W3), mod(W2 * W1), mod(W1 * W3), mod(W0 * W2));
    }
    class RistPoint {
        constructor(ep) {
            this.ep = ep;
        }
        static fromAffine(ap) {
            return new RistPoint(ed25519$1.ExtendedPoint.fromAffine(ap));
        }
        static hashToCurve(hex) {
            hex = ensureBytes$1('ristrettoHash', hex, 64);
            const r1 = bytes255ToNumberLE$1(hex.slice(0, 32));
            const R1 = calcElligatorRistrettoMap$1(r1);
            const r2 = bytes255ToNumberLE$1(hex.slice(32, 64));
            const R2 = calcElligatorRistrettoMap$1(r2);
            return new RistPoint(R1.add(R2));
        }
        static fromHex(hex) {
            hex = ensureBytes$1('ristrettoHex', hex, 32);
            const { a, d } = ed25519$1.CURVE;
            const P = ed25519$1.CURVE.Fp.ORDER;
            const mod = ed25519$1.CURVE.Fp.create;
            const emsg = 'RistrettoPoint.fromHex: the hex is not valid encoding of RistrettoPoint';
            const s = bytes255ToNumberLE$1(hex);
            if (!equalBytes$1(numberToBytesLE$2(s, 32), hex) || isNegativeLE$1(s, P))
                throw new Error(emsg);
            const s2 = mod(s * s);
            const u1 = mod(_1n$8 + a * s2);
            const u2 = mod(_1n$8 - a * s2);
            const u1_2 = mod(u1 * u1);
            const u2_2 = mod(u2 * u2);
            const v = mod(a * d * u1_2 - u2_2);
            const { isValid, value: I } = invertSqrt$1(mod(v * u2_2));
            const Dx = mod(I * u2);
            const Dy = mod(I * Dx * v);
            let x = mod((s + s) * Dx);
            if (isNegativeLE$1(x, P))
                x = mod(-x);
            const y = mod(u1 * Dy);
            const t = mod(x * y);
            if (!isValid || isNegativeLE$1(t, P) || y === _0n$9)
                throw new Error(emsg);
            return new RistPoint(new ed25519$1.ExtendedPoint(x, y, _1n$8, t));
        }
        toRawBytes() {
            let { ex: x, ey: y, ez: z, et: t } = this.ep;
            const P = ed25519$1.CURVE.Fp.ORDER;
            const mod = ed25519$1.CURVE.Fp.create;
            const u1 = mod(mod(z + y) * mod(z - y));
            const u2 = mod(x * y);
            const u2sq = mod(u2 * u2);
            const { value: invsqrt } = invertSqrt$1(mod(u1 * u2sq));
            const D1 = mod(invsqrt * u1);
            const D2 = mod(invsqrt * u2);
            const zInv = mod(D1 * D2 * t);
            let D;
            if (isNegativeLE$1(t * zInv, P)) {
                let _x = mod(y * SQRT_M1$1);
                let _y = mod(x * SQRT_M1$1);
                x = _x;
                y = _y;
                D = mod(D1 * INVSQRT_A_MINUS_D$1);
            }
            else {
                D = D2;
            }
            if (isNegativeLE$1(x * zInv, P))
                y = mod(-y);
            let s = mod((z - y) * D);
            if (isNegativeLE$1(s, P))
                s = mod(-s);
            return numberToBytesLE$2(s, 32);
        }
        toHex() {
            return bytesToHex$1(this.toRawBytes());
        }
        toString() {
            return this.toHex();
        }
        equals(other) {
            assertRstPoint(other);
            const { ex: X1, ey: Y1 } = this.ep;
            const { ex: X2, ey: Y2 } = other.ep;
            const mod = ed25519$1.CURVE.Fp.create;
            const one = mod(X1 * Y2) === mod(Y1 * X2);
            const two = mod(Y1 * Y2) === mod(X1 * X2);
            return one || two;
        }
        add(other) {
            assertRstPoint(other);
            return new RistPoint(this.ep.add(other.ep));
        }
        subtract(other) {
            assertRstPoint(other);
            return new RistPoint(this.ep.subtract(other.ep));
        }
        multiply(scalar) {
            return new RistPoint(this.ep.multiply(scalar));
        }
        multiplyUnsafe(scalar) {
            return new RistPoint(this.ep.multiplyUnsafe(scalar));
        }
        double() {
            return new RistPoint(this.ep.double());
        }
        negate() {
            return new RistPoint(this.ep.negate());
        }
    }
    (() => {
        if (!RistPoint.BASE)
            RistPoint.BASE = new RistPoint(ed25519$1.ExtendedPoint.BASE);
        if (!RistPoint.ZERO)
            RistPoint.ZERO = new RistPoint(ed25519$1.ExtendedPoint.ZERO);
        return RistPoint;
    })();

    function ed25519PairFromSeed(seed, onlyJs) {
        if (!util.hasBigInt || (!onlyJs && isReady())) {
            const full = ed25519KeypairFromSeed(seed);
            return {
                publicKey: full.slice(32),
                secretKey: full.slice(0, 64)
            };
        }
        const publicKey = ed25519$1.getPublicKey(seed);
        return {
            publicKey,
            secretKey: util.u8aConcatStrict([seed, publicKey])
        };
    }

    function ed25519PairFromRandom() {
        return ed25519PairFromSeed(randomAsU8a());
    }

    function ed25519PairFromSecret(secretKey) {
        if (secretKey.length !== 64) {
            throw new Error('Invalid secretKey provided');
        }
        return {
            publicKey: secretKey.slice(32),
            secretKey
        };
    }

    function ed25519PairFromString(value) {
        return ed25519PairFromSeed(blake2AsU8a(util.stringToU8a(value)));
    }

    function ed25519Sign(message, { publicKey, secretKey }, onlyJs) {
        if (!secretKey) {
            throw new Error('Expected a valid secretKey');
        }
        else if (!publicKey) {
            throw new Error('Expected a valid publicKey');
        }
        const messageU8a = util.u8aToU8a(message);
        const privateU8a = secretKey.subarray(0, 32);
        return !util.hasBigInt || (!onlyJs && isReady())
            ? ed25519Sign$1(publicKey, privateU8a, messageU8a)
            : ed25519$1.sign(messageU8a, privateU8a);
    }

    function ed25519Verify(message, signature, publicKey, onlyJs) {
        const messageU8a = util.u8aToU8a(message);
        const publicKeyU8a = util.u8aToU8a(publicKey);
        const signatureU8a = util.u8aToU8a(signature);
        if (publicKeyU8a.length !== 32) {
            throw new Error(`Invalid publicKey, received ${publicKeyU8a.length}, expected 32`);
        }
        else if (signatureU8a.length !== 64) {
            throw new Error(`Invalid signature, received ${signatureU8a.length} bytes, expected 64`);
        }
        try {
            return !util.hasBigInt || (!onlyJs && isReady())
                ? ed25519Verify$1(signatureU8a, messageU8a, publicKeyU8a)
                : ed25519$1.verify(signatureU8a, messageU8a, publicKeyU8a);
        }
        catch {
            return false;
        }
    }

    const keyHdkdEd25519 =  createSeedDeriveFn(ed25519PairFromSeed, ed25519DeriveHard);

    const crypto = typeof globalThis === 'object' && 'crypto' in globalThis ? globalThis.crypto : undefined;

    /*! noble-hashes - MIT License (c) 2022 Paul Miller (paulmillr.com) */
    function isBytes$1(a) {
        return a instanceof Uint8Array || (ArrayBuffer.isView(a) && a.constructor.name === 'Uint8Array');
    }
    function anumber(n) {
        if (!Number.isSafeInteger(n) || n < 0)
            throw new Error('positive integer expected, got ' + n);
    }
    function abytes$1(b, ...lengths) {
        if (!isBytes$1(b))
            throw new Error('Uint8Array expected');
        if (lengths.length > 0 && !lengths.includes(b.length))
            throw new Error('Uint8Array expected of length ' + lengths + ', got length=' + b.length);
    }
    function aexists(instance, checkFinished = true) {
        if (instance.destroyed)
            throw new Error('Hash instance has been destroyed');
        if (checkFinished && instance.finished)
            throw new Error('Hash#digest() has already been called');
    }
    function aoutput(out, instance) {
        abytes$1(out);
        const min = instance.outputLen;
        if (out.length < min) {
            throw new Error('digestInto() expects output buffer of length at least ' + min);
        }
    }
    function u32(arr) {
        return new Uint32Array(arr.buffer, arr.byteOffset, Math.floor(arr.byteLength / 4));
    }
    function clean(...arrays) {
        for (let i = 0; i < arrays.length; i++) {
            arrays[i].fill(0);
        }
    }
    function createView(arr) {
        return new DataView(arr.buffer, arr.byteOffset, arr.byteLength);
    }
    function rotr(word, shift) {
        return (word << (32 - shift)) | (word >>> shift);
    }
    const isLE =  (() => new Uint8Array(new Uint32Array([0x11223344]).buffer)[0] === 0x44)();
    function byteSwap(word) {
        return (((word << 24) & 0xff000000) |
            ((word << 8) & 0xff0000) |
            ((word >>> 8) & 0xff00) |
            ((word >>> 24) & 0xff));
    }
    function byteSwap32(arr) {
        for (let i = 0; i < arr.length; i++) {
            arr[i] = byteSwap(arr[i]);
        }
        return arr;
    }
    const swap32IfBE = isLE
        ? (u) => u
        : byteSwap32;
    const hasHexBuiltin =  (() =>
    typeof Uint8Array.from([]).toHex === 'function' && typeof Uint8Array.fromHex === 'function')();
    const hexes =  Array.from({ length: 256 }, (_, i) => i.toString(16).padStart(2, '0'));
    function bytesToHex(bytes) {
        abytes$1(bytes);
        if (hasHexBuiltin)
            return bytes.toHex();
        let hex = '';
        for (let i = 0; i < bytes.length; i++) {
            hex += hexes[bytes[i]];
        }
        return hex;
    }
    const asciis = { _0: 48, _9: 57, A: 65, F: 70, a: 97, f: 102 };
    function asciiToBase16(ch) {
        if (ch >= asciis._0 && ch <= asciis._9)
            return ch - asciis._0;
        if (ch >= asciis.A && ch <= asciis.F)
            return ch - (asciis.A - 10);
        if (ch >= asciis.a && ch <= asciis.f)
            return ch - (asciis.a - 10);
        return;
    }
    function hexToBytes(hex) {
        if (typeof hex !== 'string')
            throw new Error('hex string expected, got ' + typeof hex);
        if (hasHexBuiltin)
            return Uint8Array.fromHex(hex);
        const hl = hex.length;
        const al = hl / 2;
        if (hl % 2)
            throw new Error('hex string expected, got unpadded hex of length ' + hl);
        const array = new Uint8Array(al);
        for (let ai = 0, hi = 0; ai < al; ai++, hi += 2) {
            const n1 = asciiToBase16(hex.charCodeAt(hi));
            const n2 = asciiToBase16(hex.charCodeAt(hi + 1));
            if (n1 === undefined || n2 === undefined) {
                const char = hex[hi] + hex[hi + 1];
                throw new Error('hex string expected, got non-hex character "' + char + '" at index ' + hi);
            }
            array[ai] = n1 * 16 + n2;
        }
        return array;
    }
    function utf8ToBytes(str) {
        if (typeof str !== 'string')
            throw new Error('string expected');
        return new Uint8Array(new TextEncoder().encode(str));
    }
    function toBytes(data) {
        if (typeof data === 'string')
            data = utf8ToBytes(data);
        abytes$1(data);
        return data;
    }
    function concatBytes(...arrays) {
        let sum = 0;
        for (let i = 0; i < arrays.length; i++) {
            const a = arrays[i];
            abytes$1(a);
            sum += a.length;
        }
        const res = new Uint8Array(sum);
        for (let i = 0, pad = 0; i < arrays.length; i++) {
            const a = arrays[i];
            res.set(a, pad);
            pad += a.length;
        }
        return res;
    }
    class Hash {
    }
    function createHasher$1(hashCons) {
        const hashC = (msg) => hashCons().update(toBytes(msg)).digest();
        const tmp = hashCons();
        hashC.outputLen = tmp.outputLen;
        hashC.blockLen = tmp.blockLen;
        hashC.create = () => hashCons();
        return hashC;
    }
    function createXOFer(hashCons) {
        const hashC = (msg, opts) => hashCons(opts).update(toBytes(msg)).digest();
        const tmp = hashCons({});
        hashC.outputLen = tmp.outputLen;
        hashC.blockLen = tmp.blockLen;
        hashC.create = (opts) => hashCons(opts);
        return hashC;
    }
    function randomBytes(bytesLength = 32) {
        if (crypto && typeof crypto.getRandomValues === 'function') {
            return crypto.getRandomValues(new Uint8Array(bytesLength));
        }
        if (crypto && typeof crypto.randomBytes === 'function') {
            return Uint8Array.from(crypto.randomBytes(bytesLength));
        }
        throw new Error('crypto.getRandomValues must be defined');
    }

    /*! noble-curves - MIT License (c) 2022 Paul Miller (paulmillr.com) */
    const _0n$8 =  BigInt(0);
    const _1n$7 =  BigInt(1);
    function _abool2(value, title = '') {
        if (typeof value !== 'boolean') {
            const prefix = title && `"${title}"`;
            throw new Error(prefix + 'expected boolean, got type=' + typeof value);
        }
        return value;
    }
    function _abytes2(value, length, title = '') {
        const bytes = isBytes$1(value);
        const len = value?.length;
        const needsLen = length !== undefined;
        if (!bytes || (needsLen && len !== length)) {
            const prefix = title && `"${title}" `;
            const ofLen = needsLen ? ` of length ${length}` : '';
            const got = bytes ? `length=${len}` : `type=${typeof value}`;
            throw new Error(prefix + 'expected Uint8Array' + ofLen + ', got ' + got);
        }
        return value;
    }
    function hexToNumber(hex) {
        if (typeof hex !== 'string')
            throw new Error('hex string expected, got ' + typeof hex);
        return hex === '' ? _0n$8 : BigInt('0x' + hex);
    }
    function bytesToNumberBE(bytes) {
        return hexToNumber(bytesToHex(bytes));
    }
    function bytesToNumberLE$1(bytes) {
        abytes$1(bytes);
        return hexToNumber(bytesToHex(Uint8Array.from(bytes).reverse()));
    }
    function numberToBytesBE(n, len) {
        return hexToBytes(n.toString(16).padStart(len * 2, '0'));
    }
    function numberToBytesLE$1(n, len) {
        return numberToBytesBE(n, len).reverse();
    }
    function ensureBytes(title, hex, expectedLength) {
        let res;
        if (typeof hex === 'string') {
            try {
                res = hexToBytes(hex);
            }
            catch (e) {
                throw new Error(title + ' must be hex string or Uint8Array, cause: ' + e);
            }
        }
        else if (isBytes$1(hex)) {
            res = Uint8Array.from(hex);
        }
        else {
            throw new Error(title + ' must be hex string or Uint8Array');
        }
        const len = res.length;
        if (typeof expectedLength === 'number' && len !== expectedLength)
            throw new Error(title + ' of length ' + expectedLength + ' expected, got ' + len);
        return res;
    }
    function equalBytes(a, b) {
        if (a.length !== b.length)
            return false;
        let diff = 0;
        for (let i = 0; i < a.length; i++)
            diff |= a[i] ^ b[i];
        return diff === 0;
    }
    function copyBytes(bytes) {
        return Uint8Array.from(bytes);
    }
    const isPosBig = (n) => typeof n === 'bigint' && _0n$8 <= n;
    function inRange(n, min, max) {
        return isPosBig(n) && isPosBig(min) && isPosBig(max) && min <= n && n < max;
    }
    function aInRange$1(title, n, min, max) {
        if (!inRange(n, min, max))
            throw new Error('expected valid ' + title + ': ' + min + ' <= n < ' + max + ', got ' + n);
    }
    function bitLen(n) {
        let len;
        for (len = 0; n > _0n$8; n >>= _1n$7, len += 1)
            ;
        return len;
    }
    const bitMask$1 = (n) => (_1n$7 << BigInt(n)) - _1n$7;
    function isHash(val) {
        return typeof val === 'function' && Number.isSafeInteger(val.outputLen);
    }
    function _validateObject(object, fields, optFields = {}) {
        if (!object || typeof object !== 'object')
            throw new Error('expected valid options object');
        function checkField(fieldName, expectedType, isOpt) {
            const val = object[fieldName];
            if (isOpt && val === undefined)
                return;
            const current = typeof val;
            if (current !== expectedType || val === null)
                throw new Error(`param "${fieldName}" is invalid: expected ${expectedType}, got ${current}`);
        }
        Object.entries(fields).forEach(([k, v]) => checkField(k, v, false));
        Object.entries(optFields).forEach(([k, v]) => checkField(k, v, true));
    }
    const notImplemented = () => {
        throw new Error('not implemented');
    };
    function memoized(fn) {
        const map = new WeakMap();
        return (arg, ...args) => {
            const val = map.get(arg);
            if (val !== undefined)
                return val;
            const computed = fn(arg, ...args);
            map.set(arg, computed);
            return computed;
        };
    }

    /*! noble-curves - MIT License (c) 2022 Paul Miller (paulmillr.com) */
    const _0n$7 = BigInt(0), _1n$6 = BigInt(1), _2n$5 =  BigInt(2), _3n$2 =  BigInt(3);
    const _4n =  BigInt(4), _5n$1 =  BigInt(5), _7n$3 =  BigInt(7);
    const _8n$2 =  BigInt(8), _9n =  BigInt(9), _16n$1 =  BigInt(16);
    function mod(a, b) {
        const result = a % b;
        return result >= _0n$7 ? result : b + result;
    }
    function pow2(x, power, modulo) {
        let res = x;
        while (power-- > _0n$7) {
            res *= res;
            res %= modulo;
        }
        return res;
    }
    function invert(number, modulo) {
        if (number === _0n$7)
            throw new Error('invert: expected non-zero number');
        if (modulo <= _0n$7)
            throw new Error('invert: expected positive modulus, got ' + modulo);
        let a = mod(number, modulo);
        let b = modulo;
        let x = _0n$7, u = _1n$6;
        while (a !== _0n$7) {
            const q = b / a;
            const r = b % a;
            const m = x - u * q;
            b = a, a = r, x = u, u = m;
        }
        const gcd = b;
        if (gcd !== _1n$6)
            throw new Error('invert: does not exist');
        return mod(x, modulo);
    }
    function assertIsSquare(Fp, root, n) {
        if (!Fp.eql(Fp.sqr(root), n))
            throw new Error('Cannot find square root');
    }
    function sqrt3mod4(Fp, n) {
        const p1div4 = (Fp.ORDER + _1n$6) / _4n;
        const root = Fp.pow(n, p1div4);
        assertIsSquare(Fp, root, n);
        return root;
    }
    function sqrt5mod8(Fp, n) {
        const p5div8 = (Fp.ORDER - _5n$1) / _8n$2;
        const n2 = Fp.mul(n, _2n$5);
        const v = Fp.pow(n2, p5div8);
        const nv = Fp.mul(n, v);
        const i = Fp.mul(Fp.mul(nv, _2n$5), v);
        const root = Fp.mul(nv, Fp.sub(i, Fp.ONE));
        assertIsSquare(Fp, root, n);
        return root;
    }
    function sqrt9mod16(P) {
        const Fp_ = Field(P);
        const tn = tonelliShanks(P);
        const c1 = tn(Fp_, Fp_.neg(Fp_.ONE));
        const c2 = tn(Fp_, c1);
        const c3 = tn(Fp_, Fp_.neg(c1));
        const c4 = (P + _7n$3) / _16n$1;
        return (Fp, n) => {
            let tv1 = Fp.pow(n, c4);
            let tv2 = Fp.mul(tv1, c1);
            const tv3 = Fp.mul(tv1, c2);
            const tv4 = Fp.mul(tv1, c3);
            const e1 = Fp.eql(Fp.sqr(tv2), n);
            const e2 = Fp.eql(Fp.sqr(tv3), n);
            tv1 = Fp.cmov(tv1, tv2, e1);
            tv2 = Fp.cmov(tv4, tv3, e2);
            const e3 = Fp.eql(Fp.sqr(tv2), n);
            const root = Fp.cmov(tv1, tv2, e3);
            assertIsSquare(Fp, root, n);
            return root;
        };
    }
    function tonelliShanks(P) {
        if (P < _3n$2)
            throw new Error('sqrt is not defined for small field');
        let Q = P - _1n$6;
        let S = 0;
        while (Q % _2n$5 === _0n$7) {
            Q /= _2n$5;
            S++;
        }
        let Z = _2n$5;
        const _Fp = Field(P);
        while (FpLegendre(_Fp, Z) === 1) {
            if (Z++ > 1000)
                throw new Error('Cannot find square root: probably non-prime P');
        }
        if (S === 1)
            return sqrt3mod4;
        let cc = _Fp.pow(Z, Q);
        const Q1div2 = (Q + _1n$6) / _2n$5;
        return function tonelliSlow(Fp, n) {
            if (Fp.is0(n))
                return n;
            if (FpLegendre(Fp, n) !== 1)
                throw new Error('Cannot find square root');
            let M = S;
            let c = Fp.mul(Fp.ONE, cc);
            let t = Fp.pow(n, Q);
            let R = Fp.pow(n, Q1div2);
            while (!Fp.eql(t, Fp.ONE)) {
                if (Fp.is0(t))
                    return Fp.ZERO;
                let i = 1;
                let t_tmp = Fp.sqr(t);
                while (!Fp.eql(t_tmp, Fp.ONE)) {
                    i++;
                    t_tmp = Fp.sqr(t_tmp);
                    if (i === M)
                        throw new Error('Cannot find square root');
                }
                const exponent = _1n$6 << BigInt(M - i - 1);
                const b = Fp.pow(c, exponent);
                M = i;
                c = Fp.sqr(b);
                t = Fp.mul(t, c);
                R = Fp.mul(R, b);
            }
            return R;
        };
    }
    function FpSqrt(P) {
        if (P % _4n === _3n$2)
            return sqrt3mod4;
        if (P % _8n$2 === _5n$1)
            return sqrt5mod8;
        if (P % _16n$1 === _9n)
            return sqrt9mod16(P);
        return tonelliShanks(P);
    }
    const isNegativeLE = (num, modulo) => (mod(num, modulo) & _1n$6) === _1n$6;
    const FIELD_FIELDS = [
        'create', 'isValid', 'is0', 'neg', 'inv', 'sqrt', 'sqr',
        'eql', 'add', 'sub', 'mul', 'pow', 'div',
        'addN', 'subN', 'mulN', 'sqrN'
    ];
    function validateField(field) {
        const initial = {
            ORDER: 'bigint',
            MASK: 'bigint',
            BYTES: 'number',
            BITS: 'number',
        };
        const opts = FIELD_FIELDS.reduce((map, val) => {
            map[val] = 'function';
            return map;
        }, initial);
        _validateObject(field, opts);
        return field;
    }
    function FpPow(Fp, num, power) {
        if (power < _0n$7)
            throw new Error('invalid exponent, negatives unsupported');
        if (power === _0n$7)
            return Fp.ONE;
        if (power === _1n$6)
            return num;
        let p = Fp.ONE;
        let d = num;
        while (power > _0n$7) {
            if (power & _1n$6)
                p = Fp.mul(p, d);
            d = Fp.sqr(d);
            power >>= _1n$6;
        }
        return p;
    }
    function FpInvertBatch(Fp, nums, passZero = false) {
        const inverted = new Array(nums.length).fill(passZero ? Fp.ZERO : undefined);
        const multipliedAcc = nums.reduce((acc, num, i) => {
            if (Fp.is0(num))
                return acc;
            inverted[i] = acc;
            return Fp.mul(acc, num);
        }, Fp.ONE);
        const invertedAcc = Fp.inv(multipliedAcc);
        nums.reduceRight((acc, num, i) => {
            if (Fp.is0(num))
                return acc;
            inverted[i] = Fp.mul(acc, inverted[i]);
            return Fp.mul(acc, num);
        }, invertedAcc);
        return inverted;
    }
    function FpLegendre(Fp, n) {
        const p1mod2 = (Fp.ORDER - _1n$6) / _2n$5;
        const powered = Fp.pow(n, p1mod2);
        const yes = Fp.eql(powered, Fp.ONE);
        const zero = Fp.eql(powered, Fp.ZERO);
        const no = Fp.eql(powered, Fp.neg(Fp.ONE));
        if (!yes && !zero && !no)
            throw new Error('invalid Legendre symbol result');
        return yes ? 1 : zero ? 0 : -1;
    }
    function nLength(n, nBitLength) {
        if (nBitLength !== undefined)
            anumber(nBitLength);
        const _nBitLength = nBitLength !== undefined ? nBitLength : n.toString(2).length;
        const nByteLength = Math.ceil(_nBitLength / 8);
        return { nBitLength: _nBitLength, nByteLength };
    }
    function Field(ORDER, bitLenOrOpts,
    isLE = false, opts = {}) {
        if (ORDER <= _0n$7)
            throw new Error('invalid field: expected ORDER > 0, got ' + ORDER);
        let _nbitLength = undefined;
        let _sqrt = undefined;
        let modFromBytes = false;
        let allowedLengths = undefined;
        if (typeof bitLenOrOpts === 'object' && bitLenOrOpts != null) {
            if (opts.sqrt || isLE)
                throw new Error('cannot specify opts in two arguments');
            const _opts = bitLenOrOpts;
            if (_opts.BITS)
                _nbitLength = _opts.BITS;
            if (_opts.sqrt)
                _sqrt = _opts.sqrt;
            if (typeof _opts.isLE === 'boolean')
                isLE = _opts.isLE;
            if (typeof _opts.modFromBytes === 'boolean')
                modFromBytes = _opts.modFromBytes;
            allowedLengths = _opts.allowedLengths;
        }
        else {
            if (typeof bitLenOrOpts === 'number')
                _nbitLength = bitLenOrOpts;
            if (opts.sqrt)
                _sqrt = opts.sqrt;
        }
        const { nBitLength: BITS, nByteLength: BYTES } = nLength(ORDER, _nbitLength);
        if (BYTES > 2048)
            throw new Error('invalid field: expected ORDER of <= 2048 bytes');
        let sqrtP;
        const f = Object.freeze({
            ORDER,
            isLE,
            BITS,
            BYTES,
            MASK: bitMask$1(BITS),
            ZERO: _0n$7,
            ONE: _1n$6,
            allowedLengths: allowedLengths,
            create: (num) => mod(num, ORDER),
            isValid: (num) => {
                if (typeof num !== 'bigint')
                    throw new Error('invalid field element: expected bigint, got ' + typeof num);
                return _0n$7 <= num && num < ORDER;
            },
            is0: (num) => num === _0n$7,
            isValidNot0: (num) => !f.is0(num) && f.isValid(num),
            isOdd: (num) => (num & _1n$6) === _1n$6,
            neg: (num) => mod(-num, ORDER),
            eql: (lhs, rhs) => lhs === rhs,
            sqr: (num) => mod(num * num, ORDER),
            add: (lhs, rhs) => mod(lhs + rhs, ORDER),
            sub: (lhs, rhs) => mod(lhs - rhs, ORDER),
            mul: (lhs, rhs) => mod(lhs * rhs, ORDER),
            pow: (num, power) => FpPow(f, num, power),
            div: (lhs, rhs) => mod(lhs * invert(rhs, ORDER), ORDER),
            sqrN: (num) => num * num,
            addN: (lhs, rhs) => lhs + rhs,
            subN: (lhs, rhs) => lhs - rhs,
            mulN: (lhs, rhs) => lhs * rhs,
            inv: (num) => invert(num, ORDER),
            sqrt: _sqrt ||
                ((n) => {
                    if (!sqrtP)
                        sqrtP = FpSqrt(ORDER);
                    return sqrtP(f, n);
                }),
            toBytes: (num) => (isLE ? numberToBytesLE$1(num, BYTES) : numberToBytesBE(num, BYTES)),
            fromBytes: (bytes, skipValidation = true) => {
                if (allowedLengths) {
                    if (!allowedLengths.includes(bytes.length) || bytes.length > BYTES) {
                        throw new Error('Field.fromBytes: expected ' + allowedLengths + ' bytes, got ' + bytes.length);
                    }
                    const padded = new Uint8Array(BYTES);
                    padded.set(bytes, isLE ? 0 : padded.length - bytes.length);
                    bytes = padded;
                }
                if (bytes.length !== BYTES)
                    throw new Error('Field.fromBytes: expected ' + BYTES + ' bytes, got ' + bytes.length);
                let scalar = isLE ? bytesToNumberLE$1(bytes) : bytesToNumberBE(bytes);
                if (modFromBytes)
                    scalar = mod(scalar, ORDER);
                if (!skipValidation)
                    if (!f.isValid(scalar))
                        throw new Error('invalid field element: outside of range 0..ORDER');
                return scalar;
            },
            invertBatch: (lst) => FpInvertBatch(f, lst),
            cmov: (a, b, c) => (c ? b : a),
        });
        return Object.freeze(f);
    }
    function FpSqrtEven(Fp, elm) {
        if (!Fp.isOdd)
            throw new Error("Field doesn't have isOdd");
        const root = Fp.sqrt(elm);
        return Fp.isOdd(root) ? Fp.neg(root) : root;
    }

    const isBytes = isBytes$1;
    const bytesToNumberLE = bytesToNumberLE$1;
    const numberToBytesLE = numberToBytesLE$1;
    const aInRange = aInRange$1;
    const bitMask = bitMask$1;

    function setBigUint64(view, byteOffset, value, isLE) {
        if (typeof view.setBigUint64 === 'function')
            return view.setBigUint64(byteOffset, value, isLE);
        const _32n = BigInt(32);
        const _u32_max = BigInt(0xffffffff);
        const wh = Number((value >> _32n) & _u32_max);
        const wl = Number(value & _u32_max);
        const h = isLE ? 4 : 0;
        const l = isLE ? 0 : 4;
        view.setUint32(byteOffset + h, wh, isLE);
        view.setUint32(byteOffset + l, wl, isLE);
    }
    function Chi(a, b, c) {
        return (a & b) ^ (~a & c);
    }
    function Maj(a, b, c) {
        return (a & b) ^ (a & c) ^ (b & c);
    }
    class HashMD extends Hash {
        constructor(blockLen, outputLen, padOffset, isLE) {
            super();
            this.finished = false;
            this.length = 0;
            this.pos = 0;
            this.destroyed = false;
            this.blockLen = blockLen;
            this.outputLen = outputLen;
            this.padOffset = padOffset;
            this.isLE = isLE;
            this.buffer = new Uint8Array(blockLen);
            this.view = createView(this.buffer);
        }
        update(data) {
            aexists(this);
            data = toBytes(data);
            abytes$1(data);
            const { view, buffer, blockLen } = this;
            const len = data.length;
            for (let pos = 0; pos < len;) {
                const take = Math.min(blockLen - this.pos, len - pos);
                if (take === blockLen) {
                    const dataView = createView(data);
                    for (; blockLen <= len - pos; pos += blockLen)
                        this.process(dataView, pos);
                    continue;
                }
                buffer.set(data.subarray(pos, pos + take), this.pos);
                this.pos += take;
                pos += take;
                if (this.pos === blockLen) {
                    this.process(view, 0);
                    this.pos = 0;
                }
            }
            this.length += data.length;
            this.roundClean();
            return this;
        }
        digestInto(out) {
            aexists(this);
            aoutput(out, this);
            this.finished = true;
            const { buffer, view, blockLen, isLE } = this;
            let { pos } = this;
            buffer[pos++] = 0b10000000;
            clean(this.buffer.subarray(pos));
            if (this.padOffset > blockLen - pos) {
                this.process(view, 0);
                pos = 0;
            }
            for (let i = pos; i < blockLen; i++)
                buffer[i] = 0;
            setBigUint64(view, blockLen - 8, BigInt(this.length * 8), isLE);
            this.process(view, 0);
            const oview = createView(out);
            const len = this.outputLen;
            if (len % 4)
                throw new Error('_sha2: outputLen should be aligned to 32bit');
            const outLen = len / 4;
            const state = this.get();
            if (outLen > state.length)
                throw new Error('_sha2: outputLen bigger than state');
            for (let i = 0; i < outLen; i++)
                oview.setUint32(4 * i, state[i], isLE);
        }
        digest() {
            const { buffer, outputLen } = this;
            this.digestInto(buffer);
            const res = buffer.slice(0, outputLen);
            this.destroy();
            return res;
        }
        _cloneInto(to) {
            to || (to = new this.constructor());
            to.set(...this.get());
            const { blockLen, buffer, length, finished, destroyed, pos } = this;
            to.destroyed = destroyed;
            to.finished = finished;
            to.length = length;
            to.pos = pos;
            if (length % blockLen)
                to.buffer.set(buffer);
            return to;
        }
        clone() {
            return this._cloneInto();
        }
    }
    const SHA256_IV =  Uint32Array.from([
        0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab, 0x5be0cd19,
    ]);
    const SHA224_IV =  Uint32Array.from([
        0xc1059ed8, 0x367cd507, 0x3070dd17, 0xf70e5939, 0xffc00b31, 0x68581511, 0x64f98fa7, 0xbefa4fa4,
    ]);
    const SHA384_IV =  Uint32Array.from([
        0xcbbb9d5d, 0xc1059ed8, 0x629a292a, 0x367cd507, 0x9159015a, 0x3070dd17, 0x152fecd8, 0xf70e5939,
        0x67332667, 0xffc00b31, 0x8eb44a87, 0x68581511, 0xdb0c2e0d, 0x64f98fa7, 0x47b5481d, 0xbefa4fa4,
    ]);
    const SHA512_IV =  Uint32Array.from([
        0x6a09e667, 0xf3bcc908, 0xbb67ae85, 0x84caa73b, 0x3c6ef372, 0xfe94f82b, 0xa54ff53a, 0x5f1d36f1,
        0x510e527f, 0xade682d1, 0x9b05688c, 0x2b3e6c1f, 0x1f83d9ab, 0xfb41bd6b, 0x5be0cd19, 0x137e2179,
    ]);

    const U32_MASK64 =  BigInt(2 ** 32 - 1);
    const _32n$1 =  BigInt(32);
    function fromBig(n, le = false) {
        if (le)
            return { h: Number(n & U32_MASK64), l: Number((n >> _32n$1) & U32_MASK64) };
        return { h: Number((n >> _32n$1) & U32_MASK64) | 0, l: Number(n & U32_MASK64) | 0 };
    }
    function split(lst, le = false) {
        const len = lst.length;
        let Ah = new Uint32Array(len);
        let Al = new Uint32Array(len);
        for (let i = 0; i < len; i++) {
            const { h, l } = fromBig(lst[i], le);
            [Ah[i], Al[i]] = [h, l];
        }
        return [Ah, Al];
    }
    const shrSH = (h, _l, s) => h >>> s;
    const shrSL = (h, l, s) => (h << (32 - s)) | (l >>> s);
    const rotrSH = (h, l, s) => (h >>> s) | (l << (32 - s));
    const rotrSL = (h, l, s) => (h << (32 - s)) | (l >>> s);
    const rotrBH = (h, l, s) => (h << (64 - s)) | (l >>> (s - 32));
    const rotrBL = (h, l, s) => (h >>> (s - 32)) | (l << (64 - s));
    const rotlSH = (h, l, s) => (h << s) | (l >>> (32 - s));
    const rotlSL = (h, l, s) => (l << s) | (h >>> (32 - s));
    const rotlBH = (h, l, s) => (l << (s - 32)) | (h >>> (64 - s));
    const rotlBL = (h, l, s) => (h << (s - 32)) | (l >>> (64 - s));
    function add(Ah, Al, Bh, Bl) {
        const l = (Al >>> 0) + (Bl >>> 0);
        return { h: (Ah + Bh + ((l / 2 ** 32) | 0)) | 0, l: l | 0 };
    }
    const add3L = (Al, Bl, Cl) => (Al >>> 0) + (Bl >>> 0) + (Cl >>> 0);
    const add3H = (low, Ah, Bh, Ch) => (Ah + Bh + Ch + ((low / 2 ** 32) | 0)) | 0;
    const add4L = (Al, Bl, Cl, Dl) => (Al >>> 0) + (Bl >>> 0) + (Cl >>> 0) + (Dl >>> 0);
    const add4H = (low, Ah, Bh, Ch, Dh) => (Ah + Bh + Ch + Dh + ((low / 2 ** 32) | 0)) | 0;
    const add5L = (Al, Bl, Cl, Dl, El) => (Al >>> 0) + (Bl >>> 0) + (Cl >>> 0) + (Dl >>> 0) + (El >>> 0);
    const add5H = (low, Ah, Bh, Ch, Dh, Eh) => (Ah + Bh + Ch + Dh + Eh + ((low / 2 ** 32) | 0)) | 0;

    const SHA256_K =  Uint32Array.from([
        0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4, 0xab1c5ed5,
        0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe, 0x9bdc06a7, 0xc19bf174,
        0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f, 0x4a7484aa, 0x5cb0a9dc, 0x76f988da,
        0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7, 0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967,
        0x27b70a85, 0x2e1b2138, 0x4d2c6dfc, 0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85,
        0xa2bfe8a1, 0xa81a664b, 0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070,
        0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
        0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7, 0xc67178f2
    ]);
    const SHA256_W =  new Uint32Array(64);
    class SHA256 extends HashMD {
        constructor(outputLen = 32) {
            super(64, outputLen, 8, false);
            this.A = SHA256_IV[0] | 0;
            this.B = SHA256_IV[1] | 0;
            this.C = SHA256_IV[2] | 0;
            this.D = SHA256_IV[3] | 0;
            this.E = SHA256_IV[4] | 0;
            this.F = SHA256_IV[5] | 0;
            this.G = SHA256_IV[6] | 0;
            this.H = SHA256_IV[7] | 0;
        }
        get() {
            const { A, B, C, D, E, F, G, H } = this;
            return [A, B, C, D, E, F, G, H];
        }
        set(A, B, C, D, E, F, G, H) {
            this.A = A | 0;
            this.B = B | 0;
            this.C = C | 0;
            this.D = D | 0;
            this.E = E | 0;
            this.F = F | 0;
            this.G = G | 0;
            this.H = H | 0;
        }
        process(view, offset) {
            for (let i = 0; i < 16; i++, offset += 4)
                SHA256_W[i] = view.getUint32(offset, false);
            for (let i = 16; i < 64; i++) {
                const W15 = SHA256_W[i - 15];
                const W2 = SHA256_W[i - 2];
                const s0 = rotr(W15, 7) ^ rotr(W15, 18) ^ (W15 >>> 3);
                const s1 = rotr(W2, 17) ^ rotr(W2, 19) ^ (W2 >>> 10);
                SHA256_W[i] = (s1 + SHA256_W[i - 7] + s0 + SHA256_W[i - 16]) | 0;
            }
            let { A, B, C, D, E, F, G, H } = this;
            for (let i = 0; i < 64; i++) {
                const sigma1 = rotr(E, 6) ^ rotr(E, 11) ^ rotr(E, 25);
                const T1 = (H + sigma1 + Chi(E, F, G) + SHA256_K[i] + SHA256_W[i]) | 0;
                const sigma0 = rotr(A, 2) ^ rotr(A, 13) ^ rotr(A, 22);
                const T2 = (sigma0 + Maj(A, B, C)) | 0;
                H = G;
                G = F;
                F = E;
                E = (D + T1) | 0;
                D = C;
                C = B;
                B = A;
                A = (T1 + T2) | 0;
            }
            A = (A + this.A) | 0;
            B = (B + this.B) | 0;
            C = (C + this.C) | 0;
            D = (D + this.D) | 0;
            E = (E + this.E) | 0;
            F = (F + this.F) | 0;
            G = (G + this.G) | 0;
            H = (H + this.H) | 0;
            this.set(A, B, C, D, E, F, G, H);
        }
        roundClean() {
            clean(SHA256_W);
        }
        destroy() {
            this.set(0, 0, 0, 0, 0, 0, 0, 0);
            clean(this.buffer);
        }
    }
    class SHA224 extends SHA256 {
        constructor() {
            super(28);
            this.A = SHA224_IV[0] | 0;
            this.B = SHA224_IV[1] | 0;
            this.C = SHA224_IV[2] | 0;
            this.D = SHA224_IV[3] | 0;
            this.E = SHA224_IV[4] | 0;
            this.F = SHA224_IV[5] | 0;
            this.G = SHA224_IV[6] | 0;
            this.H = SHA224_IV[7] | 0;
        }
    }
    const K512 =  (() => split([
        '0x428a2f98d728ae22', '0x7137449123ef65cd', '0xb5c0fbcfec4d3b2f', '0xe9b5dba58189dbbc',
        '0x3956c25bf348b538', '0x59f111f1b605d019', '0x923f82a4af194f9b', '0xab1c5ed5da6d8118',
        '0xd807aa98a3030242', '0x12835b0145706fbe', '0x243185be4ee4b28c', '0x550c7dc3d5ffb4e2',
        '0x72be5d74f27b896f', '0x80deb1fe3b1696b1', '0x9bdc06a725c71235', '0xc19bf174cf692694',
        '0xe49b69c19ef14ad2', '0xefbe4786384f25e3', '0x0fc19dc68b8cd5b5', '0x240ca1cc77ac9c65',
        '0x2de92c6f592b0275', '0x4a7484aa6ea6e483', '0x5cb0a9dcbd41fbd4', '0x76f988da831153b5',
        '0x983e5152ee66dfab', '0xa831c66d2db43210', '0xb00327c898fb213f', '0xbf597fc7beef0ee4',
        '0xc6e00bf33da88fc2', '0xd5a79147930aa725', '0x06ca6351e003826f', '0x142929670a0e6e70',
        '0x27b70a8546d22ffc', '0x2e1b21385c26c926', '0x4d2c6dfc5ac42aed', '0x53380d139d95b3df',
        '0x650a73548baf63de', '0x766a0abb3c77b2a8', '0x81c2c92e47edaee6', '0x92722c851482353b',
        '0xa2bfe8a14cf10364', '0xa81a664bbc423001', '0xc24b8b70d0f89791', '0xc76c51a30654be30',
        '0xd192e819d6ef5218', '0xd69906245565a910', '0xf40e35855771202a', '0x106aa07032bbd1b8',
        '0x19a4c116b8d2d0c8', '0x1e376c085141ab53', '0x2748774cdf8eeb99', '0x34b0bcb5e19b48a8',
        '0x391c0cb3c5c95a63', '0x4ed8aa4ae3418acb', '0x5b9cca4f7763e373', '0x682e6ff3d6b2b8a3',
        '0x748f82ee5defb2fc', '0x78a5636f43172f60', '0x84c87814a1f0ab72', '0x8cc702081a6439ec',
        '0x90befffa23631e28', '0xa4506cebde82bde9', '0xbef9a3f7b2c67915', '0xc67178f2e372532b',
        '0xca273eceea26619c', '0xd186b8c721c0c207', '0xeada7dd6cde0eb1e', '0xf57d4f7fee6ed178',
        '0x06f067aa72176fba', '0x0a637dc5a2c898a6', '0x113f9804bef90dae', '0x1b710b35131c471b',
        '0x28db77f523047d84', '0x32caab7b40c72493', '0x3c9ebe0a15c9bebc', '0x431d67c49c100d4c',
        '0x4cc5d4becb3e42b6', '0x597f299cfc657e2a', '0x5fcb6fab3ad6faec', '0x6c44198c4a475817'
    ].map(n => BigInt(n))))();
    const SHA512_Kh =  (() => K512[0])();
    const SHA512_Kl =  (() => K512[1])();
    const SHA512_W_H =  new Uint32Array(80);
    const SHA512_W_L =  new Uint32Array(80);
    class SHA512 extends HashMD {
        constructor(outputLen = 64) {
            super(128, outputLen, 16, false);
            this.Ah = SHA512_IV[0] | 0;
            this.Al = SHA512_IV[1] | 0;
            this.Bh = SHA512_IV[2] | 0;
            this.Bl = SHA512_IV[3] | 0;
            this.Ch = SHA512_IV[4] | 0;
            this.Cl = SHA512_IV[5] | 0;
            this.Dh = SHA512_IV[6] | 0;
            this.Dl = SHA512_IV[7] | 0;
            this.Eh = SHA512_IV[8] | 0;
            this.El = SHA512_IV[9] | 0;
            this.Fh = SHA512_IV[10] | 0;
            this.Fl = SHA512_IV[11] | 0;
            this.Gh = SHA512_IV[12] | 0;
            this.Gl = SHA512_IV[13] | 0;
            this.Hh = SHA512_IV[14] | 0;
            this.Hl = SHA512_IV[15] | 0;
        }
        get() {
            const { Ah, Al, Bh, Bl, Ch, Cl, Dh, Dl, Eh, El, Fh, Fl, Gh, Gl, Hh, Hl } = this;
            return [Ah, Al, Bh, Bl, Ch, Cl, Dh, Dl, Eh, El, Fh, Fl, Gh, Gl, Hh, Hl];
        }
        set(Ah, Al, Bh, Bl, Ch, Cl, Dh, Dl, Eh, El, Fh, Fl, Gh, Gl, Hh, Hl) {
            this.Ah = Ah | 0;
            this.Al = Al | 0;
            this.Bh = Bh | 0;
            this.Bl = Bl | 0;
            this.Ch = Ch | 0;
            this.Cl = Cl | 0;
            this.Dh = Dh | 0;
            this.Dl = Dl | 0;
            this.Eh = Eh | 0;
            this.El = El | 0;
            this.Fh = Fh | 0;
            this.Fl = Fl | 0;
            this.Gh = Gh | 0;
            this.Gl = Gl | 0;
            this.Hh = Hh | 0;
            this.Hl = Hl | 0;
        }
        process(view, offset) {
            for (let i = 0; i < 16; i++, offset += 4) {
                SHA512_W_H[i] = view.getUint32(offset);
                SHA512_W_L[i] = view.getUint32((offset += 4));
            }
            for (let i = 16; i < 80; i++) {
                const W15h = SHA512_W_H[i - 15] | 0;
                const W15l = SHA512_W_L[i - 15] | 0;
                const s0h = rotrSH(W15h, W15l, 1) ^ rotrSH(W15h, W15l, 8) ^ shrSH(W15h, W15l, 7);
                const s0l = rotrSL(W15h, W15l, 1) ^ rotrSL(W15h, W15l, 8) ^ shrSL(W15h, W15l, 7);
                const W2h = SHA512_W_H[i - 2] | 0;
                const W2l = SHA512_W_L[i - 2] | 0;
                const s1h = rotrSH(W2h, W2l, 19) ^ rotrBH(W2h, W2l, 61) ^ shrSH(W2h, W2l, 6);
                const s1l = rotrSL(W2h, W2l, 19) ^ rotrBL(W2h, W2l, 61) ^ shrSL(W2h, W2l, 6);
                const SUMl = add4L(s0l, s1l, SHA512_W_L[i - 7], SHA512_W_L[i - 16]);
                const SUMh = add4H(SUMl, s0h, s1h, SHA512_W_H[i - 7], SHA512_W_H[i - 16]);
                SHA512_W_H[i] = SUMh | 0;
                SHA512_W_L[i] = SUMl | 0;
            }
            let { Ah, Al, Bh, Bl, Ch, Cl, Dh, Dl, Eh, El, Fh, Fl, Gh, Gl, Hh, Hl } = this;
            for (let i = 0; i < 80; i++) {
                const sigma1h = rotrSH(Eh, El, 14) ^ rotrSH(Eh, El, 18) ^ rotrBH(Eh, El, 41);
                const sigma1l = rotrSL(Eh, El, 14) ^ rotrSL(Eh, El, 18) ^ rotrBL(Eh, El, 41);
                const CHIh = (Eh & Fh) ^ (~Eh & Gh);
                const CHIl = (El & Fl) ^ (~El & Gl);
                const T1ll = add5L(Hl, sigma1l, CHIl, SHA512_Kl[i], SHA512_W_L[i]);
                const T1h = add5H(T1ll, Hh, sigma1h, CHIh, SHA512_Kh[i], SHA512_W_H[i]);
                const T1l = T1ll | 0;
                const sigma0h = rotrSH(Ah, Al, 28) ^ rotrBH(Ah, Al, 34) ^ rotrBH(Ah, Al, 39);
                const sigma0l = rotrSL(Ah, Al, 28) ^ rotrBL(Ah, Al, 34) ^ rotrBL(Ah, Al, 39);
                const MAJh = (Ah & Bh) ^ (Ah & Ch) ^ (Bh & Ch);
                const MAJl = (Al & Bl) ^ (Al & Cl) ^ (Bl & Cl);
                Hh = Gh | 0;
                Hl = Gl | 0;
                Gh = Fh | 0;
                Gl = Fl | 0;
                Fh = Eh | 0;
                Fl = El | 0;
                ({ h: Eh, l: El } = add(Dh | 0, Dl | 0, T1h | 0, T1l | 0));
                Dh = Ch | 0;
                Dl = Cl | 0;
                Ch = Bh | 0;
                Cl = Bl | 0;
                Bh = Ah | 0;
                Bl = Al | 0;
                const All = add3L(T1l, sigma0l, MAJl);
                Ah = add3H(All, T1h, sigma0h, MAJh);
                Al = All | 0;
            }
            ({ h: Ah, l: Al } = add(this.Ah | 0, this.Al | 0, Ah | 0, Al | 0));
            ({ h: Bh, l: Bl } = add(this.Bh | 0, this.Bl | 0, Bh | 0, Bl | 0));
            ({ h: Ch, l: Cl } = add(this.Ch | 0, this.Cl | 0, Ch | 0, Cl | 0));
            ({ h: Dh, l: Dl } = add(this.Dh | 0, this.Dl | 0, Dh | 0, Dl | 0));
            ({ h: Eh, l: El } = add(this.Eh | 0, this.El | 0, Eh | 0, El | 0));
            ({ h: Fh, l: Fl } = add(this.Fh | 0, this.Fl | 0, Fh | 0, Fl | 0));
            ({ h: Gh, l: Gl } = add(this.Gh | 0, this.Gl | 0, Gh | 0, Gl | 0));
            ({ h: Hh, l: Hl } = add(this.Hh | 0, this.Hl | 0, Hh | 0, Hl | 0));
            this.set(Ah, Al, Bh, Bl, Ch, Cl, Dh, Dl, Eh, El, Fh, Fl, Gh, Gl, Hh, Hl);
        }
        roundClean() {
            clean(SHA512_W_H, SHA512_W_L);
        }
        destroy() {
            clean(this.buffer);
            this.set(0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0);
        }
    }
    class SHA384 extends SHA512 {
        constructor() {
            super(48);
            this.Ah = SHA384_IV[0] | 0;
            this.Al = SHA384_IV[1] | 0;
            this.Bh = SHA384_IV[2] | 0;
            this.Bl = SHA384_IV[3] | 0;
            this.Ch = SHA384_IV[4] | 0;
            this.Cl = SHA384_IV[5] | 0;
            this.Dh = SHA384_IV[6] | 0;
            this.Dl = SHA384_IV[7] | 0;
            this.Eh = SHA384_IV[8] | 0;
            this.El = SHA384_IV[9] | 0;
            this.Fh = SHA384_IV[10] | 0;
            this.Fl = SHA384_IV[11] | 0;
            this.Gh = SHA384_IV[12] | 0;
            this.Gl = SHA384_IV[13] | 0;
            this.Hh = SHA384_IV[14] | 0;
            this.Hl = SHA384_IV[15] | 0;
        }
    }
    const T224_IV =  Uint32Array.from([
        0x8c3d37c8, 0x19544da2, 0x73e19966, 0x89dcd4d6, 0x1dfab7ae, 0x32ff9c82, 0x679dd514, 0x582f9fcf,
        0x0f6d2b69, 0x7bd44da8, 0x77e36f73, 0x04c48942, 0x3f9d85a8, 0x6a1d36c8, 0x1112e6ad, 0x91d692a1,
    ]);
    const T256_IV =  Uint32Array.from([
        0x22312194, 0xfc2bf72c, 0x9f555fa3, 0xc84c64c2, 0x2393b86b, 0x6f53b151, 0x96387719, 0x5940eabd,
        0x96283ee2, 0xa88effe3, 0xbe5e1e25, 0x53863992, 0x2b0199fc, 0x2c85b8aa, 0x0eb72ddc, 0x81c52ca2,
    ]);
    class SHA512_224 extends SHA512 {
        constructor() {
            super(28);
            this.Ah = T224_IV[0] | 0;
            this.Al = T224_IV[1] | 0;
            this.Bh = T224_IV[2] | 0;
            this.Bl = T224_IV[3] | 0;
            this.Ch = T224_IV[4] | 0;
            this.Cl = T224_IV[5] | 0;
            this.Dh = T224_IV[6] | 0;
            this.Dl = T224_IV[7] | 0;
            this.Eh = T224_IV[8] | 0;
            this.El = T224_IV[9] | 0;
            this.Fh = T224_IV[10] | 0;
            this.Fl = T224_IV[11] | 0;
            this.Gh = T224_IV[12] | 0;
            this.Gl = T224_IV[13] | 0;
            this.Hh = T224_IV[14] | 0;
            this.Hl = T224_IV[15] | 0;
        }
    }
    class SHA512_256 extends SHA512 {
        constructor() {
            super(32);
            this.Ah = T256_IV[0] | 0;
            this.Al = T256_IV[1] | 0;
            this.Bh = T256_IV[2] | 0;
            this.Bl = T256_IV[3] | 0;
            this.Ch = T256_IV[4] | 0;
            this.Cl = T256_IV[5] | 0;
            this.Dh = T256_IV[6] | 0;
            this.Dl = T256_IV[7] | 0;
            this.Eh = T256_IV[8] | 0;
            this.El = T256_IV[9] | 0;
            this.Fh = T256_IV[10] | 0;
            this.Fl = T256_IV[11] | 0;
            this.Gh = T256_IV[12] | 0;
            this.Gl = T256_IV[13] | 0;
            this.Hh = T256_IV[14] | 0;
            this.Hl = T256_IV[15] | 0;
        }
    }
    createHasher$1(() => new SHA256());
    createHasher$1(() => new SHA224());
    const sha512$1 =  createHasher$1(() => new SHA512());
    createHasher$1(() => new SHA384());
    createHasher$1(() => new SHA512_256());
    createHasher$1(() => new SHA512_224());

    /*! noble-curves - MIT License (c) 2022 Paul Miller (paulmillr.com) */
    const _0n$6 = BigInt(0);
    const _1n$5 = BigInt(1);
    function negateCt(condition, item) {
        const neg = item.negate();
        return condition ? neg : item;
    }
    function normalizeZ(c, points) {
        const invertedZs = FpInvertBatch(c.Fp, points.map((p) => p.Z));
        return points.map((p, i) => c.fromAffine(p.toAffine(invertedZs[i])));
    }
    function validateW(W, bits) {
        if (!Number.isSafeInteger(W) || W <= 0 || W > bits)
            throw new Error('invalid window size, expected [1..' + bits + '], got W=' + W);
    }
    function calcWOpts(W, scalarBits) {
        validateW(W, scalarBits);
        const windows = Math.ceil(scalarBits / W) + 1;
        const windowSize = 2 ** (W - 1);
        const maxNumber = 2 ** W;
        const mask = bitMask$1(W);
        const shiftBy = BigInt(W);
        return { windows, windowSize, mask, maxNumber, shiftBy };
    }
    function calcOffsets(n, window, wOpts) {
        const { windowSize, mask, maxNumber, shiftBy } = wOpts;
        let wbits = Number(n & mask);
        let nextN = n >> shiftBy;
        if (wbits > windowSize) {
            wbits -= maxNumber;
            nextN += _1n$5;
        }
        const offsetStart = window * windowSize;
        const offset = offsetStart + Math.abs(wbits) - 1;
        const isZero = wbits === 0;
        const isNeg = wbits < 0;
        const isNegF = window % 2 !== 0;
        const offsetF = offsetStart;
        return { nextN, offset, isZero, isNeg, isNegF, offsetF };
    }
    function validateMSMPoints(points, c) {
        if (!Array.isArray(points))
            throw new Error('array expected');
        points.forEach((p, i) => {
            if (!(p instanceof c))
                throw new Error('invalid point at index ' + i);
        });
    }
    function validateMSMScalars(scalars, field) {
        if (!Array.isArray(scalars))
            throw new Error('array of scalars expected');
        scalars.forEach((s, i) => {
            if (!field.isValid(s))
                throw new Error('invalid scalar at index ' + i);
        });
    }
    const pointPrecomputes = new WeakMap();
    const pointWindowSizes = new WeakMap();
    function getW(P) {
        return pointWindowSizes.get(P) || 1;
    }
    function assert0(n) {
        if (n !== _0n$6)
            throw new Error('invalid wNAF');
    }
    class wNAF {
        constructor(Point, bits) {
            this.BASE = Point.BASE;
            this.ZERO = Point.ZERO;
            this.Fn = Point.Fn;
            this.bits = bits;
        }
        _unsafeLadder(elm, n, p = this.ZERO) {
            let d = elm;
            while (n > _0n$6) {
                if (n & _1n$5)
                    p = p.add(d);
                d = d.double();
                n >>= _1n$5;
            }
            return p;
        }
        precomputeWindow(point, W) {
            const { windows, windowSize } = calcWOpts(W, this.bits);
            const points = [];
            let p = point;
            let base = p;
            for (let window = 0; window < windows; window++) {
                base = p;
                points.push(base);
                for (let i = 1; i < windowSize; i++) {
                    base = base.add(p);
                    points.push(base);
                }
                p = base.double();
            }
            return points;
        }
        wNAF(W, precomputes, n) {
            if (!this.Fn.isValid(n))
                throw new Error('invalid scalar');
            let p = this.ZERO;
            let f = this.BASE;
            const wo = calcWOpts(W, this.bits);
            for (let window = 0; window < wo.windows; window++) {
                const { nextN, offset, isZero, isNeg, isNegF, offsetF } = calcOffsets(n, window, wo);
                n = nextN;
                if (isZero) {
                    f = f.add(negateCt(isNegF, precomputes[offsetF]));
                }
                else {
                    p = p.add(negateCt(isNeg, precomputes[offset]));
                }
            }
            assert0(n);
            return { p, f };
        }
        wNAFUnsafe(W, precomputes, n, acc = this.ZERO) {
            const wo = calcWOpts(W, this.bits);
            for (let window = 0; window < wo.windows; window++) {
                if (n === _0n$6)
                    break;
                const { nextN, offset, isZero, isNeg } = calcOffsets(n, window, wo);
                n = nextN;
                if (isZero) {
                    continue;
                }
                else {
                    const item = precomputes[offset];
                    acc = acc.add(isNeg ? item.negate() : item);
                }
            }
            assert0(n);
            return acc;
        }
        getPrecomputes(W, point, transform) {
            let comp = pointPrecomputes.get(point);
            if (!comp) {
                comp = this.precomputeWindow(point, W);
                if (W !== 1) {
                    if (typeof transform === 'function')
                        comp = transform(comp);
                    pointPrecomputes.set(point, comp);
                }
            }
            return comp;
        }
        cached(point, scalar, transform) {
            const W = getW(point);
            return this.wNAF(W, this.getPrecomputes(W, point, transform), scalar);
        }
        unsafe(point, scalar, transform, prev) {
            const W = getW(point);
            if (W === 1)
                return this._unsafeLadder(point, scalar, prev);
            return this.wNAFUnsafe(W, this.getPrecomputes(W, point, transform), scalar, prev);
        }
        createCache(P, W) {
            validateW(W, this.bits);
            pointWindowSizes.set(P, W);
            pointPrecomputes.delete(P);
        }
        hasCache(elm) {
            return getW(elm) !== 1;
        }
    }
    function pippenger(c, fieldN, points, scalars) {
        validateMSMPoints(points, c);
        validateMSMScalars(scalars, fieldN);
        const plength = points.length;
        const slength = scalars.length;
        if (plength !== slength)
            throw new Error('arrays of points and scalars must have equal length');
        const zero = c.ZERO;
        const wbits = bitLen(BigInt(plength));
        let windowSize = 1;
        if (wbits > 12)
            windowSize = wbits - 3;
        else if (wbits > 4)
            windowSize = wbits - 2;
        else if (wbits > 0)
            windowSize = 2;
        const MASK = bitMask$1(windowSize);
        const buckets = new Array(Number(MASK) + 1).fill(zero);
        const lastBits = Math.floor((fieldN.BITS - 1) / windowSize) * windowSize;
        let sum = zero;
        for (let i = lastBits; i >= 0; i -= windowSize) {
            buckets.fill(zero);
            for (let j = 0; j < slength; j++) {
                const scalar = scalars[j];
                const wbits = Number((scalar >> BigInt(i)) & MASK);
                buckets[wbits] = buckets[wbits].add(points[j]);
            }
            let resI = zero;
            for (let j = buckets.length - 1, sumI = zero; j > 0; j--) {
                sumI = sumI.add(buckets[j]);
                resI = resI.add(sumI);
            }
            sum = sum.add(resI);
            if (i !== 0)
                for (let j = 0; j < windowSize; j++)
                    sum = sum.double();
        }
        return sum;
    }
    function createField(order, field, isLE) {
        if (field) {
            if (field.ORDER !== order)
                throw new Error('Field.ORDER must match order: Fp == p, Fn == n');
            validateField(field);
            return field;
        }
        else {
            return Field(order, { isLE });
        }
    }
    function _createCurveFields(type, CURVE, curveOpts = {}, FpFnLE) {
        if (FpFnLE === undefined)
            FpFnLE = type === 'edwards';
        if (!CURVE || typeof CURVE !== 'object')
            throw new Error(`expected valid ${type} CURVE object`);
        for (const p of ['p', 'n', 'h']) {
            const val = CURVE[p];
            if (!(typeof val === 'bigint' && val > _0n$6))
                throw new Error(`CURVE.${p} must be positive bigint`);
        }
        const Fp = createField(CURVE.p, curveOpts.Fp, FpFnLE);
        const Fn = createField(CURVE.n, curveOpts.Fn, FpFnLE);
        const _b = type === 'weierstrass' ? 'b' : 'd';
        const params = ['Gx', 'Gy', 'a', _b];
        for (const p of params) {
            if (!Fp.isValid(CURVE[p]))
                throw new Error(`CURVE.${p} must be valid field element of CURVE.Fp`);
        }
        CURVE = Object.freeze(Object.assign({}, CURVE));
        return { CURVE, Fp, Fn };
    }

    /*! noble-curves - MIT License (c) 2022 Paul Miller (paulmillr.com) */
    const _0n$5 = BigInt(0), _1n$4 = BigInt(1), _2n$4 = BigInt(2), _8n$1 = BigInt(8);
    function isEdValidXY(Fp, CURVE, x, y) {
        const x2 = Fp.sqr(x);
        const y2 = Fp.sqr(y);
        const left = Fp.add(Fp.mul(CURVE.a, x2), y2);
        const right = Fp.add(Fp.ONE, Fp.mul(CURVE.d, Fp.mul(x2, y2)));
        return Fp.eql(left, right);
    }
    function edwards(params, extraOpts = {}) {
        const validated = _createCurveFields('edwards', params, extraOpts, extraOpts.FpFnLE);
        const { Fp, Fn } = validated;
        let CURVE = validated.CURVE;
        const { h: cofactor } = CURVE;
        _validateObject(extraOpts, {}, { uvRatio: 'function' });
        const MASK = _2n$4 << (BigInt(Fn.BYTES * 8) - _1n$4);
        const modP = (n) => Fp.create(n);
        const uvRatio = extraOpts.uvRatio ||
            ((u, v) => {
                try {
                    return { isValid: true, value: Fp.sqrt(Fp.div(u, v)) };
                }
                catch (e) {
                    return { isValid: false, value: _0n$5 };
                }
            });
        if (!isEdValidXY(Fp, CURVE, CURVE.Gx, CURVE.Gy))
            throw new Error('bad curve params: generator point');
        function acoord(title, n, banZero = false) {
            const min = banZero ? _1n$4 : _0n$5;
            aInRange$1('coordinate ' + title, n, min, MASK);
            return n;
        }
        function aextpoint(other) {
            if (!(other instanceof Point))
                throw new Error('ExtendedPoint expected');
        }
        const toAffineMemo = memoized((p, iz) => {
            const { X, Y, Z } = p;
            const is0 = p.is0();
            if (iz == null)
                iz = is0 ? _8n$1 : Fp.inv(Z);
            const x = modP(X * iz);
            const y = modP(Y * iz);
            const zz = Fp.mul(Z, iz);
            if (is0)
                return { x: _0n$5, y: _1n$4 };
            if (zz !== _1n$4)
                throw new Error('invZ was invalid');
            return { x, y };
        });
        const assertValidMemo = memoized((p) => {
            const { a, d } = CURVE;
            if (p.is0())
                throw new Error('bad point: ZERO');
            const { X, Y, Z, T } = p;
            const X2 = modP(X * X);
            const Y2 = modP(Y * Y);
            const Z2 = modP(Z * Z);
            const Z4 = modP(Z2 * Z2);
            const aX2 = modP(X2 * a);
            const left = modP(Z2 * modP(aX2 + Y2));
            const right = modP(Z4 + modP(d * modP(X2 * Y2)));
            if (left !== right)
                throw new Error('bad point: equation left != right (1)');
            const XY = modP(X * Y);
            const ZT = modP(Z * T);
            if (XY !== ZT)
                throw new Error('bad point: equation left != right (2)');
            return true;
        });
        class Point {
            constructor(X, Y, Z, T) {
                this.X = acoord('x', X);
                this.Y = acoord('y', Y);
                this.Z = acoord('z', Z, true);
                this.T = acoord('t', T);
                Object.freeze(this);
            }
            static CURVE() {
                return CURVE;
            }
            static fromAffine(p) {
                if (p instanceof Point)
                    throw new Error('extended point not allowed');
                const { x, y } = p || {};
                acoord('x', x);
                acoord('y', y);
                return new Point(x, y, _1n$4, modP(x * y));
            }
            static fromBytes(bytes, zip215 = false) {
                const len = Fp.BYTES;
                const { a, d } = CURVE;
                bytes = copyBytes(_abytes2(bytes, len, 'point'));
                _abool2(zip215, 'zip215');
                const normed = copyBytes(bytes);
                const lastByte = bytes[len - 1];
                normed[len - 1] = lastByte & ~0x80;
                const y = bytesToNumberLE$1(normed);
                const max = zip215 ? MASK : Fp.ORDER;
                aInRange$1('point.y', y, _0n$5, max);
                const y2 = modP(y * y);
                const u = modP(y2 - _1n$4);
                const v = modP(d * y2 - a);
                let { isValid, value: x } = uvRatio(u, v);
                if (!isValid)
                    throw new Error('bad point: invalid y coordinate');
                const isXOdd = (x & _1n$4) === _1n$4;
                const isLastByteOdd = (lastByte & 0x80) !== 0;
                if (!zip215 && x === _0n$5 && isLastByteOdd)
                    throw new Error('bad point: x=0 and x_0=1');
                if (isLastByteOdd !== isXOdd)
                    x = modP(-x);
                return Point.fromAffine({ x, y });
            }
            static fromHex(bytes, zip215 = false) {
                return Point.fromBytes(ensureBytes('point', bytes), zip215);
            }
            get x() {
                return this.toAffine().x;
            }
            get y() {
                return this.toAffine().y;
            }
            precompute(windowSize = 8, isLazy = true) {
                wnaf.createCache(this, windowSize);
                if (!isLazy)
                    this.multiply(_2n$4);
                return this;
            }
            assertValidity() {
                assertValidMemo(this);
            }
            equals(other) {
                aextpoint(other);
                const { X: X1, Y: Y1, Z: Z1 } = this;
                const { X: X2, Y: Y2, Z: Z2 } = other;
                const X1Z2 = modP(X1 * Z2);
                const X2Z1 = modP(X2 * Z1);
                const Y1Z2 = modP(Y1 * Z2);
                const Y2Z1 = modP(Y2 * Z1);
                return X1Z2 === X2Z1 && Y1Z2 === Y2Z1;
            }
            is0() {
                return this.equals(Point.ZERO);
            }
            negate() {
                return new Point(modP(-this.X), this.Y, this.Z, modP(-this.T));
            }
            double() {
                const { a } = CURVE;
                const { X: X1, Y: Y1, Z: Z1 } = this;
                const A = modP(X1 * X1);
                const B = modP(Y1 * Y1);
                const C = modP(_2n$4 * modP(Z1 * Z1));
                const D = modP(a * A);
                const x1y1 = X1 + Y1;
                const E = modP(modP(x1y1 * x1y1) - A - B);
                const G = D + B;
                const F = G - C;
                const H = D - B;
                const X3 = modP(E * F);
                const Y3 = modP(G * H);
                const T3 = modP(E * H);
                const Z3 = modP(F * G);
                return new Point(X3, Y3, Z3, T3);
            }
            add(other) {
                aextpoint(other);
                const { a, d } = CURVE;
                const { X: X1, Y: Y1, Z: Z1, T: T1 } = this;
                const { X: X2, Y: Y2, Z: Z2, T: T2 } = other;
                const A = modP(X1 * X2);
                const B = modP(Y1 * Y2);
                const C = modP(T1 * d * T2);
                const D = modP(Z1 * Z2);
                const E = modP((X1 + Y1) * (X2 + Y2) - A - B);
                const F = D - C;
                const G = D + C;
                const H = modP(B - a * A);
                const X3 = modP(E * F);
                const Y3 = modP(G * H);
                const T3 = modP(E * H);
                const Z3 = modP(F * G);
                return new Point(X3, Y3, Z3, T3);
            }
            subtract(other) {
                return this.add(other.negate());
            }
            multiply(scalar) {
                if (!Fn.isValidNot0(scalar))
                    throw new Error('invalid scalar: expected 1 <= sc < curve.n');
                const { p, f } = wnaf.cached(this, scalar, (p) => normalizeZ(Point, p));
                return normalizeZ(Point, [p, f])[0];
            }
            multiplyUnsafe(scalar, acc = Point.ZERO) {
                if (!Fn.isValid(scalar))
                    throw new Error('invalid scalar: expected 0 <= sc < curve.n');
                if (scalar === _0n$5)
                    return Point.ZERO;
                if (this.is0() || scalar === _1n$4)
                    return this;
                return wnaf.unsafe(this, scalar, (p) => normalizeZ(Point, p), acc);
            }
            isSmallOrder() {
                return this.multiplyUnsafe(cofactor).is0();
            }
            isTorsionFree() {
                return wnaf.unsafe(this, CURVE.n).is0();
            }
            toAffine(invertedZ) {
                return toAffineMemo(this, invertedZ);
            }
            clearCofactor() {
                if (cofactor === _1n$4)
                    return this;
                return this.multiplyUnsafe(cofactor);
            }
            toBytes() {
                const { x, y } = this.toAffine();
                const bytes = Fp.toBytes(y);
                bytes[bytes.length - 1] |= x & _1n$4 ? 0x80 : 0;
                return bytes;
            }
            toHex() {
                return bytesToHex(this.toBytes());
            }
            toString() {
                return `<Point ${this.is0() ? 'ZERO' : this.toHex()}>`;
            }
            get ex() {
                return this.X;
            }
            get ey() {
                return this.Y;
            }
            get ez() {
                return this.Z;
            }
            get et() {
                return this.T;
            }
            static normalizeZ(points) {
                return normalizeZ(Point, points);
            }
            static msm(points, scalars) {
                return pippenger(Point, Fn, points, scalars);
            }
            _setWindowSize(windowSize) {
                this.precompute(windowSize);
            }
            toRawBytes() {
                return this.toBytes();
            }
        }
        Point.BASE = new Point(CURVE.Gx, CURVE.Gy, _1n$4, modP(CURVE.Gx * CURVE.Gy));
        Point.ZERO = new Point(_0n$5, _1n$4, _1n$4, _0n$5);
        Point.Fp = Fp;
        Point.Fn = Fn;
        const wnaf = new wNAF(Point, Fn.BITS);
        Point.BASE.precompute(8);
        return Point;
    }
    class PrimeEdwardsPoint {
        constructor(ep) {
            this.ep = ep;
        }
        static fromBytes(_bytes) {
            notImplemented();
        }
        static fromHex(_hex) {
            notImplemented();
        }
        get x() {
            return this.toAffine().x;
        }
        get y() {
            return this.toAffine().y;
        }
        clearCofactor() {
            return this;
        }
        assertValidity() {
            this.ep.assertValidity();
        }
        toAffine(invertedZ) {
            return this.ep.toAffine(invertedZ);
        }
        toHex() {
            return bytesToHex(this.toBytes());
        }
        toString() {
            return this.toHex();
        }
        isTorsionFree() {
            return true;
        }
        isSmallOrder() {
            return false;
        }
        add(other) {
            this.assertSame(other);
            return this.init(this.ep.add(other.ep));
        }
        subtract(other) {
            this.assertSame(other);
            return this.init(this.ep.subtract(other.ep));
        }
        multiply(scalar) {
            return this.init(this.ep.multiply(scalar));
        }
        multiplyUnsafe(scalar) {
            return this.init(this.ep.multiplyUnsafe(scalar));
        }
        double() {
            return this.init(this.ep.double());
        }
        negate() {
            return this.init(this.ep.negate());
        }
        precompute(windowSize, isLazy) {
            return this.init(this.ep.precompute(windowSize, isLazy));
        }
        toRawBytes() {
            return this.toBytes();
        }
    }
    function eddsa(Point, cHash, eddsaOpts = {}) {
        if (typeof cHash !== 'function')
            throw new Error('"hash" function param is required');
        _validateObject(eddsaOpts, {}, {
            adjustScalarBytes: 'function',
            randomBytes: 'function',
            domain: 'function',
            prehash: 'function',
            mapToCurve: 'function',
        });
        const { prehash } = eddsaOpts;
        const { BASE, Fp, Fn } = Point;
        const randomBytes$1 = eddsaOpts.randomBytes || randomBytes;
        const adjustScalarBytes = eddsaOpts.adjustScalarBytes || ((bytes) => bytes);
        const domain = eddsaOpts.domain ||
            ((data, ctx, phflag) => {
                _abool2(phflag, 'phflag');
                if (ctx.length || phflag)
                    throw new Error('Contexts/pre-hash are not supported');
                return data;
            });
        function modN_LE(hash) {
            return Fn.create(bytesToNumberLE$1(hash));
        }
        function getPrivateScalar(key) {
            const len = lengths.secretKey;
            key = ensureBytes('private key', key, len);
            const hashed = ensureBytes('hashed private key', cHash(key), 2 * len);
            const head = adjustScalarBytes(hashed.slice(0, len));
            const prefix = hashed.slice(len, 2 * len);
            const scalar = modN_LE(head);
            return { head, prefix, scalar };
        }
        function getExtendedPublicKey(secretKey) {
            const { head, prefix, scalar } = getPrivateScalar(secretKey);
            const point = BASE.multiply(scalar);
            const pointBytes = point.toBytes();
            return { head, prefix, scalar, point, pointBytes };
        }
        function getPublicKey(secretKey) {
            return getExtendedPublicKey(secretKey).pointBytes;
        }
        function hashDomainToScalar(context = Uint8Array.of(), ...msgs) {
            const msg = concatBytes(...msgs);
            return modN_LE(cHash(domain(msg, ensureBytes('context', context), !!prehash)));
        }
        function sign(msg, secretKey, options = {}) {
            msg = ensureBytes('message', msg);
            if (prehash)
                msg = prehash(msg);
            const { prefix, scalar, pointBytes } = getExtendedPublicKey(secretKey);
            const r = hashDomainToScalar(options.context, prefix, msg);
            const R = BASE.multiply(r).toBytes();
            const k = hashDomainToScalar(options.context, R, pointBytes, msg);
            const s = Fn.create(r + k * scalar);
            if (!Fn.isValid(s))
                throw new Error('sign failed: invalid s');
            const rs = concatBytes(R, Fn.toBytes(s));
            return _abytes2(rs, lengths.signature, 'result');
        }
        const verifyOpts = { zip215: true };
        function verify(sig, msg, publicKey, options = verifyOpts) {
            const { context, zip215 } = options;
            const len = lengths.signature;
            sig = ensureBytes('signature', sig, len);
            msg = ensureBytes('message', msg);
            publicKey = ensureBytes('publicKey', publicKey, lengths.publicKey);
            if (zip215 !== undefined)
                _abool2(zip215, 'zip215');
            if (prehash)
                msg = prehash(msg);
            const mid = len / 2;
            const r = sig.subarray(0, mid);
            const s = bytesToNumberLE$1(sig.subarray(mid, len));
            let A, R, SB;
            try {
                A = Point.fromBytes(publicKey, zip215);
                R = Point.fromBytes(r, zip215);
                SB = BASE.multiplyUnsafe(s);
            }
            catch (error) {
                return false;
            }
            if (!zip215 && A.isSmallOrder())
                return false;
            const k = hashDomainToScalar(context, R.toBytes(), A.toBytes(), msg);
            const RkA = R.add(A.multiplyUnsafe(k));
            return RkA.subtract(SB).clearCofactor().is0();
        }
        const _size = Fp.BYTES;
        const lengths = {
            secretKey: _size,
            publicKey: _size,
            signature: 2 * _size,
            seed: _size,
        };
        function randomSecretKey(seed = randomBytes$1(lengths.seed)) {
            return _abytes2(seed, lengths.seed, 'seed');
        }
        function keygen(seed) {
            const secretKey = utils.randomSecretKey(seed);
            return { secretKey, publicKey: getPublicKey(secretKey) };
        }
        function isValidSecretKey(key) {
            return isBytes$1(key) && key.length === Fn.BYTES;
        }
        function isValidPublicKey(key, zip215) {
            try {
                return !!Point.fromBytes(key, zip215);
            }
            catch (error) {
                return false;
            }
        }
        const utils = {
            getExtendedPublicKey,
            randomSecretKey,
            isValidSecretKey,
            isValidPublicKey,
            toMontgomery(publicKey) {
                const { y } = Point.fromBytes(publicKey);
                const size = lengths.publicKey;
                const is25519 = size === 32;
                if (!is25519 && size !== 57)
                    throw new Error('only defined for 25519 and 448');
                const u = is25519 ? Fp.div(_1n$4 + y, _1n$4 - y) : Fp.div(y - _1n$4, y + _1n$4);
                return Fp.toBytes(u);
            },
            toMontgomerySecret(secretKey) {
                const size = lengths.secretKey;
                _abytes2(secretKey, size);
                const hashed = cHash(secretKey.subarray(0, size));
                return adjustScalarBytes(hashed).subarray(0, size);
            },
            randomPrivateKey: randomSecretKey,
            precompute(windowSize = 8, point = Point.BASE) {
                return point.precompute(windowSize, false);
            },
        };
        return Object.freeze({
            keygen,
            getPublicKey,
            sign,
            verify,
            utils,
            Point,
            lengths,
        });
    }
    function _eddsa_legacy_opts_to_new(c) {
        const CURVE = {
            a: c.a,
            d: c.d,
            p: c.Fp.ORDER,
            n: c.n,
            h: c.h,
            Gx: c.Gx,
            Gy: c.Gy,
        };
        const Fp = c.Fp;
        const Fn = Field(CURVE.n, c.nBitLength, true);
        const curveOpts = { Fp, Fn, uvRatio: c.uvRatio };
        const eddsaOpts = {
            randomBytes: c.randomBytes,
            adjustScalarBytes: c.adjustScalarBytes,
            domain: c.domain,
            prehash: c.prehash,
            mapToCurve: c.mapToCurve,
        };
        return { CURVE, curveOpts, hash: c.hash, eddsaOpts };
    }
    function _eddsa_new_output_to_legacy(c, eddsa) {
        const Point = eddsa.Point;
        const legacy = Object.assign({}, eddsa, {
            ExtendedPoint: Point,
            CURVE: c,
            nBitLength: Point.Fn.BITS,
            nByteLength: Point.Fn.BYTES,
        });
        return legacy;
    }
    function twistedEdwards(c) {
        const { CURVE, curveOpts, hash, eddsaOpts } = _eddsa_legacy_opts_to_new(c);
        const Point = edwards(CURVE, curveOpts);
        const EDDSA = eddsa(Point, hash, eddsaOpts);
        return _eddsa_new_output_to_legacy(c, EDDSA);
    }

    const os2ip = bytesToNumberBE;
    function i2osp(value, length) {
        anum(value);
        anum(length);
        if (value < 0 || value >= 1 << (8 * length))
            throw new Error('invalid I2OSP input: ' + value);
        const res = Array.from({ length }).fill(0);
        for (let i = length - 1; i >= 0; i--) {
            res[i] = value & 0xff;
            value >>>= 8;
        }
        return new Uint8Array(res);
    }
    function strxor(a, b) {
        const arr = new Uint8Array(a.length);
        for (let i = 0; i < a.length; i++) {
            arr[i] = a[i] ^ b[i];
        }
        return arr;
    }
    function anum(item) {
        if (!Number.isSafeInteger(item))
            throw new Error('number expected');
    }
    function normDST(DST) {
        if (!isBytes$1(DST) && typeof DST !== 'string')
            throw new Error('DST must be Uint8Array or string');
        return typeof DST === 'string' ? utf8ToBytes(DST) : DST;
    }
    function expand_message_xmd(msg, DST, lenInBytes, H) {
        abytes$1(msg);
        anum(lenInBytes);
        DST = normDST(DST);
        if (DST.length > 255)
            DST = H(concatBytes(utf8ToBytes('H2C-OVERSIZE-DST-'), DST));
        const { outputLen: b_in_bytes, blockLen: r_in_bytes } = H;
        const ell = Math.ceil(lenInBytes / b_in_bytes);
        if (lenInBytes > 65535 || ell > 255)
            throw new Error('expand_message_xmd: invalid lenInBytes');
        const DST_prime = concatBytes(DST, i2osp(DST.length, 1));
        const Z_pad = i2osp(0, r_in_bytes);
        const l_i_b_str = i2osp(lenInBytes, 2);
        const b = new Array(ell);
        const b_0 = H(concatBytes(Z_pad, msg, l_i_b_str, i2osp(0, 1), DST_prime));
        b[0] = H(concatBytes(b_0, i2osp(1, 1), DST_prime));
        for (let i = 1; i <= ell; i++) {
            const args = [strxor(b_0, b[i - 1]), i2osp(i + 1, 1), DST_prime];
            b[i] = H(concatBytes(...args));
        }
        const pseudo_random_bytes = concatBytes(...b);
        return pseudo_random_bytes.slice(0, lenInBytes);
    }
    function expand_message_xof(msg, DST, lenInBytes, k, H) {
        abytes$1(msg);
        anum(lenInBytes);
        DST = normDST(DST);
        if (DST.length > 255) {
            const dkLen = Math.ceil((2 * k) / 8);
            DST = H.create({ dkLen }).update(utf8ToBytes('H2C-OVERSIZE-DST-')).update(DST).digest();
        }
        if (lenInBytes > 65535 || DST.length > 255)
            throw new Error('expand_message_xof: invalid lenInBytes');
        return (H.create({ dkLen: lenInBytes })
            .update(msg)
            .update(i2osp(lenInBytes, 2))
            .update(DST)
            .update(i2osp(DST.length, 1))
            .digest());
    }
    function hash_to_field(msg, count, options) {
        _validateObject(options, {
            p: 'bigint',
            m: 'number',
            k: 'number',
            hash: 'function',
        });
        const { p, k, m, hash, expand, DST } = options;
        if (!isHash(options.hash))
            throw new Error('expected valid hash');
        abytes$1(msg);
        anum(count);
        const log2p = p.toString(2).length;
        const L = Math.ceil((log2p + k) / 8);
        const len_in_bytes = count * m * L;
        let prb;
        if (expand === 'xmd') {
            prb = expand_message_xmd(msg, DST, len_in_bytes, hash);
        }
        else if (expand === 'xof') {
            prb = expand_message_xof(msg, DST, len_in_bytes, k, hash);
        }
        else if (expand === '_internal_pass') {
            prb = msg;
        }
        else {
            throw new Error('expand must be "xmd" or "xof"');
        }
        const u = new Array(count);
        for (let i = 0; i < count; i++) {
            const e = new Array(m);
            for (let j = 0; j < m; j++) {
                const elm_offset = L * (j + i * m);
                const tv = prb.subarray(elm_offset, elm_offset + L);
                e[j] = mod(os2ip(tv), p);
            }
            u[i] = e;
        }
        return u;
    }
    const _DST_scalar = utf8ToBytes('HashToScalar-');
    function createHasher(Point, mapToCurve, defaults) {
        if (typeof mapToCurve !== 'function')
            throw new Error('mapToCurve() must be defined');
        function map(num) {
            return Point.fromAffine(mapToCurve(num));
        }
        function clear(initial) {
            const P = initial.clearCofactor();
            if (P.equals(Point.ZERO))
                return Point.ZERO;
            P.assertValidity();
            return P;
        }
        return {
            defaults,
            hashToCurve(msg, options) {
                const opts = Object.assign({}, defaults, options);
                const u = hash_to_field(msg, 2, opts);
                const u0 = map(u[0]);
                const u1 = map(u[1]);
                return clear(u0.add(u1));
            },
            encodeToCurve(msg, options) {
                const optsDst = defaults.encodeDST ? { DST: defaults.encodeDST } : {};
                const opts = Object.assign({}, defaults, optsDst, options);
                const u = hash_to_field(msg, 1, opts);
                const u0 = map(u[0]);
                return clear(u0);
            },
            mapToCurve(scalars) {
                if (!Array.isArray(scalars))
                    throw new Error('expected array of bigints');
                for (const i of scalars)
                    if (typeof i !== 'bigint')
                        throw new Error('expected array of bigints');
                return clear(map(scalars));
            },
            hashToScalar(msg, options) {
                const N = Point.Fn.ORDER;
                const opts = Object.assign({}, defaults, { p: N, m: 1, DST: _DST_scalar }, options);
                return hash_to_field(msg, 1, opts)[0][0];
            },
        };
    }

    /*! noble-curves - MIT License (c) 2022 Paul Miller (paulmillr.com) */
    const _0n$4 = BigInt(0);
    const _1n$3 = BigInt(1);
    const _2n$3 = BigInt(2);
    function validateOpts(curve) {
        _validateObject(curve, {
            adjustScalarBytes: 'function',
            powPminus2: 'function',
        });
        return Object.freeze({ ...curve });
    }
    function montgomery(curveDef) {
        const CURVE = validateOpts(curveDef);
        const { P, type, adjustScalarBytes, powPminus2, randomBytes: rand } = CURVE;
        const is25519 = type === 'x25519';
        if (!is25519 && type !== 'x448')
            throw new Error('invalid type');
        const randomBytes_ = rand || randomBytes;
        const montgomeryBits = is25519 ? 255 : 448;
        const fieldLen = is25519 ? 32 : 56;
        const Gu = is25519 ? BigInt(9) : BigInt(5);
        const a24 = is25519 ? BigInt(121665) : BigInt(39081);
        const minScalar = is25519 ? _2n$3 ** BigInt(254) : _2n$3 ** BigInt(447);
        const maxAdded = is25519
            ? BigInt(8) * _2n$3 ** BigInt(251) - _1n$3
            : BigInt(4) * _2n$3 ** BigInt(445) - _1n$3;
        const maxScalar = minScalar + maxAdded + _1n$3;
        const modP = (n) => mod(n, P);
        const GuBytes = encodeU(Gu);
        function encodeU(u) {
            return numberToBytesLE$1(modP(u), fieldLen);
        }
        function decodeU(u) {
            const _u = ensureBytes('u coordinate', u, fieldLen);
            if (is25519)
                _u[31] &= 127;
            return modP(bytesToNumberLE$1(_u));
        }
        function decodeScalar(scalar) {
            return bytesToNumberLE$1(adjustScalarBytes(ensureBytes('scalar', scalar, fieldLen)));
        }
        function scalarMult(scalar, u) {
            const pu = montgomeryLadder(decodeU(u), decodeScalar(scalar));
            if (pu === _0n$4)
                throw new Error('invalid private or public key received');
            return encodeU(pu);
        }
        function scalarMultBase(scalar) {
            return scalarMult(scalar, GuBytes);
        }
        function cswap(swap, x_2, x_3) {
            const dummy = modP(swap * (x_2 - x_3));
            x_2 = modP(x_2 - dummy);
            x_3 = modP(x_3 + dummy);
            return { x_2, x_3 };
        }
        function montgomeryLadder(u, scalar) {
            aInRange$1('u', u, _0n$4, P);
            aInRange$1('scalar', scalar, minScalar, maxScalar);
            const k = scalar;
            const x_1 = u;
            let x_2 = _1n$3;
            let z_2 = _0n$4;
            let x_3 = u;
            let z_3 = _1n$3;
            let swap = _0n$4;
            for (let t = BigInt(montgomeryBits - 1); t >= _0n$4; t--) {
                const k_t = (k >> t) & _1n$3;
                swap ^= k_t;
                ({ x_2, x_3 } = cswap(swap, x_2, x_3));
                ({ x_2: z_2, x_3: z_3 } = cswap(swap, z_2, z_3));
                swap = k_t;
                const A = x_2 + z_2;
                const AA = modP(A * A);
                const B = x_2 - z_2;
                const BB = modP(B * B);
                const E = AA - BB;
                const C = x_3 + z_3;
                const D = x_3 - z_3;
                const DA = modP(D * A);
                const CB = modP(C * B);
                const dacb = DA + CB;
                const da_cb = DA - CB;
                x_3 = modP(dacb * dacb);
                z_3 = modP(x_1 * modP(da_cb * da_cb));
                x_2 = modP(AA * BB);
                z_2 = modP(E * (AA + modP(a24 * E)));
            }
            ({ x_2, x_3 } = cswap(swap, x_2, x_3));
            ({ x_2: z_2, x_3: z_3 } = cswap(swap, z_2, z_3));
            const z2 = powPminus2(z_2);
            return modP(x_2 * z2);
        }
        const lengths = {
            secretKey: fieldLen,
            publicKey: fieldLen,
            seed: fieldLen,
        };
        const randomSecretKey = (seed = randomBytes_(fieldLen)) => {
            abytes$1(seed, lengths.seed);
            return seed;
        };
        function keygen(seed) {
            const secretKey = randomSecretKey(seed);
            return { secretKey, publicKey: scalarMultBase(secretKey) };
        }
        const utils = {
            randomSecretKey,
            randomPrivateKey: randomSecretKey,
        };
        return {
            keygen,
            getSharedSecret: (secretKey, publicKey) => scalarMult(secretKey, publicKey),
            getPublicKey: (secretKey) => scalarMultBase(secretKey),
            scalarMult,
            scalarMultBase,
            utils,
            GuBytes: GuBytes.slice(),
            lengths,
        };
    }

    /*! noble-curves - MIT License (c) 2022 Paul Miller (paulmillr.com) */
    const _0n$3 =  BigInt(0), _1n$2 = BigInt(1), _2n$2 = BigInt(2), _3n$1 = BigInt(3);
    const _5n = BigInt(5), _8n = BigInt(8);
    const ed25519_CURVE_p = BigInt('0x7fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffed');
    const ed25519_CURVE =  (() => ({
        p: ed25519_CURVE_p,
        n: BigInt('0x1000000000000000000000000000000014def9dea2f79cd65812631a5cf5d3ed'),
        h: _8n,
        a: BigInt('0x7fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffec'),
        d: BigInt('0x52036cee2b6ffe738cc740797779e89800700a4d4141d8ab75eb4dca135978a3'),
        Gx: BigInt('0x216936d3cd6e53fec0a4e231fdd6dc5c692cc7609525a7b2c9562d608f25d51a'),
        Gy: BigInt('0x6666666666666666666666666666666666666666666666666666666666666658'),
    }))();
    function ed25519_pow_2_252_3(x) {
        const _10n = BigInt(10), _20n = BigInt(20), _40n = BigInt(40), _80n = BigInt(80);
        const P = ed25519_CURVE_p;
        const x2 = (x * x) % P;
        const b2 = (x2 * x) % P;
        const b4 = (pow2(b2, _2n$2, P) * b2) % P;
        const b5 = (pow2(b4, _1n$2, P) * x) % P;
        const b10 = (pow2(b5, _5n, P) * b5) % P;
        const b20 = (pow2(b10, _10n, P) * b10) % P;
        const b40 = (pow2(b20, _20n, P) * b20) % P;
        const b80 = (pow2(b40, _40n, P) * b40) % P;
        const b160 = (pow2(b80, _80n, P) * b80) % P;
        const b240 = (pow2(b160, _80n, P) * b80) % P;
        const b250 = (pow2(b240, _10n, P) * b10) % P;
        const pow_p_5_8 = (pow2(b250, _2n$2, P) * x) % P;
        return { pow_p_5_8, b2 };
    }
    function adjustScalarBytes(bytes) {
        bytes[0] &= 248;
        bytes[31] &= 127;
        bytes[31] |= 64;
        return bytes;
    }
    const ED25519_SQRT_M1 =  BigInt('19681161376707505956807079304988542015446066515923890162744021073123829784752');
    function uvRatio(u, v) {
        const P = ed25519_CURVE_p;
        const v3 = mod(v * v * v, P);
        const v7 = mod(v3 * v3 * v, P);
        const pow = ed25519_pow_2_252_3(u * v7).pow_p_5_8;
        let x = mod(u * v3 * pow, P);
        const vx2 = mod(v * x * x, P);
        const root1 = x;
        const root2 = mod(x * ED25519_SQRT_M1, P);
        const useRoot1 = vx2 === u;
        const useRoot2 = vx2 === mod(-u, P);
        const noRoot = vx2 === mod(-u * ED25519_SQRT_M1, P);
        if (useRoot1)
            x = root1;
        if (useRoot2 || noRoot)
            x = root2;
        if (isNegativeLE(x, P))
            x = mod(-x, P);
        return { isValid: useRoot1 || useRoot2, value: x };
    }
    const Fp =  (() => Field(ed25519_CURVE.p, { isLE: true }))();
    const Fn =  (() => Field(ed25519_CURVE.n, { isLE: true }))();
    const ed25519Defaults =  (() => ({
        ...ed25519_CURVE,
        Fp,
        hash: sha512$1,
        adjustScalarBytes,
        uvRatio,
    }))();
    const ed25519 =  (() => twistedEdwards(ed25519Defaults))();
    function ed25519_domain(data, ctx, phflag) {
        if (ctx.length > 255)
            throw new Error('Context is too big');
        return concatBytes(utf8ToBytes('SigEd25519 no Ed25519 collisions'), new Uint8Array([phflag ? 1 : 0, ctx.length]), ctx, data);
    }
    (() => twistedEdwards({
        ...ed25519Defaults,
        domain: ed25519_domain,
    }))();
    (() => twistedEdwards(Object.assign({}, ed25519Defaults, {
        domain: ed25519_domain,
        prehash: sha512$1,
    })))();
    (() => {
        const P = Fp.ORDER;
        return montgomery({
            P,
            type: 'x25519',
            powPminus2: (x) => {
                const { pow_p_5_8, b2 } = ed25519_pow_2_252_3(x);
                return mod(pow2(pow_p_5_8, _3n$1, P) * b2, P);
            },
            adjustScalarBytes,
        });
    })();
    const ELL2_C1 =  (() => (ed25519_CURVE_p + _3n$1) / _8n)();
    const ELL2_C2 =  (() => Fp.pow(_2n$2, ELL2_C1))();
    const ELL2_C3 =  (() => Fp.sqrt(Fp.neg(Fp.ONE)))();
    function map_to_curve_elligator2_curve25519(u) {
        const ELL2_C4 = (ed25519_CURVE_p - _5n) / _8n;
        const ELL2_J = BigInt(486662);
        let tv1 = Fp.sqr(u);
        tv1 = Fp.mul(tv1, _2n$2);
        let xd = Fp.add(tv1, Fp.ONE);
        let x1n = Fp.neg(ELL2_J);
        let tv2 = Fp.sqr(xd);
        let gxd = Fp.mul(tv2, xd);
        let gx1 = Fp.mul(tv1, ELL2_J);
        gx1 = Fp.mul(gx1, x1n);
        gx1 = Fp.add(gx1, tv2);
        gx1 = Fp.mul(gx1, x1n);
        let tv3 = Fp.sqr(gxd);
        tv2 = Fp.sqr(tv3);
        tv3 = Fp.mul(tv3, gxd);
        tv3 = Fp.mul(tv3, gx1);
        tv2 = Fp.mul(tv2, tv3);
        let y11 = Fp.pow(tv2, ELL2_C4);
        y11 = Fp.mul(y11, tv3);
        let y12 = Fp.mul(y11, ELL2_C3);
        tv2 = Fp.sqr(y11);
        tv2 = Fp.mul(tv2, gxd);
        let e1 = Fp.eql(tv2, gx1);
        let y1 = Fp.cmov(y12, y11, e1);
        let x2n = Fp.mul(x1n, tv1);
        let y21 = Fp.mul(y11, u);
        y21 = Fp.mul(y21, ELL2_C2);
        let y22 = Fp.mul(y21, ELL2_C3);
        let gx2 = Fp.mul(gx1, tv1);
        tv2 = Fp.sqr(y21);
        tv2 = Fp.mul(tv2, gxd);
        let e2 = Fp.eql(tv2, gx2);
        let y2 = Fp.cmov(y22, y21, e2);
        tv2 = Fp.sqr(y1);
        tv2 = Fp.mul(tv2, gxd);
        let e3 = Fp.eql(tv2, gx1);
        let xn = Fp.cmov(x2n, x1n, e3);
        let y = Fp.cmov(y2, y1, e3);
        let e4 = Fp.isOdd(y);
        y = Fp.cmov(y, Fp.neg(y), e3 !== e4);
        return { xMn: xn, xMd: xd, yMn: y, yMd: _1n$2 };
    }
    const ELL2_C1_EDWARDS =  (() => FpSqrtEven(Fp, Fp.neg(BigInt(486664))))();
    function map_to_curve_elligator2_edwards25519(u) {
        const { xMn, xMd, yMn, yMd } = map_to_curve_elligator2_curve25519(u);
        let xn = Fp.mul(xMn, yMd);
        xn = Fp.mul(xn, ELL2_C1_EDWARDS);
        let xd = Fp.mul(xMd, yMn);
        let yn = Fp.sub(xMn, xMd);
        let yd = Fp.add(xMn, xMd);
        let tv1 = Fp.mul(xd, yd);
        let e = Fp.eql(tv1, Fp.ZERO);
        xn = Fp.cmov(xn, Fp.ZERO, e);
        xd = Fp.cmov(xd, Fp.ONE, e);
        yn = Fp.cmov(yn, Fp.ONE, e);
        yd = Fp.cmov(yd, Fp.ONE, e);
        const [xd_inv, yd_inv] = FpInvertBatch(Fp, [xd, yd], true);
        return { x: Fp.mul(xn, xd_inv), y: Fp.mul(yn, yd_inv) };
    }
    (() => createHasher(ed25519.Point, (scalars) => map_to_curve_elligator2_edwards25519(scalars[0]), {
        DST: 'edwards25519_XMD:SHA-512_ELL2_RO_',
        encodeDST: 'edwards25519_XMD:SHA-512_ELL2_NU_',
        p: ed25519_CURVE_p,
        m: 1,
        k: 128,
        expand: 'xmd',
        hash: sha512$1,
    }))();
    const SQRT_M1 = ED25519_SQRT_M1;
    const SQRT_AD_MINUS_ONE =  BigInt('25063068953384623474111414158702152701244531502492656460079210482610430750235');
    const INVSQRT_A_MINUS_D =  BigInt('54469307008909316920995813868745141605393597292927456921205312896311721017578');
    const ONE_MINUS_D_SQ =  BigInt('1159843021668779879193775521855586647937357759715417654439879720876111806838');
    const D_MINUS_ONE_SQ =  BigInt('40440834346308536858101042469323190826248399146238708352240133220865137265952');
    const invertSqrt = (number) => uvRatio(_1n$2, number);
    const MAX_255B =  BigInt('0x7fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff');
    const bytes255ToNumberLE = (bytes) => ed25519.Point.Fp.create(bytesToNumberLE$1(bytes) & MAX_255B);
    function calcElligatorRistrettoMap(r0) {
        const { d } = ed25519_CURVE;
        const P = ed25519_CURVE_p;
        const mod = (n) => Fp.create(n);
        const r = mod(SQRT_M1 * r0 * r0);
        const Ns = mod((r + _1n$2) * ONE_MINUS_D_SQ);
        let c = BigInt(-1);
        const D = mod((c - d * r) * mod(r + d));
        let { isValid: Ns_D_is_sq, value: s } = uvRatio(Ns, D);
        let s_ = mod(s * r0);
        if (!isNegativeLE(s_, P))
            s_ = mod(-s_);
        if (!Ns_D_is_sq)
            s = s_;
        if (!Ns_D_is_sq)
            c = r;
        const Nt = mod(c * (r - _1n$2) * D_MINUS_ONE_SQ - D);
        const s2 = s * s;
        const W0 = mod((s + s) * D);
        const W1 = mod(Nt * SQRT_AD_MINUS_ONE);
        const W2 = mod(_1n$2 - s2);
        const W3 = mod(_1n$2 + s2);
        return new ed25519.Point(mod(W0 * W3), mod(W2 * W1), mod(W1 * W3), mod(W0 * W2));
    }
    function ristretto255_map(bytes) {
        abytes$1(bytes, 64);
        const r1 = bytes255ToNumberLE(bytes.subarray(0, 32));
        const R1 = calcElligatorRistrettoMap(r1);
        const r2 = bytes255ToNumberLE(bytes.subarray(32, 64));
        const R2 = calcElligatorRistrettoMap(r2);
        return new _RistrettoPoint(R1.add(R2));
    }
    class _RistrettoPoint extends PrimeEdwardsPoint {
        constructor(ep) {
            super(ep);
        }
        static fromAffine(ap) {
            return new _RistrettoPoint(ed25519.Point.fromAffine(ap));
        }
        assertSame(other) {
            if (!(other instanceof _RistrettoPoint))
                throw new Error('RistrettoPoint expected');
        }
        init(ep) {
            return new _RistrettoPoint(ep);
        }
        static hashToCurve(hex) {
            return ristretto255_map(ensureBytes('ristrettoHash', hex, 64));
        }
        static fromBytes(bytes) {
            abytes$1(bytes, 32);
            const { a, d } = ed25519_CURVE;
            const P = ed25519_CURVE_p;
            const mod = (n) => Fp.create(n);
            const s = bytes255ToNumberLE(bytes);
            if (!equalBytes(Fp.toBytes(s), bytes) || isNegativeLE(s, P))
                throw new Error('invalid ristretto255 encoding 1');
            const s2 = mod(s * s);
            const u1 = mod(_1n$2 + a * s2);
            const u2 = mod(_1n$2 - a * s2);
            const u1_2 = mod(u1 * u1);
            const u2_2 = mod(u2 * u2);
            const v = mod(a * d * u1_2 - u2_2);
            const { isValid, value: I } = invertSqrt(mod(v * u2_2));
            const Dx = mod(I * u2);
            const Dy = mod(I * Dx * v);
            let x = mod((s + s) * Dx);
            if (isNegativeLE(x, P))
                x = mod(-x);
            const y = mod(u1 * Dy);
            const t = mod(x * y);
            if (!isValid || isNegativeLE(t, P) || y === _0n$3)
                throw new Error('invalid ristretto255 encoding 2');
            return new _RistrettoPoint(new ed25519.Point(x, y, _1n$2, t));
        }
        static fromHex(hex) {
            return _RistrettoPoint.fromBytes(ensureBytes('ristrettoHex', hex, 32));
        }
        static msm(points, scalars) {
            return pippenger(_RistrettoPoint, ed25519.Point.Fn, points, scalars);
        }
        toBytes() {
            let { X, Y, Z, T } = this.ep;
            const P = ed25519_CURVE_p;
            const mod = (n) => Fp.create(n);
            const u1 = mod(mod(Z + Y) * mod(Z - Y));
            const u2 = mod(X * Y);
            const u2sq = mod(u2 * u2);
            const { value: invsqrt } = invertSqrt(mod(u1 * u2sq));
            const D1 = mod(invsqrt * u1);
            const D2 = mod(invsqrt * u2);
            const zInv = mod(D1 * D2 * T);
            let D;
            if (isNegativeLE(T * zInv, P)) {
                let _x = mod(Y * SQRT_M1);
                let _y = mod(X * SQRT_M1);
                X = _x;
                Y = _y;
                D = mod(D1 * INVSQRT_A_MINUS_D);
            }
            else {
                D = D2;
            }
            if (isNegativeLE(X * zInv, P))
                Y = mod(-Y);
            let s = mod((Z - Y) * D);
            if (isNegativeLE(s, P))
                s = mod(-s);
            return Fp.toBytes(s);
        }
        equals(other) {
            this.assertSame(other);
            const { X: X1, Y: Y1 } = this.ep;
            const { X: X2, Y: Y2 } = other.ep;
            const mod = (n) => Fp.create(n);
            const one = mod(X1 * Y2) === mod(Y1 * X2);
            const two = mod(Y1 * Y2) === mod(X1 * X2);
            return one || two;
        }
        is0() {
            return this.equals(_RistrettoPoint.ZERO);
        }
    }
    _RistrettoPoint.BASE =
     (() => new _RistrettoPoint(ed25519.Point.BASE))();
    _RistrettoPoint.ZERO =
     (() => new _RistrettoPoint(ed25519.Point.ZERO))();
    _RistrettoPoint.Fp =
     (() => Fp)();
    _RistrettoPoint.Fn =
     (() => Fn)();
    const RistrettoPoint = _RistrettoPoint;

    const _0n$2 = BigInt(0);
    const _1n$1 = BigInt(1);
    const _2n$1 = BigInt(2);
    const _7n$2 = BigInt(7);
    const _256n$2 = BigInt(256);
    const _0x71n$1 = BigInt(0x71);
    const SHA3_PI$1 = [];
    const SHA3_ROTL$1 = [];
    const _SHA3_IOTA$1 = [];
    for (let round = 0, R = _1n$1, x = 1, y = 0; round < 24; round++) {
        [x, y] = [y, (2 * x + 3 * y) % 5];
        SHA3_PI$1.push(2 * (5 * y + x));
        SHA3_ROTL$1.push((((round + 1) * (round + 2)) / 2) % 64);
        let t = _0n$2;
        for (let j = 0; j < 7; j++) {
            R = ((R << _1n$1) ^ ((R >> _7n$2) * _0x71n$1)) % _256n$2;
            if (R & _2n$1)
                t ^= _1n$1 << ((_1n$1 <<  BigInt(j)) - _1n$1);
        }
        _SHA3_IOTA$1.push(t);
    }
    const IOTAS = split(_SHA3_IOTA$1, true);
    const SHA3_IOTA_H$1 = IOTAS[0];
    const SHA3_IOTA_L$1 = IOTAS[1];
    const rotlH$1 = (h, l, s) => (s > 32 ? rotlBH(h, l, s) : rotlSH(h, l, s));
    const rotlL$1 = (h, l, s) => (s > 32 ? rotlBL(h, l, s) : rotlSL(h, l, s));
    function keccakP$1(s, rounds = 24) {
        const B = new Uint32Array(5 * 2);
        for (let round = 24 - rounds; round < 24; round++) {
            for (let x = 0; x < 10; x++)
                B[x] = s[x] ^ s[x + 10] ^ s[x + 20] ^ s[x + 30] ^ s[x + 40];
            for (let x = 0; x < 10; x += 2) {
                const idx1 = (x + 8) % 10;
                const idx0 = (x + 2) % 10;
                const B0 = B[idx0];
                const B1 = B[idx0 + 1];
                const Th = rotlH$1(B0, B1, 1) ^ B[idx1];
                const Tl = rotlL$1(B0, B1, 1) ^ B[idx1 + 1];
                for (let y = 0; y < 50; y += 10) {
                    s[x + y] ^= Th;
                    s[x + y + 1] ^= Tl;
                }
            }
            let curH = s[2];
            let curL = s[3];
            for (let t = 0; t < 24; t++) {
                const shift = SHA3_ROTL$1[t];
                const Th = rotlH$1(curH, curL, shift);
                const Tl = rotlL$1(curH, curL, shift);
                const PI = SHA3_PI$1[t];
                curH = s[PI];
                curL = s[PI + 1];
                s[PI] = Th;
                s[PI + 1] = Tl;
            }
            for (let y = 0; y < 50; y += 10) {
                for (let x = 0; x < 10; x++)
                    B[x] = s[y + x];
                for (let x = 0; x < 10; x++)
                    s[y + x] ^= ~B[(x + 2) % 10] & B[(x + 4) % 10];
            }
            s[0] ^= SHA3_IOTA_H$1[round];
            s[1] ^= SHA3_IOTA_L$1[round];
        }
        clean(B);
    }
    let Keccak$1 = class Keccak extends Hash {
        constructor(blockLen, suffix, outputLen, enableXOF = false, rounds = 24) {
            super();
            this.pos = 0;
            this.posOut = 0;
            this.finished = false;
            this.destroyed = false;
            this.enableXOF = false;
            this.blockLen = blockLen;
            this.suffix = suffix;
            this.outputLen = outputLen;
            this.enableXOF = enableXOF;
            this.rounds = rounds;
            anumber(outputLen);
            if (!(0 < blockLen && blockLen < 200))
                throw new Error('only keccak-f1600 function is supported');
            this.state = new Uint8Array(200);
            this.state32 = u32(this.state);
        }
        clone() {
            return this._cloneInto();
        }
        keccak() {
            swap32IfBE(this.state32);
            keccakP$1(this.state32, this.rounds);
            swap32IfBE(this.state32);
            this.posOut = 0;
            this.pos = 0;
        }
        update(data) {
            aexists(this);
            data = toBytes(data);
            abytes$1(data);
            const { blockLen, state } = this;
            const len = data.length;
            for (let pos = 0; pos < len;) {
                const take = Math.min(blockLen - this.pos, len - pos);
                for (let i = 0; i < take; i++)
                    state[this.pos++] ^= data[pos++];
                if (this.pos === blockLen)
                    this.keccak();
            }
            return this;
        }
        finish() {
            if (this.finished)
                return;
            this.finished = true;
            const { state, suffix, pos, blockLen } = this;
            state[pos] ^= suffix;
            if ((suffix & 0x80) !== 0 && pos === blockLen - 1)
                this.keccak();
            state[blockLen - 1] ^= 0x80;
            this.keccak();
        }
        writeInto(out) {
            aexists(this, false);
            abytes$1(out);
            this.finish();
            const bufferOut = this.state;
            const { blockLen } = this;
            for (let pos = 0, len = out.length; pos < len;) {
                if (this.posOut >= blockLen)
                    this.keccak();
                const take = Math.min(blockLen - this.posOut, len - pos);
                out.set(bufferOut.subarray(this.posOut, this.posOut + take), pos);
                this.posOut += take;
                pos += take;
            }
            return out;
        }
        xofInto(out) {
            if (!this.enableXOF)
                throw new Error('XOF is not possible for this instance');
            return this.writeInto(out);
        }
        xof(bytes) {
            anumber(bytes);
            return this.xofInto(new Uint8Array(bytes));
        }
        digestInto(out) {
            aoutput(out, this);
            if (this.finished)
                throw new Error('digest() was already called');
            this.writeInto(out);
            this.destroy();
            return out;
        }
        digest() {
            return this.digestInto(new Uint8Array(this.outputLen));
        }
        destroy() {
            this.destroyed = true;
            clean(this.state);
        }
        _cloneInto(to) {
            const { blockLen, suffix, outputLen, rounds, enableXOF } = this;
            to || (to = new Keccak(blockLen, suffix, outputLen, enableXOF, rounds));
            to.state32.set(this.state32);
            to.pos = this.pos;
            to.posOut = this.posOut;
            to.finished = this.finished;
            to.rounds = rounds;
            to.suffix = suffix;
            to.outputLen = outputLen;
            to.enableXOF = enableXOF;
            to.destroyed = this.destroyed;
            return to;
        }
    };
    const gen$1 = (suffix, blockLen, outputLen) => createHasher$1(() => new Keccak$1(blockLen, suffix, outputLen));
    (() => gen$1(0x06, 144, 224 / 8))();
    (() => gen$1(0x06, 136, 256 / 8))();
    (() => gen$1(0x06, 104, 384 / 8))();
    (() => gen$1(0x06, 72, 512 / 8))();
    (() => gen$1(0x01, 144, 224 / 8))();
    (() => gen$1(0x01, 136, 256 / 8))();
    (() => gen$1(0x01, 104, 384 / 8))();
    (() => gen$1(0x01, 72, 512 / 8))();
    const genShake$1 = (suffix, blockLen, outputLen) => createXOFer((opts = {}) => new Keccak$1(blockLen, suffix, opts.dkLen === undefined ? outputLen : opts.dkLen, true));
    (() => genShake$1(0x1f, 168, 128 / 8))();
    (() => genShake$1(0x1f, 136, 256 / 8))();

    const sha512 = sha512$1;

    const _0n$1 = BigInt(0), _3n = BigInt(3);
    function toData(d) {
        if (typeof d === 'string')
            return utf8ToBytes(d);
        if (isBytes(d))
            return d;
        throw new Error('Wrong data');
    }
    function abytes(title, b, ...lengths) {
        if (!isBytes(b))
            throw new Error(`${title}: Uint8Array expected`);
        if (lengths.length && !lengths.includes(b.length))
            throw new Error(`${title}: Uint8Array expected of length ${lengths}, not of length=${b.length}`);
    }
    function checkU32(title, n) {
        if (!Number.isSafeInteger(n) || n < 0 || n > 4294967295)
            throw new Error(`${title}: wrong u32 integer: ${n}`);
        return n;
    }
    function cleanBytes(...list) {
        for (const t of list)
            t.fill(0);
    }
    const EMPTY = Uint8Array.of();
    const CURVE_ORDER = ed25519.CURVE.n;
    function parseScalar(title, bytes) {
        abytes(title, bytes, 32);
        const n = bytesToNumberLE(bytes);
        aInRange(title, n, _0n$1, CURVE_ORDER);
        return n;
    }
    const modN = (n) => mod(n, CURVE_ORDER);
    const STROBE_R = 166;
    const Flags = {
        I: 1,
        A: 1 << 1,
        C: 1 << 2,
        T: 1 << 3,
        M: 1 << 4,
        K: 1 << 5,
    };
    class Strobe128 {
        constructor(protocolLabel) {
            this.state = new Uint8Array(200);
            this.pos = 0;
            this.posBegin = 0;
            this.curFlags = 0;
            this.state.set([1, STROBE_R + 2, 1, 0, 1, 96], 0);
            this.state.set(utf8ToBytes('STROBEv1.0.2'), 6);
            this.state32 = u32(this.state);
            this.keccakF1600();
            this.metaAD(protocolLabel, false);
        }
        keccakF1600() {
            keccakP$1(this.state32);
        }
        runF() {
            this.state[this.pos] ^= this.posBegin;
            this.state[this.pos + 1] ^= 0x04;
            this.state[STROBE_R + 1] ^= 0x80;
            this.keccakF1600();
            this.pos = 0;
            this.posBegin = 0;
        }
        absorb(data) {
            for (let i = 0; i < data.length; i++) {
                this.state[this.pos++] ^= data[i];
                if (this.pos === STROBE_R)
                    this.runF();
            }
        }
        squeeze(len) {
            const data = new Uint8Array(len);
            for (let i = 0; i < data.length; i++) {
                data[i] = this.state[this.pos];
                this.state[this.pos++] = 0;
                if (this.pos === STROBE_R)
                    this.runF();
            }
            return data;
        }
        overwrite(data) {
            for (let i = 0; i < data.length; i++) {
                this.state[this.pos++] = data[i];
                if (this.pos === STROBE_R)
                    this.runF();
            }
        }
        beginOp(flags, more) {
            if (more) {
                if (this.curFlags !== flags) {
                    throw new Error(`Continued op with changed flags from ${this.curFlags.toString(2)} to ${flags.toString(2)}`);
                }
                return;
            }
            if ((flags & Flags.T) !== 0)
                throw new Error('T flag is not supported');
            const oldBegin = this.posBegin;
            this.posBegin = this.pos + 1;
            this.curFlags = flags;
            this.absorb(new Uint8Array([oldBegin, flags]));
            const forceF = (flags & (Flags.C | Flags.K)) !== 0;
            if (forceF && this.pos !== 0)
                this.runF();
        }
        metaAD(data, more) {
            this.beginOp(Flags.M | Flags.A, more);
            this.absorb(toData(data));
        }
        AD(data, more) {
            this.beginOp(Flags.A, more);
            this.absorb(toData(data));
        }
        PRF(len, more) {
            this.beginOp(Flags.I | Flags.A | Flags.C, more);
            return this.squeeze(len);
        }
        KEY(data, more) {
            this.beginOp(Flags.A | Flags.C, more);
            this.overwrite(toData(data));
        }
        clone() {
            const n = new Strobe128('0');
            n.pos = this.pos;
            n.posBegin = this.posBegin;
            n.state.set(this.state);
            n.curFlags = this.curFlags;
            return n;
        }
        clean() {
            this.state.fill(0);
            this.pos = 0;
            this.curFlags = 0;
            this.posBegin = 0;
        }
    }
    class Merlin {
        constructor(label) {
            this.strobe = new Strobe128('Merlin v1.0');
            this.appendMessage('dom-sep', label);
        }
        appendMessage(label, message) {
            this.strobe.metaAD(label, false);
            checkU32('Merlin.appendMessage', message.length);
            this.strobe.metaAD(numberToBytesLE(message.length, 4), true);
            this.strobe.AD(message, false);
        }
        challengeBytes(label, len) {
            this.strobe.metaAD(label, false);
            checkU32('Merlin.challengeBytes', len);
            this.strobe.metaAD(numberToBytesLE(len, 4), true);
            return this.strobe.PRF(len, false);
        }
        clean() {
            this.strobe.clean();
        }
    }
    class SigningContext extends Merlin {
        constructor(name, rng = randomBytes) {
            super(name);
            this.rng = rng;
        }
        label(label) {
            this.appendMessage('', label);
        }
        bytes(bytes) {
            this.appendMessage('sign-bytes', bytes);
            return this;
        }
        protoName(label) {
            this.appendMessage('proto-name', label);
        }
        commitPoint(label, point) {
            this.appendMessage(label, point.toRawBytes());
        }
        challengeScalar(label) {
            return modN(bytesToNumberLE(this.challengeBytes(label, 64)));
        }
        witnessScalar(label, nonceSeeds = []) {
            return modN(bytesToNumberLE(this.witnessBytes(label, 64, nonceSeeds)));
        }
        witnessBytes(label, len, nonceSeeds = []) {
            checkU32('SigningContext.witnessBytes', len);
            const strobeRng = this.strobe.clone();
            for (const ns of nonceSeeds) {
                strobeRng.metaAD(label, false);
                checkU32('SigningContext.witnessBytes nonce length', ns.length);
                strobeRng.metaAD(numberToBytesLE(ns.length, 4), true);
                strobeRng.KEY(ns, false);
            }
            const random = this.rng(32);
            strobeRng.metaAD('rng', false);
            strobeRng.KEY(random, false);
            strobeRng.metaAD(numberToBytesLE(len, 4), false);
            return strobeRng.PRF(len, false);
        }
    }
    const MASK = bitMask(256);
    const encodeScalar = (n) => numberToBytesLE((n << _3n) & MASK, 32);
    const decodeScalar = (n) => bytesToNumberLE(n) >> _3n;
    function getPublicKey(secretKey) {
        abytes('secretKey', secretKey, 64);
        const scalar = decodeScalar(secretKey.subarray(0, 32));
        return RistrettoPoint.BASE.multiply(scalar).toRawBytes();
    }
    function secretFromSeed(seed) {
        abytes('seed', seed, 32);
        const r = sha512(seed);
        r[0] &= 248;
        r[31] &= 63;
        r[31] |= 64;
        const key = encodeScalar(decodeScalar(r.subarray(0, 32)));
        const nonce = r.subarray(32, 64);
        const res = concatBytes(key, nonce);
        cleanBytes(key, nonce, r);
        return res;
    }
    const SUBSTRATE_CONTEXT = utf8ToBytes('substrate');
    function sign(secretKey, message, rng = randomBytes) {
        abytes('message', message);
        abytes('secretKey', secretKey, 64);
        const t = new SigningContext('SigningContext', rng);
        t.label(SUBSTRATE_CONTEXT);
        t.bytes(message);
        const keyScalar = decodeScalar(secretKey.subarray(0, 32));
        const nonce = secretKey.subarray(32, 64);
        const pubPoint = RistrettoPoint.fromHex(getPublicKey(secretKey));
        t.protoName('Schnorr-sig');
        t.commitPoint('sign:pk', pubPoint);
        const r = t.witnessScalar('signing', [nonce]);
        const R = RistrettoPoint.BASE.multiply(r);
        t.commitPoint('sign:R', R);
        const k = t.challengeScalar('sign:c');
        const s = modN(k * keyScalar + r);
        const res = concatBytes(R.toRawBytes(), numberToBytesLE(s, 32));
        res[63] |= 128;
        t.clean();
        return res;
    }
    function verify(message, signature, publicKey) {
        abytes('message', message);
        abytes('signature', signature, 64);
        abytes('publicKey', publicKey, 32);
        if ((signature[63] & 128) === 0)
            throw new Error('Schnorrkel marker missing');
        const sBytes = Uint8Array.from(signature.subarray(32, 64));
        sBytes[31] &= 127;
        const R = RistrettoPoint.fromHex(signature.subarray(0, 32));
        const s = bytesToNumberLE(sBytes);
        aInRange('s', s, _0n$1, CURVE_ORDER);
        const t = new SigningContext('SigningContext');
        t.label(SUBSTRATE_CONTEXT);
        t.bytes(message);
        const pubPoint = RistrettoPoint.fromHex(publicKey);
        if (pubPoint.equals(RistrettoPoint.ZERO))
            return false;
        t.protoName('Schnorr-sig');
        t.commitPoint('sign:pk', pubPoint);
        t.commitPoint('sign:R', R);
        const k = t.challengeScalar('sign:c');
        const sP = RistrettoPoint.BASE.multiply(s);
        const RR = pubPoint.negate().multiply(k).add(sP);
        t.clean();
        cleanBytes(sBytes);
        return RR.equals(R);
    }
    function getSharedSecret(secretKey, publicKey) {
        abytes('secretKey', secretKey, 64);
        abytes('publicKey', publicKey, 32);
        const keyScalar = decodeScalar(secretKey.subarray(0, 32));
        const pubPoint = RistrettoPoint.fromHex(publicKey);
        if (pubPoint.equals(RistrettoPoint.ZERO))
            throw new Error('wrong public key (infinity)');
        return pubPoint.multiply(keyScalar).toRawBytes();
    }
    const HDKD = {
        secretSoft(secretKey, chainCode, rng = randomBytes) {
            abytes('secretKey', secretKey, 64);
            abytes('chainCode', chainCode, 32);
            const masterScalar = decodeScalar(secretKey.subarray(0, 32));
            const masterNonce = secretKey.subarray(32, 64);
            const pubPoint = RistrettoPoint.fromHex(getPublicKey(secretKey));
            const t = new SigningContext('SchnorrRistrettoHDKD', rng);
            t.bytes(EMPTY);
            t.appendMessage('chain-code', chainCode);
            t.commitPoint('public-key', pubPoint);
            const scalar = t.challengeScalar('HDKD-scalar');
            const hdkdChainCode = t.challengeBytes('HDKD-chaincode', 32);
            const nonceSeed = concatBytes(numberToBytesLE(masterScalar, 32), masterNonce);
            const nonce = t.witnessBytes('HDKD-nonce', 32, [masterNonce, nonceSeed]);
            const key = encodeScalar(modN(masterScalar + scalar));
            const res = concatBytes(key, nonce);
            cleanBytes(key, nonce, nonceSeed, hdkdChainCode);
            t.clean();
            return res;
        },
        publicSoft(publicKey, chainCode) {
            abytes('publicKey', publicKey, 32);
            abytes('chainCode', chainCode, 32);
            const pubPoint = RistrettoPoint.fromHex(publicKey);
            const t = new SigningContext('SchnorrRistrettoHDKD');
            t.bytes(EMPTY);
            t.appendMessage('chain-code', chainCode);
            t.commitPoint('public-key', pubPoint);
            const scalar = t.challengeScalar('HDKD-scalar');
            t.challengeBytes('HDKD-chaincode', 32);
            t.clean();
            return pubPoint.add(RistrettoPoint.BASE.multiply(scalar)).toRawBytes();
        },
        secretHard(secretKey, chainCode) {
            abytes('secretKey', secretKey, 64);
            abytes('chainCode', chainCode, 32);
            const key = numberToBytesLE(decodeScalar(secretKey.subarray(0, 32)), 32);
            const t = new SigningContext('SchnorrRistrettoHDKD');
            t.bytes(EMPTY);
            t.appendMessage('chain-code', chainCode);
            t.appendMessage('secret-key', key);
            const msk = t.challengeBytes('HDKD-hard', 32);
            const hdkdChainCode = t.challengeBytes('HDKD-chaincode', 32);
            t.clean();
            const res = secretFromSeed(msk);
            cleanBytes(key, msk, hdkdChainCode);
            t.clean();
            return res;
        },
    };
    const dleq = {
        proove(keyScalar, nonce, pubPoint, t, input, output) {
            t.protoName('DLEQProof');
            t.commitPoint('vrf:h', input);
            const r = t.witnessScalar(`proving${'\0'}0`, [nonce]);
            const R = RistrettoPoint.BASE.multiply(r);
            t.commitPoint('vrf:R=g^r', R);
            const Hr = input.multiply(r);
            t.commitPoint('vrf:h^r', Hr);
            t.commitPoint('vrf:pk', pubPoint);
            t.commitPoint('vrf:h^sk', output);
            const c = t.challengeScalar('prove');
            const s = modN(r - c * keyScalar);
            return { proof: { c, s }, proofBatchable: { R, Hr, s } };
        },
        verify(pubPoint, t, input, output, proof) {
            if (pubPoint.equals(RistrettoPoint.ZERO))
                return false;
            t.protoName('DLEQProof');
            t.commitPoint('vrf:h', input);
            const R = pubPoint.multiply(proof.c).add(RistrettoPoint.BASE.multiply(proof.s));
            t.commitPoint('vrf:R=g^r', R);
            const Hr = output.multiply(proof.c).add(input.multiply(proof.s));
            t.commitPoint('vrf:h^r', Hr);
            t.commitPoint('vrf:pk', pubPoint);
            t.commitPoint('vrf:h^sk', output);
            const realC = t.challengeScalar('prove');
            if (proof.c === realC)
                return { R, Hr, s: proof.s };
            return false;
        },
    };
    function initVRF(ctx, msg, extra, pubPoint, rng = randomBytes) {
        const t = new SigningContext('SigningContext', rng);
        t.label(ctx);
        t.bytes(msg);
        t.commitPoint('vrf-nm-pk', pubPoint);
        const hash = t.challengeBytes('VRFHash', 64);
        const input = RistrettoPoint.hashToCurve(hash);
        const transcript = new SigningContext('VRF', rng);
        if (extra.length)
            transcript.label(extra);
        t.clean();
        cleanBytes(hash);
        return { input, t: transcript };
    }
    const vrf = {
        sign(msg, secretKey, ctx = EMPTY, extra = EMPTY, rng = randomBytes) {
            abytes('msg', msg);
            abytes('secretKey', secretKey, 64);
            abytes('ctx', ctx);
            abytes('extra', extra);
            const keyScalar = decodeScalar(secretKey.subarray(0, 32));
            const nonce = secretKey.subarray(32, 64);
            const pubPoint = RistrettoPoint.fromHex(getPublicKey(secretKey));
            const { input, t } = initVRF(ctx, msg, extra, pubPoint, rng);
            const output = input.multiply(keyScalar);
            const p = { input, output };
            const { proof } = dleq.proove(keyScalar, nonce, pubPoint, t, input, output);
            const cBytes = numberToBytesLE(proof.c, 32);
            const sBytes = numberToBytesLE(proof.s, 32);
            const res = concatBytes(p.output.toRawBytes(), cBytes, sBytes);
            cleanBytes(nonce, cBytes, sBytes);
            return res;
        },
        verify(msg, signature, publicKey, ctx = EMPTY, extra = EMPTY, rng = randomBytes) {
            abytes('msg', msg);
            abytes('signature', signature, 96);
            abytes('pubkey', publicKey, 32);
            abytes('ctx', ctx);
            abytes('extra', extra);
            const pubPoint = RistrettoPoint.fromHex(publicKey);
            if (pubPoint.equals(RistrettoPoint.ZERO))
                return false;
            const proof = {
                c: parseScalar('signature.c', signature.subarray(32, 64)),
                s: parseScalar('signature.s', signature.subarray(64, 96)),
            };
            const { input, t } = initVRF(ctx, msg, extra, pubPoint, rng);
            const output = RistrettoPoint.fromHex(signature.subarray(0, 32));
            if (output.equals(RistrettoPoint.ZERO))
                throw new Error('vrf.verify: wrong output point (identity)');
            const proofBatchable = dleq.verify(pubPoint, t, input, output, proof);
            return proofBatchable === false ? false : true;
        },
    };

    function createDeriveFn(derive) {
        return (keypair, chainCode) => {
            if (!util.isU8a(chainCode) || chainCode.length !== 32) {
                throw new Error('Invalid chainCode passed to derive');
            }
            const secretKey = derive(keypair.secretKey, chainCode);
            const publicKey = getPublicKey(secretKey);
            return { publicKey, secretKey };
        };
    }

    const sr25519DeriveHard =  createDeriveFn(HDKD.secretHard);

    const sr25519DeriveSoft =  createDeriveFn(HDKD.secretSoft);

    function keyHdkdSr25519(keypair, { chainCode, isSoft }) {
        return isSoft
            ? sr25519DeriveSoft(keypair, chainCode)
            : sr25519DeriveHard(keypair, chainCode);
    }

    const generators = {
        ecdsa: keyHdkdEcdsa,
        ed25519: keyHdkdEd25519,
        ethereum: keyHdkdEcdsa,
        sr25519: keyHdkdSr25519
    };
    function keyFromPath(pair, path, type) {
        const keyHdkd = generators[type];
        let result = pair;
        for (const junction of path) {
            result = keyHdkd(result, junction);
        }
        return result;
    }

    function sr25519Agreement(secretKey, publicKey) {
        const secretKeyU8a = util.u8aToU8a(secretKey);
        const publicKeyU8a = util.u8aToU8a(publicKey);
        if (publicKeyU8a.length !== 32) {
            throw new Error(`Invalid publicKey, received ${publicKeyU8a.length} bytes, expected 32`);
        }
        else if (secretKeyU8a.length !== 64) {
            throw new Error(`Invalid secretKey, received ${secretKeyU8a.length} bytes, expected 64`);
        }
        return getSharedSecret(secretKeyU8a, publicKeyU8a);
    }

    function sr25519DerivePublic(publicKey, chainCode) {
        const publicKeyU8a = util.u8aToU8a(publicKey);
        if (!util.isU8a(chainCode) || chainCode.length !== 32) {
            throw new Error('Invalid chainCode passed to derive');
        }
        else if (publicKeyU8a.length !== 32) {
            throw new Error(`Invalid publicKey, received ${publicKeyU8a.length} bytes, expected 32`);
        }
        return HDKD.publicSoft(publicKeyU8a, chainCode);
    }

    function sr25519PairFromSeed(seed) {
        const seedU8a = util.u8aToU8a(seed);
        if (seedU8a.length !== 32) {
            throw new Error(`Expected a seed matching 32 bytes, found ${seedU8a.length}`);
        }
        const sec = secretFromSeed(seedU8a);
        const pub = getPublicKey(sec);
        return {
            publicKey: pub,
            secretKey: sec
        };
    }

    function sr25519Sign(message, { publicKey, secretKey }) {
        if (publicKey?.length !== 32) {
            throw new Error('Expected a valid publicKey, 32-bytes');
        }
        else if (secretKey?.length !== 64) {
            throw new Error('Expected a valid secretKey, 64-bytes');
        }
        return sign(secretKey, util.u8aToU8a(message));
    }

    function sr25519Verify(message, signature, publicKey) {
        const publicKeyU8a = util.u8aToU8a(publicKey);
        const signatureU8a = util.u8aToU8a(signature);
        if (publicKeyU8a.length !== 32) {
            throw new Error(`Invalid publicKey, received ${publicKeyU8a.length} bytes, expected 32`);
        }
        else if (signatureU8a.length !== 64) {
            throw new Error(`Invalid signature, received ${signatureU8a.length} bytes, expected 64`);
        }
        return verify(util.u8aToU8a(message), signatureU8a, publicKeyU8a);
    }

    const EMPTY_U8A$1 = new Uint8Array();
    function sr25519VrfSign(message, { secretKey }, context = EMPTY_U8A$1, extra = EMPTY_U8A$1) {
        if (secretKey?.length !== 64) {
            throw new Error('Invalid secretKey, expected 64-bytes');
        }
        return vrf.sign(util.u8aToU8a(message), secretKey, util.u8aToU8a(context), util.u8aToU8a(extra), randomBytes$1);
    }

    const EMPTY_U8A = new Uint8Array();
    function sr25519VrfVerify(message, signOutput, publicKey, context = EMPTY_U8A, extra = EMPTY_U8A) {
        const publicKeyU8a = util.u8aToU8a(publicKey);
        const proofU8a = util.u8aToU8a(signOutput);
        if (publicKeyU8a.length !== 32) {
            throw new Error('Invalid publicKey, expected 32-bytes');
        }
        else if (proofU8a.length !== 96) {
            throw new Error('Invalid vrfSign output, expected 96 bytes');
        }
        return vrf.verify(util.u8aToU8a(message), proofU8a, publicKeyU8a, util.u8aToU8a(context), util.u8aToU8a(extra));
    }

    function encodeAddress(key, ss58Format = defaults.prefix) {
        const u8a = decodeAddress(key);
        if ((ss58Format < 0) || (ss58Format > 16383 && !ss58Exceptions.includes(ss58Format)) || [46, 47].includes(ss58Format)) {
            throw new Error('Out of range ss58Format specified');
        }
        else if (!defaults.allowedDecodedLengths.includes(u8a.length)) {
            throw new Error(`Expected a valid key to convert, with length ${defaults.allowedDecodedLengths.join(', ')}`);
        }
        const input = util.u8aConcat(ss58Format < 64
            ? [ss58Format]
            : [
                ((ss58Format & 0b0000_0000_1111_1100) >> 2) | 0b0100_0000,
                (ss58Format >> 8) | ((ss58Format & 0b0000_0000_0000_0011) << 6)
            ], u8a);
        return base58Encode(util.u8aConcat(input, sshash(input).subarray(0, [32, 33].includes(u8a.length) ? 2 : 1)));
    }
    const ss58Exceptions = [29972];

    function filterHard({ isHard }) {
        return isHard;
    }
    function deriveAddress(who, suri, ss58Format) {
        const { path } = keyExtractPath(suri);
        if (!path.length || path.every(filterHard)) {
            throw new Error('Expected suri to contain a combination of non-hard paths');
        }
        let publicKey = decodeAddress(who);
        for (const { chainCode } of path) {
            publicKey = sr25519DerivePublic(publicKey, chainCode);
        }
        return encodeAddress(publicKey, ss58Format);
    }

    const PREFIX$1 = util.stringToU8a('modlpy/utilisuba');
    function createKeyDerived(who, index) {
        return blake2AsU8a(util.u8aConcat(PREFIX$1, decodeAddress(who), util.bnToU8a(index, BN_LE_16_OPTS)));
    }

    function encodeDerivedAddress(who, index, ss58Format) {
        return encodeAddress(createKeyDerived(decodeAddress(who), index), ss58Format);
    }

    function addressToU8a(who) {
        return decodeAddress(who);
    }

    const PREFIX = util.stringToU8a('modlpy/utilisuba');
    function createKeyMulti(who, threshold) {
        return blake2AsU8a(util.u8aConcat(PREFIX, util.compactToU8a(who.length), ...util.u8aSorted(who.map(addressToU8a)), util.bnToU8a(threshold, BN_LE_16_OPTS)));
    }

    function encodeMultiAddress(who, threshold, ss58Format) {
        return encodeAddress(createKeyMulti(who, threshold), ss58Format);
    }

    function addressEq(a, b) {
        return util.u8aEq(decodeAddress(a), decodeAddress(b));
    }

    const [SHA3_PI, SHA3_ROTL, _SHA3_IOTA] = [[], [], []];
    const _0n =  BigInt(0);
    const _1n =  BigInt(1);
    const _2n =  BigInt(2);
    const _7n$1 =  BigInt(7);
    const _256n$1 =  BigInt(256);
    const _0x71n =  BigInt(0x71);
    for (let round = 0, R = _1n, x = 1, y = 0; round < 24; round++) {
        [x, y] = [y, (2 * x + 3 * y) % 5];
        SHA3_PI.push(2 * (5 * y + x));
        SHA3_ROTL.push((((round + 1) * (round + 2)) / 2) % 64);
        let t = _0n;
        for (let j = 0; j < 7; j++) {
            R = ((R << _1n) ^ ((R >> _7n$1) * _0x71n)) % _256n$1;
            if (R & _2n)
                t ^= _1n << ((_1n <<  BigInt(j)) - _1n);
        }
        _SHA3_IOTA.push(t);
    }
    const [SHA3_IOTA_H, SHA3_IOTA_L] =  split$1(_SHA3_IOTA, true);
    const rotlH = (h, l, s) => (s > 32 ? rotlBH$1(h, l, s) : rotlSH$1(h, l, s));
    const rotlL = (h, l, s) => (s > 32 ? rotlBL$1(h, l, s) : rotlSL$1(h, l, s));
    function keccakP(s, rounds = 24) {
        const B = new Uint32Array(5 * 2);
        for (let round = 24 - rounds; round < 24; round++) {
            for (let x = 0; x < 10; x++)
                B[x] = s[x] ^ s[x + 10] ^ s[x + 20] ^ s[x + 30] ^ s[x + 40];
            for (let x = 0; x < 10; x += 2) {
                const idx1 = (x + 8) % 10;
                const idx0 = (x + 2) % 10;
                const B0 = B[idx0];
                const B1 = B[idx0 + 1];
                const Th = rotlH(B0, B1, 1) ^ B[idx1];
                const Tl = rotlL(B0, B1, 1) ^ B[idx1 + 1];
                for (let y = 0; y < 50; y += 10) {
                    s[x + y] ^= Th;
                    s[x + y + 1] ^= Tl;
                }
            }
            let curH = s[2];
            let curL = s[3];
            for (let t = 0; t < 24; t++) {
                const shift = SHA3_ROTL[t];
                const Th = rotlH(curH, curL, shift);
                const Tl = rotlL(curH, curL, shift);
                const PI = SHA3_PI[t];
                curH = s[PI];
                curL = s[PI + 1];
                s[PI] = Th;
                s[PI + 1] = Tl;
            }
            for (let y = 0; y < 50; y += 10) {
                for (let x = 0; x < 10; x++)
                    B[x] = s[y + x];
                for (let x = 0; x < 10; x++)
                    s[y + x] ^= ~B[(x + 2) % 10] & B[(x + 4) % 10];
            }
            s[0] ^= SHA3_IOTA_H[round];
            s[1] ^= SHA3_IOTA_L[round];
        }
        B.fill(0);
    }
    class Keccak extends Hash$1 {
        constructor(blockLen, suffix, outputLen, enableXOF = false, rounds = 24) {
            super();
            this.blockLen = blockLen;
            this.suffix = suffix;
            this.outputLen = outputLen;
            this.enableXOF = enableXOF;
            this.rounds = rounds;
            this.pos = 0;
            this.posOut = 0;
            this.finished = false;
            this.destroyed = false;
            number(outputLen);
            if (0 >= this.blockLen || this.blockLen >= 200)
                throw new Error('Sha3 supports only keccak-f1600 function');
            this.state = new Uint8Array(200);
            this.state32 = u32$1(this.state);
        }
        keccak() {
            keccakP(this.state32, this.rounds);
            this.posOut = 0;
            this.pos = 0;
        }
        update(data) {
            exists(this);
            const { blockLen, state } = this;
            data = toBytes$1(data);
            const len = data.length;
            for (let pos = 0; pos < len;) {
                const take = Math.min(blockLen - this.pos, len - pos);
                for (let i = 0; i < take; i++)
                    state[this.pos++] ^= data[pos++];
                if (this.pos === blockLen)
                    this.keccak();
            }
            return this;
        }
        finish() {
            if (this.finished)
                return;
            this.finished = true;
            const { state, suffix, pos, blockLen } = this;
            state[pos] ^= suffix;
            if ((suffix & 0x80) !== 0 && pos === blockLen - 1)
                this.keccak();
            state[blockLen - 1] ^= 0x80;
            this.keccak();
        }
        writeInto(out) {
            exists(this, false);
            bytes(out);
            this.finish();
            const bufferOut = this.state;
            const { blockLen } = this;
            for (let pos = 0, len = out.length; pos < len;) {
                if (this.posOut >= blockLen)
                    this.keccak();
                const take = Math.min(blockLen - this.posOut, len - pos);
                out.set(bufferOut.subarray(this.posOut, this.posOut + take), pos);
                this.posOut += take;
                pos += take;
            }
            return out;
        }
        xofInto(out) {
            if (!this.enableXOF)
                throw new Error('XOF is not possible for this instance');
            return this.writeInto(out);
        }
        xof(bytes) {
            number(bytes);
            return this.xofInto(new Uint8Array(bytes));
        }
        digestInto(out) {
            output(out, this);
            if (this.finished)
                throw new Error('digest() was already called');
            this.writeInto(out);
            this.destroy();
            return out;
        }
        digest() {
            return this.digestInto(new Uint8Array(this.outputLen));
        }
        destroy() {
            this.destroyed = true;
            this.state.fill(0);
        }
        _cloneInto(to) {
            const { blockLen, suffix, outputLen, rounds, enableXOF } = this;
            to || (to = new Keccak(blockLen, suffix, outputLen, enableXOF, rounds));
            to.state32.set(this.state32);
            to.pos = this.pos;
            to.posOut = this.posOut;
            to.finished = this.finished;
            to.rounds = rounds;
            to.suffix = suffix;
            to.outputLen = outputLen;
            to.enableXOF = enableXOF;
            to.destroyed = this.destroyed;
            return to;
        }
    }
    const gen = (suffix, blockLen, outputLen) => wrapConstructor(() => new Keccak(blockLen, suffix, outputLen));
    gen(0x06, 144, 224 / 8);
    gen(0x06, 136, 256 / 8);
    gen(0x06, 104, 384 / 8);
    gen(0x06, 72, 512 / 8);
    gen(0x01, 144, 224 / 8);
    const keccak_256 =  gen(0x01, 136, 256 / 8);
    gen(0x01, 104, 384 / 8);
    const keccak_512 =  gen(0x01, 72, 512 / 8);
    const genShake = (suffix, blockLen, outputLen) => wrapXOFConstructorWithOpts((opts = {}) => new Keccak(blockLen, suffix, opts.dkLen === undefined ? outputLen : opts.dkLen, true));
    genShake(0x1f, 168, 128 / 8);
    genShake(0x1f, 136, 256 / 8);

    const keccakAsU8a =  createDualHasher({ 256: keccak256, 512: keccak512 }, { 256: keccak_256, 512: keccak_512 });
    const keccak256AsU8a =  createBitHasher(256, keccakAsU8a);
    const keccak512AsU8a =  createBitHasher(512, keccakAsU8a);
    const keccakAsHex =  createAsHex(keccakAsU8a);

    function hasher(hashType, data, onlyJs) {
        return hashType === 'keccak'
            ? keccakAsU8a(data, undefined, onlyJs)
            : blake2AsU8a(data, undefined, undefined, onlyJs);
    }

    function evmToAddress(evmAddress, ss58Format, hashType = 'blake2') {
        const message = util.u8aConcat('evm:', evmAddress);
        if (message.length !== 24) {
            throw new Error(`Converting ${evmAddress}: Invalid evm address length`);
        }
        return encodeAddress(hasher(hashType, message), ss58Format);
    }

    function validateAddress(encoded, ignoreChecksum, ss58Format) {
        return !!decodeAddress(encoded, ignoreChecksum, ss58Format);
    }

    function isAddress(address, ignoreChecksum, ss58Format) {
        try {
            return validateAddress(address, ignoreChecksum, ss58Format);
        }
        catch {
            return false;
        }
    }

    function sortAddresses(addresses, ss58Format) {
        const u8aToAddress = (u8a) => encodeAddress(u8a, ss58Format);
        return util.u8aSorted(addresses.map(addressToU8a)).map(u8aToAddress);
    }

    const l = util.logger('setSS58Format');
    function setSS58Format(prefix) {
        l.warn('Global setting of the ss58Format is deprecated and not recommended. Set format on the keyring (if used) or as part of the address encode function');
        defaults.prefix = prefix;
    }

    const chars = 'abcdefghijklmnopqrstuvwxyz234567';
    const config$1 = {
        chars,
        coder: utils.chain(
        utils.radix2(5), utils.alphabet(chars), {
            decode: (input) => input.split(''),
            encode: (input) => input.join('')
        }),
        ipfs: 'b',
        type: 'base32'
    };
    const base32Validate =  createValidate(config$1);
    const isBase32 =  createIs(base32Validate);
    const base32Decode =  createDecode(config$1, base32Validate);
    const base32Encode =  createEncode(config$1);

    const config = {
        chars: 'ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/',
        coder: base64,
        type: 'base64',
        withPadding: true
    };
    const base64Validate =  createValidate(config);
    const isBase64 =  createIs(base64Validate);
    const base64Decode =  createDecode(config, base64Validate);
    const base64Encode =  createEncode(config);

    function base64Pad(value) {
        return value.padEnd(value.length + (value.length % 4), '=');
    }

    function base64Trim(value) {
        while (value.length && value.endsWith('=')) {
            value = value.slice(0, -1);
        }
        return value;
    }

    function secp256k1Compress(publicKey, onlyJs) {
        if (![33, 65].includes(publicKey.length)) {
            throw new Error(`Invalid publicKey provided, received ${publicKey.length} bytes input`);
        }
        if (publicKey.length === 33) {
            return publicKey;
        }
        return !util.hasBigInt || (!onlyJs && isReady())
            ? secp256k1Compress$1(publicKey)
            : secp256k1.ProjectivePoint.fromHex(publicKey).toRawBytes(true);
    }

    function secp256k1Expand(publicKey, onlyJs) {
        if (![33, 65].includes(publicKey.length)) {
            throw new Error(`Invalid publicKey provided, received ${publicKey.length} bytes input`);
        }
        if (publicKey.length === 65) {
            return publicKey.subarray(1);
        }
        if (!util.hasBigInt || (!onlyJs && isReady())) {
            return secp256k1Expand$1(publicKey).subarray(1);
        }
        const { x, y } = secp256k1.ProjectivePoint.fromHex(publicKey);
        return util.u8aConcat(util.bnToU8a(x, BN_BE_256_OPTS), util.bnToU8a(y, BN_BE_256_OPTS));
    }

    function secp256k1Recover(msgHash, signature, recovery, hashType = 'blake2', onlyJs) {
        const sig = util.u8aToU8a(signature).subarray(0, 64);
        const msg = util.u8aToU8a(msgHash);
        const publicKey = !util.hasBigInt || (!onlyJs && isReady())
            ? secp256k1Recover$1(msg, sig, recovery)
            : secp256k1.Signature
                .fromCompact(sig)
                .addRecoveryBit(recovery)
                .recoverPublicKey(msg)
                .toRawBytes();
        if (!publicKey) {
            throw new Error('Unable to recover publicKey from signature');
        }
        return hashType === 'keccak'
            ? secp256k1Expand(publicKey, onlyJs)
            : secp256k1Compress(publicKey, onlyJs);
    }

    function secp256k1Sign(message, { secretKey }, hashType = 'blake2', onlyJs) {
        if (secretKey?.length !== 32) {
            throw new Error('Expected valid secp256k1 secretKey, 32-bytes');
        }
        const data = hasher(hashType, message, onlyJs);
        if (!util.hasBigInt || (!onlyJs && isReady())) {
            return secp256k1Sign$1(data, secretKey);
        }
        const signature = secp256k1.sign(data, secretKey, { lowS: true });
        return util.u8aConcat(util.bnToU8a(signature.r, BN_BE_256_OPTS), util.bnToU8a(signature.s, BN_BE_256_OPTS), new Uint8Array([signature.recovery || 0]));
    }

    const N = 'ffffffff ffffffff ffffffff fffffffe baaedce6 af48a03b bfd25e8c d0364141'.replace(/ /g, '');
    const N_BI = BigInt$1(`0x${N}`);
    const N_BN = new util.BN(N, 'hex');
    function addBi(seckey, tweak) {
        let res = util.u8aToBigInt(tweak, BN_BE_OPTS);
        if (res >= N_BI) {
            throw new Error('Tweak parameter is out of range');
        }
        res += util.u8aToBigInt(seckey, BN_BE_OPTS);
        if (res >= N_BI) {
            res -= N_BI;
        }
        if (res === util._0n) {
            throw new Error('Invalid resulting private key');
        }
        return util.nToU8a(res, BN_BE_256_OPTS);
    }
    function addBn(seckey, tweak) {
        const res = new util.BN(tweak);
        if (res.cmp(N_BN) >= 0) {
            throw new Error('Tweak parameter is out of range');
        }
        res.iadd(new util.BN(seckey));
        if (res.cmp(N_BN) >= 0) {
            res.isub(N_BN);
        }
        if (res.isZero()) {
            throw new Error('Invalid resulting private key');
        }
        return util.bnToU8a(res, BN_BE_256_OPTS);
    }
    function secp256k1PrivateKeyTweakAdd(seckey, tweak, onlyBn) {
        if (!util.isU8a(seckey) || seckey.length !== 32) {
            throw new Error('Expected seckey to be an Uint8Array with length 32');
        }
        else if (!util.isU8a(tweak) || tweak.length !== 32) {
            throw new Error('Expected tweak to be an Uint8Array with length 32');
        }
        return !util.hasBigInt || onlyBn
            ? addBn(seckey, tweak)
            : addBi(seckey, tweak);
    }

    function secp256k1Verify(msgHash, signature, address, hashType = 'blake2', onlyJs) {
        const sig = util.u8aToU8a(signature);
        if (sig.length !== 65) {
            throw new Error(`Expected signature with 65 bytes, ${sig.length} found instead`);
        }
        const publicKey = secp256k1Recover(hasher(hashType, msgHash), sig, sig[64], hashType, onlyJs);
        const signerAddr = hasher(hashType, publicKey, onlyJs);
        const inputAddr = util.u8aToU8a(address);
        return util.u8aEq(publicKey, inputAddr) || (hashType === 'keccak'
            ? util.u8aEq(signerAddr.slice(-20), inputAddr.slice(-20))
            : util.u8aEq(signerAddr, inputAddr));
    }

    function getH160(u8a) {
        if ([33, 65].includes(u8a.length)) {
            u8a = keccakAsU8a(secp256k1Expand(u8a));
        }
        return u8a.slice(-20);
    }
    function ethereumEncode(addressOrPublic) {
        if (!addressOrPublic) {
            return '0x';
        }
        const u8aAddress = util.u8aToU8a(addressOrPublic);
        if (![20, 32, 33, 65].includes(u8aAddress.length)) {
            throw new Error(`Invalid address or publicKey provided, received ${u8aAddress.length} bytes input`);
        }
        const address = util.u8aToHex(getH160(u8aAddress), -1, false);
        const hash = util.u8aToHex(keccakAsU8a(address), -1, false);
        let result = '';
        for (let i = 0; i < 40; i++) {
            result = `${result}${parseInt(hash[i], 16) > 7 ? address[i].toUpperCase() : address[i]}`;
        }
        return `0x${result}`;
    }

    function isInvalidChar(char, byte) {
        return char !== (byte > 7
            ? char.toUpperCase()
            : char.toLowerCase());
    }
    function isEthereumChecksum(_address) {
        const address = _address.replace('0x', '');
        const hash = util.u8aToHex(keccakAsU8a(address.toLowerCase()), -1, false);
        for (let i = 0; i < 40; i++) {
            if (isInvalidChar(address[i], parseInt(hash[i], 16))) {
                return false;
            }
        }
        return true;
    }

    function isEthereumAddress(address) {
        if (!address || address.length !== 42 || !util.isHex(address)) {
            return false;
        }
        else if (/^(0x)?[0-9a-f]{40}$/.test(address) || /^(0x)?[0-9A-F]{40}$/.test(address)) {
            return true;
        }
        return isEthereumChecksum(address);
    }

    const JS_HASH = {
        256: sha256,
        512: sha512$2
    };
    const WA_MHAC = {
        256: hmacSha256,
        512: hmacSha512
    };
    function createSha(bitLength) {
        return (key, data, onlyJs) => hmacShaAsU8a(key, data, bitLength, onlyJs);
    }
    function hmacShaAsU8a(key, data, bitLength = 256, onlyJs) {
        const u8aKey = util.u8aToU8a(key);
        return !util.hasBigInt || (!onlyJs && isReady())
            ? WA_MHAC[bitLength](u8aKey, data)
            : hmac(JS_HASH[bitLength], u8aKey, data);
    }
    const hmacSha256AsU8a =  createSha(256);
    const hmacSha512AsU8a =  createSha(512);

    const HARDENED = 0x80000000;
    function hdValidatePath(path) {
        if (!path.startsWith('m/')) {
            return false;
        }
        const parts = path.split('/').slice(1);
        for (const p of parts) {
            const n = /^\d+'?$/.test(p)
                ? parseInt(p.replace(/'$/, ''), 10)
                : Number.NaN;
            if (isNaN(n) || (n >= HARDENED) || (n < 0)) {
                return false;
            }
        }
        return true;
    }

    const MASTER_SECRET = util.stringToU8a('Bitcoin seed');
    function createCoded(secretKey, chainCode) {
        return {
            chainCode,
            publicKey: secp256k1PairFromSeed(secretKey).publicKey,
            secretKey
        };
    }
    function deriveChild(hd, index) {
        const indexBuffer = util.bnToU8a(index, BN_BE_32_OPTS);
        const data = index >= HARDENED
            ? util.u8aConcat(new Uint8Array(1), hd.secretKey, indexBuffer)
            : util.u8aConcat(hd.publicKey, indexBuffer);
        try {
            const I = hmacShaAsU8a(hd.chainCode, data, 512);
            return createCoded(secp256k1PrivateKeyTweakAdd(hd.secretKey, I.slice(0, 32)), I.slice(32));
        }
        catch {
            return deriveChild(hd, index + 1);
        }
    }
    function hdEthereum(seed, path = '') {
        const I = hmacShaAsU8a(MASTER_SECRET, seed, 512);
        let hd = createCoded(I.slice(0, 32), I.slice(32));
        if (!path || path === 'm' || path === 'M' || path === "m'" || path === "M'") {
            return hd;
        }
        if (!hdValidatePath(path)) {
            throw new Error('Invalid derivation path');
        }
        const parts = path.split('/').slice(1);
        for (const p of parts) {
            hd = deriveChild(hd, parseInt(p, 10) + ((p.length > 1) && p.endsWith("'")
                ? HARDENED
                : 0));
        }
        return hd;
    }

    function pbkdf2Init(hash$1, _password, _salt, _opts) {
        hash(hash$1);
        const opts = checkOpts({ dkLen: 32, asyncTick: 10 }, _opts);
        const { c, dkLen, asyncTick } = opts;
        number(c);
        number(dkLen);
        number(asyncTick);
        if (c < 1)
            throw new Error('PBKDF2: iterations (c) should be >= 1');
        const password = toBytes$1(_password);
        const salt = toBytes$1(_salt);
        const DK = new Uint8Array(dkLen);
        const PRF = hmac.create(hash$1, password);
        const PRFSalt = PRF._cloneInto().update(salt);
        return { c, dkLen, asyncTick, DK, PRF, PRFSalt };
    }
    function pbkdf2Output(PRF, PRFSalt, DK, prfW, u) {
        PRF.destroy();
        PRFSalt.destroy();
        if (prfW)
            prfW.destroy();
        u.fill(0);
        return DK;
    }
    function pbkdf2(hash, password, salt, opts) {
        const { c, dkLen, DK, PRF, PRFSalt } = pbkdf2Init(hash, password, salt, opts);
        let prfW;
        const arr = new Uint8Array(4);
        const view = createView$1(arr);
        const u = new Uint8Array(PRF.outputLen);
        for (let ti = 1, pos = 0; pos < dkLen; ti++, pos += PRF.outputLen) {
            const Ti = DK.subarray(pos, pos + PRF.outputLen);
            view.setInt32(0, ti, false);
            (prfW = PRFSalt._cloneInto(prfW)).update(arr).digestInto(u);
            Ti.set(u.subarray(0, Ti.length));
            for (let ui = 1; ui < c; ui++) {
                PRF._cloneInto(prfW).update(u).digestInto(u);
                for (let i = 0; i < Ti.length; i++)
                    Ti[i] ^= u[i];
            }
        }
        return pbkdf2Output(PRF, PRFSalt, DK, prfW, u);
    }

    function pbkdf2Encode(passphrase, salt = randomAsU8a(), rounds = 2048, onlyJs) {
        const u8aPass = util.u8aToU8a(passphrase);
        const u8aSalt = util.u8aToU8a(salt);
        return {
            password: !util.hasBigInt || (!onlyJs && isReady())
                ? pbkdf2$1(u8aPass, u8aSalt, rounds)
                : pbkdf2(sha512$2, u8aPass, u8aSalt, { c: rounds, dkLen: 64 }),
            rounds,
            salt
        };
    }

    const shaAsU8a =  createDualHasher({ 256: sha256$1, 512: sha512$3 }, { 256: sha256, 512: sha512$2 });
    const sha256AsU8a =  createBitHasher(256, shaAsU8a);
    const sha512AsU8a =  createBitHasher(512, shaAsU8a);

    const DEFAULT_WORDLIST = 'abandon|ability|able|about|above|absent|absorb|abstract|absurd|abuse|access|accident|account|accuse|achieve|acid|acoustic|acquire|across|act|action|actor|actress|actual|adapt|add|addict|address|adjust|admit|adult|advance|advice|aerobic|affair|afford|afraid|again|age|agent|agree|ahead|aim|air|airport|aisle|alarm|album|alcohol|alert|alien|all|alley|allow|almost|alone|alpha|already|also|alter|always|amateur|amazing|among|amount|amused|analyst|anchor|ancient|anger|angle|angry|animal|ankle|announce|annual|another|answer|antenna|antique|anxiety|any|apart|apology|appear|apple|approve|april|arch|arctic|area|arena|argue|arm|armed|armor|army|around|arrange|arrest|arrive|arrow|art|artefact|artist|artwork|ask|aspect|assault|asset|assist|assume|asthma|athlete|atom|attack|attend|attitude|attract|auction|audit|august|aunt|author|auto|autumn|average|avocado|avoid|awake|aware|away|awesome|awful|awkward|axis|baby|bachelor|bacon|badge|bag|balance|balcony|ball|bamboo|banana|banner|bar|barely|bargain|barrel|base|basic|basket|battle|beach|bean|beauty|because|become|beef|before|begin|behave|behind|believe|below|belt|bench|benefit|best|betray|better|between|beyond|bicycle|bid|bike|bind|biology|bird|birth|bitter|black|blade|blame|blanket|blast|bleak|bless|blind|blood|blossom|blouse|blue|blur|blush|board|boat|body|boil|bomb|bone|bonus|book|boost|border|boring|borrow|boss|bottom|bounce|box|boy|bracket|brain|brand|brass|brave|bread|breeze|brick|bridge|brief|bright|bring|brisk|broccoli|broken|bronze|broom|brother|brown|brush|bubble|buddy|budget|buffalo|build|bulb|bulk|bullet|bundle|bunker|burden|burger|burst|bus|business|busy|butter|buyer|buzz|cabbage|cabin|cable|cactus|cage|cake|call|calm|camera|camp|can|canal|cancel|candy|cannon|canoe|canvas|canyon|capable|capital|captain|car|carbon|card|cargo|carpet|carry|cart|case|cash|casino|castle|casual|cat|catalog|catch|category|cattle|caught|cause|caution|cave|ceiling|celery|cement|census|century|cereal|certain|chair|chalk|champion|change|chaos|chapter|charge|chase|chat|cheap|check|cheese|chef|cherry|chest|chicken|chief|child|chimney|choice|choose|chronic|chuckle|chunk|churn|cigar|cinnamon|circle|citizen|city|civil|claim|clap|clarify|claw|clay|clean|clerk|clever|click|client|cliff|climb|clinic|clip|clock|clog|close|cloth|cloud|clown|club|clump|cluster|clutch|coach|coast|coconut|code|coffee|coil|coin|collect|color|column|combine|come|comfort|comic|common|company|concert|conduct|confirm|congress|connect|consider|control|convince|cook|cool|copper|copy|coral|core|corn|correct|cost|cotton|couch|country|couple|course|cousin|cover|coyote|crack|cradle|craft|cram|crane|crash|crater|crawl|crazy|cream|credit|creek|crew|cricket|crime|crisp|critic|crop|cross|crouch|crowd|crucial|cruel|cruise|crumble|crunch|crush|cry|crystal|cube|culture|cup|cupboard|curious|current|curtain|curve|cushion|custom|cute|cycle|dad|damage|damp|dance|danger|daring|dash|daughter|dawn|day|deal|debate|debris|decade|december|decide|decline|decorate|decrease|deer|defense|define|defy|degree|delay|deliver|demand|demise|denial|dentist|deny|depart|depend|deposit|depth|deputy|derive|describe|desert|design|desk|despair|destroy|detail|detect|develop|device|devote|diagram|dial|diamond|diary|dice|diesel|diet|differ|digital|dignity|dilemma|dinner|dinosaur|direct|dirt|disagree|discover|disease|dish|dismiss|disorder|display|distance|divert|divide|divorce|dizzy|doctor|document|dog|doll|dolphin|domain|donate|donkey|donor|door|dose|double|dove|draft|dragon|drama|drastic|draw|dream|dress|drift|drill|drink|drip|drive|drop|drum|dry|duck|dumb|dune|during|dust|dutch|duty|dwarf|dynamic|eager|eagle|early|earn|earth|easily|east|easy|echo|ecology|economy|edge|edit|educate|effort|egg|eight|either|elbow|elder|electric|elegant|element|elephant|elevator|elite|else|embark|embody|embrace|emerge|emotion|employ|empower|empty|enable|enact|end|endless|endorse|enemy|energy|enforce|engage|engine|enhance|enjoy|enlist|enough|enrich|enroll|ensure|enter|entire|entry|envelope|episode|equal|equip|era|erase|erode|erosion|error|erupt|escape|essay|essence|estate|eternal|ethics|evidence|evil|evoke|evolve|exact|example|excess|exchange|excite|exclude|excuse|execute|exercise|exhaust|exhibit|exile|exist|exit|exotic|expand|expect|expire|explain|expose|express|extend|extra|eye|eyebrow|fabric|face|faculty|fade|faint|faith|fall|false|fame|family|famous|fan|fancy|fantasy|farm|fashion|fat|fatal|father|fatigue|fault|favorite|feature|february|federal|fee|feed|feel|female|fence|festival|fetch|fever|few|fiber|fiction|field|figure|file|film|filter|final|find|fine|finger|finish|fire|firm|first|fiscal|fish|fit|fitness|fix|flag|flame|flash|flat|flavor|flee|flight|flip|float|flock|floor|flower|fluid|flush|fly|foam|focus|fog|foil|fold|follow|food|foot|force|forest|forget|fork|fortune|forum|forward|fossil|foster|found|fox|fragile|frame|frequent|fresh|friend|fringe|frog|front|frost|frown|frozen|fruit|fuel|fun|funny|furnace|fury|future|gadget|gain|galaxy|gallery|game|gap|garage|garbage|garden|garlic|garment|gas|gasp|gate|gather|gauge|gaze|general|genius|genre|gentle|genuine|gesture|ghost|giant|gift|giggle|ginger|giraffe|girl|give|glad|glance|glare|glass|glide|glimpse|globe|gloom|glory|glove|glow|glue|goat|goddess|gold|good|goose|gorilla|gospel|gossip|govern|gown|grab|grace|grain|grant|grape|grass|gravity|great|green|grid|grief|grit|grocery|group|grow|grunt|guard|guess|guide|guilt|guitar|gun|gym|habit|hair|half|hammer|hamster|hand|happy|harbor|hard|harsh|harvest|hat|have|hawk|hazard|head|health|heart|heavy|hedgehog|height|hello|helmet|help|hen|hero|hidden|high|hill|hint|hip|hire|history|hobby|hockey|hold|hole|holiday|hollow|home|honey|hood|hope|horn|horror|horse|hospital|host|hotel|hour|hover|hub|huge|human|humble|humor|hundred|hungry|hunt|hurdle|hurry|hurt|husband|hybrid|ice|icon|idea|identify|idle|ignore|ill|illegal|illness|image|imitate|immense|immune|impact|impose|improve|impulse|inch|include|income|increase|index|indicate|indoor|industry|infant|inflict|inform|inhale|inherit|initial|inject|injury|inmate|inner|innocent|input|inquiry|insane|insect|inside|inspire|install|intact|interest|into|invest|invite|involve|iron|island|isolate|issue|item|ivory|jacket|jaguar|jar|jazz|jealous|jeans|jelly|jewel|job|join|joke|journey|joy|judge|juice|jump|jungle|junior|junk|just|kangaroo|keen|keep|ketchup|key|kick|kid|kidney|kind|kingdom|kiss|kit|kitchen|kite|kitten|kiwi|knee|knife|knock|know|lab|label|labor|ladder|lady|lake|lamp|language|laptop|large|later|latin|laugh|laundry|lava|law|lawn|lawsuit|layer|lazy|leader|leaf|learn|leave|lecture|left|leg|legal|legend|leisure|lemon|lend|length|lens|leopard|lesson|letter|level|liar|liberty|library|license|life|lift|light|like|limb|limit|link|lion|liquid|list|little|live|lizard|load|loan|lobster|local|lock|logic|lonely|long|loop|lottery|loud|lounge|love|loyal|lucky|luggage|lumber|lunar|lunch|luxury|lyrics|machine|mad|magic|magnet|maid|mail|main|major|make|mammal|man|manage|mandate|mango|mansion|manual|maple|marble|march|margin|marine|market|marriage|mask|mass|master|match|material|math|matrix|matter|maximum|maze|meadow|mean|measure|meat|mechanic|medal|media|melody|melt|member|memory|mention|menu|mercy|merge|merit|merry|mesh|message|metal|method|middle|midnight|milk|million|mimic|mind|minimum|minor|minute|miracle|mirror|misery|miss|mistake|mix|mixed|mixture|mobile|model|modify|mom|moment|monitor|monkey|monster|month|moon|moral|more|morning|mosquito|mother|motion|motor|mountain|mouse|move|movie|much|muffin|mule|multiply|muscle|museum|mushroom|music|must|mutual|myself|mystery|myth|naive|name|napkin|narrow|nasty|nation|nature|near|neck|need|negative|neglect|neither|nephew|nerve|nest|net|network|neutral|never|news|next|nice|night|noble|noise|nominee|noodle|normal|north|nose|notable|note|nothing|notice|novel|now|nuclear|number|nurse|nut|oak|obey|object|oblige|obscure|observe|obtain|obvious|occur|ocean|october|odor|off|offer|office|often|oil|okay|old|olive|olympic|omit|once|one|onion|online|only|open|opera|opinion|oppose|option|orange|orbit|orchard|order|ordinary|organ|orient|original|orphan|ostrich|other|outdoor|outer|output|outside|oval|oven|over|own|owner|oxygen|oyster|ozone|pact|paddle|page|pair|palace|palm|panda|panel|panic|panther|paper|parade|parent|park|parrot|party|pass|patch|path|patient|patrol|pattern|pause|pave|payment|peace|peanut|pear|peasant|pelican|pen|penalty|pencil|people|pepper|perfect|permit|person|pet|phone|photo|phrase|physical|piano|picnic|picture|piece|pig|pigeon|pill|pilot|pink|pioneer|pipe|pistol|pitch|pizza|place|planet|plastic|plate|play|please|pledge|pluck|plug|plunge|poem|poet|point|polar|pole|police|pond|pony|pool|popular|portion|position|possible|post|potato|pottery|poverty|powder|power|practice|praise|predict|prefer|prepare|present|pretty|prevent|price|pride|primary|print|priority|prison|private|prize|problem|process|produce|profit|program|project|promote|proof|property|prosper|protect|proud|provide|public|pudding|pull|pulp|pulse|pumpkin|punch|pupil|puppy|purchase|purity|purpose|purse|push|put|puzzle|pyramid|quality|quantum|quarter|question|quick|quit|quiz|quote|rabbit|raccoon|race|rack|radar|radio|rail|rain|raise|rally|ramp|ranch|random|range|rapid|rare|rate|rather|raven|raw|razor|ready|real|reason|rebel|rebuild|recall|receive|recipe|record|recycle|reduce|reflect|reform|refuse|region|regret|regular|reject|relax|release|relief|rely|remain|remember|remind|remove|render|renew|rent|reopen|repair|repeat|replace|report|require|rescue|resemble|resist|resource|response|result|retire|retreat|return|reunion|reveal|review|reward|rhythm|rib|ribbon|rice|rich|ride|ridge|rifle|right|rigid|ring|riot|ripple|risk|ritual|rival|river|road|roast|robot|robust|rocket|romance|roof|rookie|room|rose|rotate|rough|round|route|royal|rubber|rude|rug|rule|run|runway|rural|sad|saddle|sadness|safe|sail|salad|salmon|salon|salt|salute|same|sample|sand|satisfy|satoshi|sauce|sausage|save|say|scale|scan|scare|scatter|scene|scheme|school|science|scissors|scorpion|scout|scrap|screen|script|scrub|sea|search|season|seat|second|secret|section|security|seed|seek|segment|select|sell|seminar|senior|sense|sentence|series|service|session|settle|setup|seven|shadow|shaft|shallow|share|shed|shell|sheriff|shield|shift|shine|ship|shiver|shock|shoe|shoot|shop|short|shoulder|shove|shrimp|shrug|shuffle|shy|sibling|sick|side|siege|sight|sign|silent|silk|silly|silver|similar|simple|since|sing|siren|sister|situate|six|size|skate|sketch|ski|skill|skin|skirt|skull|slab|slam|sleep|slender|slice|slide|slight|slim|slogan|slot|slow|slush|small|smart|smile|smoke|smooth|snack|snake|snap|sniff|snow|soap|soccer|social|sock|soda|soft|solar|soldier|solid|solution|solve|someone|song|soon|sorry|sort|soul|sound|soup|source|south|space|spare|spatial|spawn|speak|special|speed|spell|spend|sphere|spice|spider|spike|spin|spirit|split|spoil|sponsor|spoon|sport|spot|spray|spread|spring|spy|square|squeeze|squirrel|stable|stadium|staff|stage|stairs|stamp|stand|start|state|stay|steak|steel|stem|step|stereo|stick|still|sting|stock|stomach|stone|stool|story|stove|strategy|street|strike|strong|struggle|student|stuff|stumble|style|subject|submit|subway|success|such|sudden|suffer|sugar|suggest|suit|summer|sun|sunny|sunset|super|supply|supreme|sure|surface|surge|surprise|surround|survey|suspect|sustain|swallow|swamp|swap|swarm|swear|sweet|swift|swim|swing|switch|sword|symbol|symptom|syrup|system|table|tackle|tag|tail|talent|talk|tank|tape|target|task|taste|tattoo|taxi|teach|team|tell|ten|tenant|tennis|tent|term|test|text|thank|that|theme|then|theory|there|they|thing|this|thought|three|thrive|throw|thumb|thunder|ticket|tide|tiger|tilt|timber|time|tiny|tip|tired|tissue|title|toast|tobacco|today|toddler|toe|together|toilet|token|tomato|tomorrow|tone|tongue|tonight|tool|tooth|top|topic|topple|torch|tornado|tortoise|toss|total|tourist|toward|tower|town|toy|track|trade|traffic|tragic|train|transfer|trap|trash|travel|tray|treat|tree|trend|trial|tribe|trick|trigger|trim|trip|trophy|trouble|truck|true|truly|trumpet|trust|truth|try|tube|tuition|tumble|tuna|tunnel|turkey|turn|turtle|twelve|twenty|twice|twin|twist|two|type|typical|ugly|umbrella|unable|unaware|uncle|uncover|under|undo|unfair|unfold|unhappy|uniform|unique|unit|universe|unknown|unlock|until|unusual|unveil|update|upgrade|uphold|upon|upper|upset|urban|urge|usage|use|used|useful|useless|usual|utility|vacant|vacuum|vague|valid|valley|valve|van|vanish|vapor|various|vast|vault|vehicle|velvet|vendor|venture|venue|verb|verify|version|very|vessel|veteran|viable|vibrant|vicious|victory|video|view|village|vintage|violin|virtual|virus|visa|visit|visual|vital|vivid|vocal|voice|void|volcano|volume|vote|voyage|wage|wagon|wait|walk|wall|walnut|want|warfare|warm|warrior|wash|wasp|waste|water|wave|way|wealth|weapon|wear|weasel|weather|web|wedding|weekend|weird|welcome|west|wet|whale|what|wheat|wheel|when|where|whip|whisper|wide|width|wife|wild|will|win|window|wine|wing|wink|winner|winter|wire|wisdom|wise|wish|witness|wolf|woman|wonder|wood|wool|word|work|world|worry|worth|wrap|wreck|wrestle|wrist|write|wrong|yard|year|yellow|you|young|youth|zebra|zero|zone|zoo'.split('|');

    const INVALID_MNEMONIC = 'Invalid mnemonic';
    const INVALID_ENTROPY = 'Invalid entropy';
    const INVALID_CHECKSUM = 'Invalid mnemonic checksum';
    function normalize(str) {
        return (str || '').normalize('NFKD');
    }
    function binaryToByte(bin) {
        return parseInt(bin, 2);
    }
    function bytesToBinary(bytes) {
        return bytes.map((x) => x.toString(2).padStart(8, '0')).join('');
    }
    function deriveChecksumBits(entropyBuffer) {
        return bytesToBinary(Array.from(sha256AsU8a(entropyBuffer))).slice(0, (entropyBuffer.length * 8) / 32);
    }
    function mnemonicToSeedSync(mnemonic, password) {
        return pbkdf2Encode(util.stringToU8a(normalize(mnemonic)), util.stringToU8a(`mnemonic${normalize(password)}`)).password;
    }
    function mnemonicToEntropy$1(mnemonic, wordlist = DEFAULT_WORDLIST) {
        const words = normalize(mnemonic).split(' ');
        if (words.length % 3 !== 0) {
            throw new Error(INVALID_MNEMONIC);
        }
        const bits = words
            .map((word) => {
            const index = wordlist.indexOf(word);
            if (index === -1) {
                throw new Error(INVALID_MNEMONIC);
            }
            return index.toString(2).padStart(11, '0');
        })
            .join('');
        const dividerIndex = Math.floor(bits.length / 33) * 32;
        const entropyBits = bits.slice(0, dividerIndex);
        const checksumBits = bits.slice(dividerIndex);
        const matched = entropyBits.match(/(.{1,8})/g);
        const entropyBytes = matched?.map(binaryToByte);
        if (!entropyBytes || (entropyBytes.length % 4 !== 0) || (entropyBytes.length < 16) || (entropyBytes.length > 32)) {
            throw new Error(INVALID_ENTROPY);
        }
        const entropy = util.u8aToU8a(entropyBytes);
        if (deriveChecksumBits(entropy) !== checksumBits) {
            throw new Error(INVALID_CHECKSUM);
        }
        return entropy;
    }
    function entropyToMnemonic(entropy, wordlist = DEFAULT_WORDLIST) {
        if ((entropy.length % 4 !== 0) || (entropy.length < 16) || (entropy.length > 32)) {
            throw new Error(INVALID_ENTROPY);
        }
        const matched = `${bytesToBinary(Array.from(entropy))}${deriveChecksumBits(entropy)}`.match(/(.{1,11})/g);
        const mapped = matched?.map((b) => wordlist[binaryToByte(b)]);
        if (!mapped || (mapped.length < 12)) {
            throw new Error('Unable to map entropy to mnemonic');
        }
        return mapped.join(' ');
    }
    function generateMnemonic(numWords, wordlist) {
        return entropyToMnemonic(randomAsU8a((numWords / 3) * 4), wordlist);
    }
    function validateMnemonic(mnemonic, wordlist) {
        try {
            mnemonicToEntropy$1(mnemonic, wordlist);
        }
        catch {
            return false;
        }
        return true;
    }

    function mnemonicGenerate(numWords = 12, wordlist, onlyJs) {
        return !util.hasBigInt || (!wordlist && !onlyJs && isReady())
            ? bip39Generate(numWords)
            : generateMnemonic(numWords, wordlist);
    }

    function mnemonicToEntropy(mnemonic, wordlist, onlyJs) {
        return !util.hasBigInt || (!wordlist && !onlyJs && isReady())
            ? bip39ToEntropy(mnemonic)
            : mnemonicToEntropy$1(mnemonic, wordlist);
    }

    function mnemonicValidate(mnemonic, wordlist, onlyJs) {
        return !util.hasBigInt || (!wordlist && !onlyJs && isReady())
            ? bip39Validate(mnemonic)
            : validateMnemonic(mnemonic, wordlist);
    }

    function mnemonicToLegacySeed(mnemonic, password = '', onlyJs, byteLength = 32) {
        if (!mnemonicValidate(mnemonic)) {
            throw new Error('Invalid bip39 mnemonic specified');
        }
        else if (![32, 64].includes(byteLength)) {
            throw new Error(`Invalid seed length ${byteLength}, expected 32 or 64`);
        }
        return byteLength === 32
            ? !util.hasBigInt || (!onlyJs && isReady())
                ? bip39ToSeed(mnemonic, password)
                : mnemonicToSeedSync(mnemonic, password).subarray(0, 32)
            : mnemonicToSeedSync(mnemonic, password);
    }

    function mnemonicToMiniSecret(mnemonic, password = '', wordlist, onlyJs) {
        if (!mnemonicValidate(mnemonic, wordlist, onlyJs)) {
            throw new Error('Invalid bip39 mnemonic specified');
        }
        else if (!wordlist && !onlyJs && isReady()) {
            return bip39ToMiniSecret(mnemonic, password);
        }
        const entropy = mnemonicToEntropy(mnemonic, wordlist);
        const salt = util.stringToU8a(`mnemonic${password}`);
        return pbkdf2Encode(entropy, salt).password.slice(0, 32);
    }

    function ledgerDerivePrivate(xprv, index) {
        const kl = xprv.subarray(0, 32);
        const kr = xprv.subarray(32, 64);
        const cc = xprv.subarray(64, 96);
        const data = util.u8aConcat([0], kl, kr, util.bnToU8a(index, BN_LE_32_OPTS));
        const z = hmacShaAsU8a(cc, data, 512);
        data[0] = 0x01;
        return util.u8aConcat(util.bnToU8a(util.u8aToBn(kl, BN_LE_OPTS).iadd(util.u8aToBn(z.subarray(0, 28), BN_LE_OPTS).imul(util.BN_EIGHT)), BN_LE_512_OPTS).subarray(0, 32), util.bnToU8a(util.u8aToBn(kr, BN_LE_OPTS).iadd(util.u8aToBn(z.subarray(32, 64), BN_LE_OPTS)), BN_LE_512_OPTS).subarray(0, 32), hmacShaAsU8a(cc, data, 512).subarray(32, 64));
    }

    const ED25519_CRYPTO = 'ed25519 seed';
    function ledgerMaster(mnemonic, password) {
        const seed = mnemonicToSeedSync(mnemonic, password);
        const chainCode = hmacShaAsU8a(ED25519_CRYPTO, new Uint8Array([1, ...seed]), 256);
        let priv;
        while (!priv || (priv[31] & 0b0010_0000)) {
            priv = hmacShaAsU8a(ED25519_CRYPTO, priv || seed, 512);
        }
        priv[0] &= 0b1111_1000;
        priv[31] &= 0b0111_1111;
        priv[31] |= 0b0100_0000;
        return util.u8aConcat(priv, chainCode);
    }

    function hdLedger(_mnemonic, path) {
        const words = _mnemonic
            .split(' ')
            .map((s) => s.trim())
            .filter((s) => s);
        if (![12, 24, 25].includes(words.length)) {
            throw new Error('Expected a mnemonic with 24 words (or 25 including a password)');
        }
        const [mnemonic, password] = words.length === 25
            ? [words.slice(0, 24).join(' '), words[24]]
            : [words.join(' '), ''];
        if (!mnemonicValidate(mnemonic)) {
            throw new Error('Invalid mnemonic passed to ledger derivation');
        }
        else if (!hdValidatePath(path)) {
            throw new Error('Invalid derivation path');
        }
        const parts = path.split('/').slice(1);
        let seed = ledgerMaster(mnemonic, password);
        for (const p of parts) {
            const n = parseInt(p.replace(/'$/, ''), 10);
            seed = ledgerDerivePrivate(seed, (n < HARDENED) ? (n + HARDENED) : n);
        }
        return ed25519PairFromSeed(seed.slice(0, 32));
    }

    function L32(x, c) { return (x << c) | (x >>> (32 - c)); }
    function ld32(x, i) {
        let u = x[i + 3] & 0xff;
        u = (u << 8) | (x[i + 2] & 0xff);
        u = (u << 8) | (x[i + 1] & 0xff);
        return (u << 8) | (x[i + 0] & 0xff);
    }
    function st32(x, j, u) {
        for (let i = 0; i < 4; i++) {
            x[j + i] = u & 255;
            u >>>= 8;
        }
    }
    function vn(x, xi, y, yi, n) {
        let d = 0;
        for (let i = 0; i < n; i++)
            d |= x[xi + i] ^ y[yi + i];
        return (1 & ((d - 1) >>> 8)) - 1;
    }
    function core(out, inp, k, c, h) {
        const w = new Uint32Array(16), x = new Uint32Array(16), y = new Uint32Array(16), t = new Uint32Array(4);
        let i, j, m;
        for (i = 0; i < 4; i++) {
            x[5 * i] = ld32(c, 4 * i);
            x[1 + i] = ld32(k, 4 * i);
            x[6 + i] = ld32(inp, 4 * i);
            x[11 + i] = ld32(k, 16 + 4 * i);
        }
        for (i = 0; i < 16; i++)
            y[i] = x[i];
        for (i = 0; i < 20; i++) {
            for (j = 0; j < 4; j++) {
                for (m = 0; m < 4; m++)
                    t[m] = x[(5 * j + 4 * m) % 16];
                t[1] ^= L32((t[0] + t[3]) | 0, 7);
                t[2] ^= L32((t[1] + t[0]) | 0, 9);
                t[3] ^= L32((t[2] + t[1]) | 0, 13);
                t[0] ^= L32((t[3] + t[2]) | 0, 18);
                for (m = 0; m < 4; m++)
                    w[4 * j + (j + m) % 4] = t[m];
            }
            for (m = 0; m < 16; m++)
                x[m] = w[m];
        }
        if (h) {
            for (i = 0; i < 16; i++)
                x[i] = (x[i] + y[i]) | 0;
            for (i = 0; i < 4; i++) {
                x[5 * i] = (x[5 * i] - ld32(c, 4 * i)) | 0;
                x[6 + i] = (x[6 + i] - ld32(inp, 4 * i)) | 0;
            }
            for (i = 0; i < 4; i++) {
                st32(out, 4 * i, x[5 * i]);
                st32(out, 16 + 4 * i, x[6 + i]);
            }
        }
        else {
            for (i = 0; i < 16; i++)
                st32(out, 4 * i, (x[i] + y[i]) | 0);
        }
    }
    const sigma = new Uint8Array([101, 120, 112, 97, 110, 100, 32, 51, 50, 45, 98, 121, 116, 101, 32, 107]);
    function crypto_stream_salsa20_xor(c, cpos, m, mpos, b, n, k) {
        const z = new Uint8Array(16), x = new Uint8Array(64);
        let u, i;
        if (!b)
            return 0;
        for (i = 0; i < 16; i++)
            z[i] = 0;
        for (i = 0; i < 8; i++)
            z[i] = n[i];
        while (b >= 64) {
            core(x, z, k, sigma, false);
            for (i = 0; i < 64; i++)
                c[cpos + i] = (m ? m[mpos + i] : 0) ^ x[i];
            u = 1;
            for (i = 8; i < 16; i++) {
                u = u + (z[i] & 0xff) | 0;
                z[i] = u & 0xff;
                u >>>= 8;
            }
            b -= 64;
            cpos += 64;
            if (m)
                mpos += 64;
        }
        if (b > 0) {
            core(x, z, k, sigma, false);
            for (i = 0; i < b; i++)
                c[cpos + i] = (m ? m[mpos + i] : 0) ^ x[i];
        }
        return 0;
    }
    function crypto_stream_xor(c, cpos, m, mpos, d, n, k) {
        const s = new Uint8Array(32);
        core(s, n, k, sigma, true);
        return crypto_stream_salsa20_xor(c, cpos, m, mpos, d, n.subarray(16), s);
    }
    function add1305(h, c) {
        let u = 0;
        for (let j = 0; j < 17; j++) {
            u = (u + ((h[j] + c[j]) | 0)) | 0;
            h[j] = u & 255;
            u >>>= 8;
        }
    }
    const minusp = new Uint32Array([5, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 252]);
    function crypto_onetimeauth(out, outpos, m, mpos, n, k) {
        let i, j, u;
        const x = new Uint32Array(17), r = new Uint32Array(17), h = new Uint32Array(17), c = new Uint32Array(17), g = new Uint32Array(17);
        for (j = 0; j < 17; j++)
            r[j] = h[j] = 0;
        for (j = 0; j < 16; j++)
            r[j] = k[j];
        r[3] &= 15;
        r[4] &= 252;
        r[7] &= 15;
        r[8] &= 252;
        r[11] &= 15;
        r[12] &= 252;
        r[15] &= 15;
        while (n > 0) {
            for (j = 0; j < 17; j++)
                c[j] = 0;
            for (j = 0; (j < 16) && (j < n); ++j)
                c[j] = m[mpos + j];
            c[j] = 1;
            mpos += j;
            n -= j;
            add1305(h, c);
            for (i = 0; i < 17; i++) {
                x[i] = 0;
                for (j = 0; j < 17; j++)
                    x[i] = (x[i] + (h[j] * ((j <= i) ? r[i - j] : ((320 * r[i + 17 - j]) | 0))) | 0) | 0;
            }
            for (i = 0; i < 17; i++)
                h[i] = x[i];
            u = 0;
            for (j = 0; j < 16; j++) {
                u = (u + h[j]) | 0;
                h[j] = u & 255;
                u >>>= 8;
            }
            u = (u + h[16]) | 0;
            h[16] = u & 3;
            u = (5 * (u >>> 2)) | 0;
            for (j = 0; j < 16; j++) {
                u = (u + h[j]) | 0;
                h[j] = u & 255;
                u >>>= 8;
            }
            u = (u + h[16]) | 0;
            h[16] = u;
        }
        for (j = 0; j < 17; j++)
            g[j] = h[j];
        add1305(h, minusp);
        const s = (-(h[16] >>> 7) | 0);
        for (j = 0; j < 17; j++)
            h[j] ^= s & (g[j] ^ h[j]);
        for (j = 0; j < 16; j++)
            c[j] = k[j + 16];
        c[16] = 0;
        add1305(h, c);
        for (j = 0; j < 16; j++)
            out[outpos + j] = h[j];
        return 0;
    }
    function crypto_onetimeauth_verify(h, hpos, m, mpos, n, k) {
        const x = new Uint8Array(16);
        crypto_onetimeauth(x, 0, m, mpos, n, k);
        return vn(h, hpos, x, 0, 16);
    }
    function crypto_secretbox(c, m, d, n, k) {
        if (d < 32)
            return -1;
        crypto_stream_xor(c, 0, m, 0, d, n, k);
        crypto_onetimeauth(c, 16, c, 32, d - 32, c);
        for (let i = 0; i < 16; i++)
            c[i] = 0;
        return 0;
    }
    function crypto_secretbox_open(m, c, d, n, k) {
        const x = new Uint8Array(32);
        if (d < 32)
            return -1;
        crypto_stream_xor(x, 0, null, 0, 32, n, k);
        if (crypto_onetimeauth_verify(c, 16, c, 32, d - 32, x) !== 0)
            return -1;
        crypto_stream_xor(m, 0, c, 0, d, n, k);
        for (let i = 0; i < 32; i++)
            m[i] = 0;
        return 0;
    }
    const crypto_secretbox_KEYBYTES = 32;
    const crypto_secretbox_NONCEBYTES = 24;
    const crypto_secretbox_ZEROBYTES = 32;
    const crypto_secretbox_BOXZEROBYTES = 16;
    function checkLengths(k, n) {
        if (k.length !== crypto_secretbox_KEYBYTES)
            throw new Error('bad key size');
        if (n.length !== crypto_secretbox_NONCEBYTES)
            throw new Error('bad nonce size');
    }
    function checkArrayTypes(...args) {
        for (let i = 0, count = args.length; i < count; i++) {
            if (!(args[i] instanceof Uint8Array))
                throw new TypeError('unexpected type, use Uint8Array');
        }
    }
    function naclSecretbox(msg, nonce, key) {
        checkArrayTypes(msg, nonce, key);
        checkLengths(key, nonce);
        const m = new Uint8Array(crypto_secretbox_ZEROBYTES + msg.length);
        const c = new Uint8Array(m.length);
        for (let i = 0; i < msg.length; i++)
            m[i + crypto_secretbox_ZEROBYTES] = msg[i];
        crypto_secretbox(c, m, m.length, nonce, key);
        return c.subarray(crypto_secretbox_BOXZEROBYTES);
    }
    function naclSecretboxOpen(box, nonce, key) {
        checkArrayTypes(box, nonce, key);
        checkLengths(key, nonce);
        const c = new Uint8Array(crypto_secretbox_BOXZEROBYTES + box.length);
        const m = new Uint8Array(c.length);
        for (let i = 0; i < box.length; i++)
            c[i + crypto_secretbox_BOXZEROBYTES] = box[i];
        if (c.length < 32)
            return null;
        if (crypto_secretbox_open(m, c, c.length, nonce, key) !== 0)
            return null;
        return m.subarray(crypto_secretbox_ZEROBYTES);
    }

    function naclDecrypt(encrypted, nonce, secret) {
        return naclSecretboxOpen(encrypted, nonce, secret);
    }

    function naclEncrypt(message, secret, nonce = randomAsU8a(24)) {
        return {
            encrypted: naclSecretbox(message, nonce, secret),
            nonce
        };
    }

    const rotl$1 = (a, b) => (a << b) | (a >>> (32 - b));
    function XorAndSalsa(prev, pi, input, ii, out, oi) {
        let y00 = prev[pi++] ^ input[ii++], y01 = prev[pi++] ^ input[ii++];
        let y02 = prev[pi++] ^ input[ii++], y03 = prev[pi++] ^ input[ii++];
        let y04 = prev[pi++] ^ input[ii++], y05 = prev[pi++] ^ input[ii++];
        let y06 = prev[pi++] ^ input[ii++], y07 = prev[pi++] ^ input[ii++];
        let y08 = prev[pi++] ^ input[ii++], y09 = prev[pi++] ^ input[ii++];
        let y10 = prev[pi++] ^ input[ii++], y11 = prev[pi++] ^ input[ii++];
        let y12 = prev[pi++] ^ input[ii++], y13 = prev[pi++] ^ input[ii++];
        let y14 = prev[pi++] ^ input[ii++], y15 = prev[pi++] ^ input[ii++];
        let x00 = y00, x01 = y01, x02 = y02, x03 = y03, x04 = y04, x05 = y05, x06 = y06, x07 = y07, x08 = y08, x09 = y09, x10 = y10, x11 = y11, x12 = y12, x13 = y13, x14 = y14, x15 = y15;
        for (let i = 0; i < 8; i += 2) {
            x04 ^= rotl$1(x00 + x12 | 0, 7);
            x08 ^= rotl$1(x04 + x00 | 0, 9);
            x12 ^= rotl$1(x08 + x04 | 0, 13);
            x00 ^= rotl$1(x12 + x08 | 0, 18);
            x09 ^= rotl$1(x05 + x01 | 0, 7);
            x13 ^= rotl$1(x09 + x05 | 0, 9);
            x01 ^= rotl$1(x13 + x09 | 0, 13);
            x05 ^= rotl$1(x01 + x13 | 0, 18);
            x14 ^= rotl$1(x10 + x06 | 0, 7);
            x02 ^= rotl$1(x14 + x10 | 0, 9);
            x06 ^= rotl$1(x02 + x14 | 0, 13);
            x10 ^= rotl$1(x06 + x02 | 0, 18);
            x03 ^= rotl$1(x15 + x11 | 0, 7);
            x07 ^= rotl$1(x03 + x15 | 0, 9);
            x11 ^= rotl$1(x07 + x03 | 0, 13);
            x15 ^= rotl$1(x11 + x07 | 0, 18);
            x01 ^= rotl$1(x00 + x03 | 0, 7);
            x02 ^= rotl$1(x01 + x00 | 0, 9);
            x03 ^= rotl$1(x02 + x01 | 0, 13);
            x00 ^= rotl$1(x03 + x02 | 0, 18);
            x06 ^= rotl$1(x05 + x04 | 0, 7);
            x07 ^= rotl$1(x06 + x05 | 0, 9);
            x04 ^= rotl$1(x07 + x06 | 0, 13);
            x05 ^= rotl$1(x04 + x07 | 0, 18);
            x11 ^= rotl$1(x10 + x09 | 0, 7);
            x08 ^= rotl$1(x11 + x10 | 0, 9);
            x09 ^= rotl$1(x08 + x11 | 0, 13);
            x10 ^= rotl$1(x09 + x08 | 0, 18);
            x12 ^= rotl$1(x15 + x14 | 0, 7);
            x13 ^= rotl$1(x12 + x15 | 0, 9);
            x14 ^= rotl$1(x13 + x12 | 0, 13);
            x15 ^= rotl$1(x14 + x13 | 0, 18);
        }
        out[oi++] = (y00 + x00) | 0;
        out[oi++] = (y01 + x01) | 0;
        out[oi++] = (y02 + x02) | 0;
        out[oi++] = (y03 + x03) | 0;
        out[oi++] = (y04 + x04) | 0;
        out[oi++] = (y05 + x05) | 0;
        out[oi++] = (y06 + x06) | 0;
        out[oi++] = (y07 + x07) | 0;
        out[oi++] = (y08 + x08) | 0;
        out[oi++] = (y09 + x09) | 0;
        out[oi++] = (y10 + x10) | 0;
        out[oi++] = (y11 + x11) | 0;
        out[oi++] = (y12 + x12) | 0;
        out[oi++] = (y13 + x13) | 0;
        out[oi++] = (y14 + x14) | 0;
        out[oi++] = (y15 + x15) | 0;
    }
    function BlockMix(input, ii, out, oi, r) {
        let head = oi + 0;
        let tail = oi + 16 * r;
        for (let i = 0; i < 16; i++)
            out[tail + i] = input[ii + (2 * r - 1) * 16 + i];
        for (let i = 0; i < r; i++, head += 16, ii += 16) {
            XorAndSalsa(out, tail, input, ii, out, head);
            if (i > 0)
                tail += 16;
            XorAndSalsa(out, head, input, (ii += 16), out, tail);
        }
    }
    function scryptInit(password, salt, _opts) {
        const opts = checkOpts({
            dkLen: 32,
            asyncTick: 10,
            maxmem: 1024 ** 3 + 1024,
        }, _opts);
        const { N, r, p, dkLen, asyncTick, maxmem, onProgress } = opts;
        number(N);
        number(r);
        number(p);
        number(dkLen);
        number(asyncTick);
        number(maxmem);
        if (onProgress !== undefined && typeof onProgress !== 'function')
            throw new Error('progressCb should be function');
        const blockSize = 128 * r;
        const blockSize32 = blockSize / 4;
        if (N <= 1 || (N & (N - 1)) !== 0 || N >= 2 ** (blockSize / 8) || N > 2 ** 32) {
            throw new Error('Scrypt: N must be larger than 1, a power of 2, less than 2^(128 * r / 8) and less than 2^32');
        }
        if (p < 0 || p > ((2 ** 32 - 1) * 32) / blockSize) {
            throw new Error('Scrypt: p must be a positive integer less than or equal to ((2^32 - 1) * 32) / (128 * r)');
        }
        if (dkLen < 0 || dkLen > (2 ** 32 - 1) * 32) {
            throw new Error('Scrypt: dkLen should be positive integer less than or equal to (2^32 - 1) * 32');
        }
        const memUsed = blockSize * (N + p);
        if (memUsed > maxmem) {
            throw new Error(`Scrypt: parameters too large, ${memUsed} (128 * r * (N + p)) > ${maxmem} (maxmem)`);
        }
        const B = pbkdf2(sha256, password, salt, { c: 1, dkLen: blockSize * p });
        const B32 = u32$1(B);
        const V = u32$1(new Uint8Array(blockSize * N));
        const tmp = u32$1(new Uint8Array(blockSize));
        let blockMixCb = () => { };
        if (onProgress) {
            const totalBlockMix = 2 * N * p;
            const callbackPer = Math.max(Math.floor(totalBlockMix / 10000), 1);
            let blockMixCnt = 0;
            blockMixCb = () => {
                blockMixCnt++;
                if (onProgress && (!(blockMixCnt % callbackPer) || blockMixCnt === totalBlockMix))
                    onProgress(blockMixCnt / totalBlockMix);
            };
        }
        return { N, r, p, dkLen, blockSize32, V, B32, B, tmp, blockMixCb, asyncTick };
    }
    function scryptOutput(password, dkLen, B, V, tmp) {
        const res = pbkdf2(sha256, password, B, { c: 1, dkLen });
        B.fill(0);
        V.fill(0);
        tmp.fill(0);
        return res;
    }
    function scrypt(password, salt, opts) {
        const { N, r, p, dkLen, blockSize32, V, B32, B, tmp, blockMixCb } = scryptInit(password, salt, opts);
        for (let pi = 0; pi < p; pi++) {
            const Pi = blockSize32 * pi;
            for (let i = 0; i < blockSize32; i++)
                V[i] = B32[Pi + i];
            for (let i = 0, pos = 0; i < N - 1; i++) {
                BlockMix(V, pos, V, (pos += blockSize32), r);
                blockMixCb();
            }
            BlockMix(V, (N - 1) * blockSize32, B32, Pi, r);
            blockMixCb();
            for (let i = 0; i < N; i++) {
                const j = B32[Pi + blockSize32 - 16] % N;
                for (let k = 0; k < blockSize32; k++)
                    tmp[k] = B32[Pi + k] ^ V[j * blockSize32 + k];
                BlockMix(tmp, 0, B32, Pi, r);
                blockMixCb();
            }
        }
        return scryptOutput(password, dkLen, B, V, tmp);
    }

    const ALLOWED_PARAMS = [
        { N: 1 << 13, p: 10, r: 8 },
        { N: 1 << 14, p: 5, r: 8 },
        { N: 1 << 15, p: 3, r: 8 },
        { N: 1 << 15, p: 1, r: 8 },
        { N: 1 << 16, p: 2, r: 8 },
        { N: 1 << 17, p: 1, r: 8 }
    ];
    const DEFAULT_PARAMS = {
        N: 1 << 17,
        p: 1,
        r: 8
    };

    function scryptEncode(passphrase, salt = randomAsU8a(), params = DEFAULT_PARAMS, onlyJs) {
        const u8a = util.u8aToU8a(passphrase);
        return {
            params,
            password: !util.hasBigInt || (!onlyJs && isReady())
                ? scrypt$1(u8a, salt, Math.log2(params.N), params.r, params.p)
                : scrypt(u8a, salt, util.objectSpread({ dkLen: 64 }, params)),
            salt
        };
    }

    function scryptFromU8a(data) {
        if (!(data instanceof Uint8Array)) {
            throw new Error('Expected input to be a Uint8Array');
        }
        if (data.length < 32 + 12) {
            throw new Error(`Invalid input length: expected 44 bytes, found ${data.length}`);
        }
        const salt = data.subarray(0, 32);
        const N = util.u8aToBn(data.subarray(32, 36), BN_LE_OPTS).toNumber();
        const p = util.u8aToBn(data.subarray(36, 40), BN_LE_OPTS).toNumber();
        const r = util.u8aToBn(data.subarray(40, 44), BN_LE_OPTS).toNumber();
        if (N > (1 << 20) || p > 4 || r > 16) {
            throw new Error('Scrypt parameters exceed safe limits');
        }
        const isAllowed = ALLOWED_PARAMS.some((preset) => preset.N === N && preset.p === p && preset.r === r);
        if (!isAllowed) {
            throw new Error('Invalid injected scrypt params found');
        }
        return { params: { N, p, r }, salt };
    }

    function scryptToU8a(salt, { N, p, r }) {
        return util.u8aConcat(salt, util.bnToU8a(N, BN_LE_32_OPTS), util.bnToU8a(p, BN_LE_32_OPTS), util.bnToU8a(r, BN_LE_32_OPTS));
    }

    const ENCODING = ['scrypt', 'xsalsa20-poly1305'];
    const ENCODING_NONE = ['none'];
    const ENCODING_VERSION = '3';
    const NONCE_LENGTH = 24;
    const SCRYPT_LENGTH = 32 + (3 * 4);

    function jsonDecryptData(encrypted, passphrase, encType = ENCODING) {
        if (!encrypted) {
            throw new Error('No encrypted data available to decode');
        }
        else if (encType.includes('xsalsa20-poly1305') && !passphrase) {
            throw new Error('Password required to decode encrypted data');
        }
        let encoded = encrypted;
        if (passphrase) {
            let password;
            if (encType.includes('scrypt')) {
                const { params, salt } = scryptFromU8a(encrypted);
                password = scryptEncode(passphrase, salt, params).password;
                encrypted = encrypted.subarray(SCRYPT_LENGTH);
            }
            else {
                password = util.stringToU8a(passphrase);
            }
            encoded = naclDecrypt(encrypted.subarray(NONCE_LENGTH), encrypted.subarray(0, NONCE_LENGTH), util.u8aFixLength(password, 256, true));
        }
        if (!encoded) {
            throw new Error('Unable to decode using the supplied passphrase');
        }
        return encoded;
    }

    function jsonDecrypt({ encoded, encoding }, passphrase) {
        if (!encoded) {
            throw new Error('No encrypted data available to decode');
        }
        return jsonDecryptData(util.isHex(encoded)
            ? util.hexToU8a(encoded)
            : base64Decode(encoded), passphrase, Array.isArray(encoding.type)
            ? encoding.type
            : [encoding.type]);
    }

    function jsonEncryptFormat(encoded, contentType, isEncrypted) {
        return {
            encoded: base64Encode(encoded),
            encoding: {
                content: contentType,
                type: isEncrypted
                    ? ENCODING
                    : ENCODING_NONE,
                version: ENCODING_VERSION
            }
        };
    }

    function jsonEncrypt(data, contentType, passphrase) {
        let isEncrypted = false;
        let encoded = data;
        if (passphrase) {
            const { params, password, salt } = scryptEncode(passphrase);
            const { encrypted, nonce } = naclEncrypt(encoded, password.subarray(0, 32));
            isEncrypted = true;
            encoded = util.u8aConcat(scryptToU8a(salt, params), nonce, encrypted);
        }
        return jsonEncryptFormat(encoded, contentType, isEncrypted);
    }

    const secp256k1VerifyHasher = (hashType) => (message, signature, publicKey) => secp256k1Verify(message, signature, publicKey, hashType, true);
    const VERIFIERS_ECDSA = [
        ['ecdsa', secp256k1VerifyHasher('blake2')],
        ['ethereum', secp256k1VerifyHasher('keccak')]
    ];
    const VERIFIERS = [
        ['ed25519', ed25519Verify],
        ['sr25519', sr25519Verify]
    ];
    function verifyDetect(result, { message, publicKey, signature }, verifiers = [...VERIFIERS, ...VERIFIERS_ECDSA]) {
        result.isValid = verifiers.some(([crypto, verify]) => {
            try {
                if (verify(message, signature, publicKey)) {
                    result.crypto = crypto;
                    return true;
                }
            }
            catch {
            }
            return false;
        });
        return result;
    }
    function verifyMultisig(result, { message, publicKey, signature }) {
        if (![0, 1, 2].includes(signature[0]) || ![65, 66].includes(signature.length)) {
            throw new Error(`Unknown crypto type, expected signature prefix [0..2], found ${signature[0]}`);
        }
        if (signature.length === 66) {
            result = verifyDetect(result, { message, publicKey, signature: signature.subarray(1) }, VERIFIERS_ECDSA);
        }
        else {
            result = verifyDetect(result, { message, publicKey, signature: signature.subarray(1) }, VERIFIERS);
            if (!result.isValid) {
                result = verifyDetect(result, { message, publicKey, signature }, VERIFIERS_ECDSA);
            }
            if (!result.isValid) {
                result.crypto = 'none';
            }
        }
        return result;
    }
    function getVerifyFn(signature) {
        return [0, 1, 2].includes(signature[0]) && [65, 66].includes(signature.length)
            ? verifyMultisig
            : verifyDetect;
    }
    function signatureVerify(message, signature, addressOrPublicKey) {
        const signatureU8a = util.u8aToU8a(signature);
        if (![64, 65, 66].includes(signatureU8a.length)) {
            throw new Error(`Invalid signature length, expected [64..66] bytes, found ${signatureU8a.length}`);
        }
        const publicKey = decodeAddress(addressOrPublicKey);
        const input = { message: util.u8aToU8a(message), publicKey, signature: signatureU8a };
        const result = { crypto: 'none', isValid: false, isWrapped: util.u8aIsWrapped(input.message, true), publicKey };
        const isWrappedBytes = util.u8aIsWrapped(input.message, false);
        const verifyFn = getVerifyFn(signatureU8a);
        verifyFn(result, input);
        if (result.crypto !== 'none' || (result.isWrapped && !isWrappedBytes)) {
            return result;
        }
        input.message = isWrappedBytes
            ? util.u8aUnwrapBytes(input.message)
            : util.u8aWrapBytes(input.message);
        return verifyFn(result, input);
    }

    const P64_1 = BigInt$1('11400714785074694791');
    const P64_2 = BigInt$1('14029467366897019727');
    const P64_3 = BigInt$1('1609587929392839161');
    const P64_4 = BigInt$1('9650029242287828579');
    const P64_5 = BigInt$1('2870177450012600261');
    const U64 = BigInt$1('0xffffffffffffffff');
    const _7n = BigInt$1(7);
    const _11n = BigInt$1(11);
    const _12n = BigInt$1(12);
    const _16n = BigInt$1(16);
    const _18n = BigInt$1(18);
    const _23n = BigInt$1(23);
    const _27n = BigInt$1(27);
    const _29n = BigInt$1(29);
    const _31n = BigInt$1(31);
    const _32n = BigInt$1(32);
    const _33n = BigInt$1(33);
    const _64n = BigInt$1(64);
    const _256n = BigInt$1(256);
    function rotl(a, b) {
        const c = a & U64;
        return ((c << b) | (c >> (_64n - b))) & U64;
    }
    function fromU8a(u8a, p, count) {
        const bigints = new Array(count);
        let offset = 0;
        for (let i = 0; i < count; i++, offset += 2) {
            bigints[i] = BigInt$1(u8a[p + offset] | (u8a[p + 1 + offset] << 8));
        }
        let result = util._0n;
        for (let i = count - 1; i >= 0; i--) {
            result = (result << _16n) + bigints[i];
        }
        return result;
    }
    function init(seed, input) {
        const state = {
            seed,
            u8a: new Uint8Array(32),
            u8asize: 0,
            v1: seed + P64_1 + P64_2,
            v2: seed + P64_2,
            v3: seed,
            v4: seed - P64_1
        };
        if (input.length < 32) {
            state.u8a.set(input);
            state.u8asize = input.length;
            return state;
        }
        const limit = input.length - 32;
        let p = 0;
        if (limit >= 0) {
            const adjustV = (v) => P64_1 * rotl(v + P64_2 * fromU8a(input, p, 4), _31n);
            do {
                state.v1 = adjustV(state.v1);
                p += 8;
                state.v2 = adjustV(state.v2);
                p += 8;
                state.v3 = adjustV(state.v3);
                p += 8;
                state.v4 = adjustV(state.v4);
                p += 8;
            } while (p <= limit);
        }
        if (p < input.length) {
            state.u8a.set(input.subarray(p, input.length));
            state.u8asize = input.length - p;
        }
        return state;
    }
    function xxhash64(input, initSeed) {
        const { seed, u8a, u8asize, v1, v2, v3, v4 } = init(BigInt$1(initSeed), input);
        let p = 0;
        let h64 = U64 & (BigInt$1(input.length) + (input.length >= 32
            ? (((((((((rotl(v1, util._1n) + rotl(v2, _7n) + rotl(v3, _12n) + rotl(v4, _18n)) ^ (P64_1 * rotl(v1 * P64_2, _31n))) * P64_1 + P64_4) ^ (P64_1 * rotl(v2 * P64_2, _31n))) * P64_1 + P64_4) ^ (P64_1 * rotl(v3 * P64_2, _31n))) * P64_1 + P64_4) ^ (P64_1 * rotl(v4 * P64_2, _31n))) * P64_1 + P64_4)
            : (seed + P64_5)));
        while (p <= (u8asize - 8)) {
            h64 = U64 & (P64_4 + P64_1 * rotl(h64 ^ (P64_1 * rotl(P64_2 * fromU8a(u8a, p, 4), _31n)), _27n));
            p += 8;
        }
        if ((p + 4) <= u8asize) {
            h64 = U64 & (P64_3 + P64_2 * rotl(h64 ^ (P64_1 * fromU8a(u8a, p, 2)), _23n));
            p += 4;
        }
        while (p < u8asize) {
            h64 = U64 & (P64_1 * rotl(h64 ^ (P64_5 * BigInt$1(u8a[p++])), _11n));
        }
        h64 = U64 & (P64_2 * (h64 ^ (h64 >> _33n)));
        h64 = U64 & (P64_3 * (h64 ^ (h64 >> _29n)));
        h64 = U64 & (h64 ^ (h64 >> _32n));
        const result = new Uint8Array(8);
        for (let i = 7; i >= 0; i--) {
            result[i] = Number(h64 % _256n);
            h64 = h64 / _256n;
        }
        return result;
    }

    function xxhashAsU8a(data, bitLength = 64, onlyJs) {
        const rounds = Math.ceil(bitLength / 64);
        const u8a = util.u8aToU8a(data);
        if (!util.hasBigInt || (!onlyJs && isReady())) {
            return twox(u8a, rounds);
        }
        const result = new Uint8Array(rounds * 8);
        for (let seed = 0; seed < rounds; seed++) {
            result.set(xxhash64(u8a, seed).reverse(), seed * 8);
        }
        return result;
    }
    const xxhashAsHex =  createAsHex(xxhashAsU8a);

    exports.addressEq = addressEq;
    exports.addressToEvm = addressToEvm;
    exports.allNetworks = allNetworks;
    exports.availableNetworks = availableNetworks;
    exports.base32Decode = base32Decode;
    exports.base32Encode = base32Encode;
    exports.base32Validate = base32Validate;
    exports.base58Decode = base58Decode;
    exports.base58Encode = base58Encode;
    exports.base58Validate = base58Validate;
    exports.base64Decode = base64Decode;
    exports.base64Encode = base64Encode;
    exports.base64Pad = base64Pad;
    exports.base64Trim = base64Trim;
    exports.base64Validate = base64Validate;
    exports.blake2AsHex = blake2AsHex;
    exports.blake2AsU8a = blake2AsU8a;
    exports.checkAddress = checkAddress;
    exports.checkAddressChecksum = checkAddressChecksum;
    exports.createKeyDerived = createKeyDerived;
    exports.createKeyMulti = createKeyMulti;
    exports.cryptoIsReady = cryptoIsReady;
    exports.cryptoWaitReady = cryptoWaitReady;
    exports.decodeAddress = decodeAddress;
    exports.deriveAddress = deriveAddress;
    exports.ed25519DeriveHard = ed25519DeriveHard;
    exports.ed25519PairFromRandom = ed25519PairFromRandom;
    exports.ed25519PairFromSecret = ed25519PairFromSecret;
    exports.ed25519PairFromSeed = ed25519PairFromSeed;
    exports.ed25519PairFromString = ed25519PairFromString;
    exports.ed25519Sign = ed25519Sign;
    exports.ed25519Verify = ed25519Verify;
    exports.encodeAddress = encodeAddress;
    exports.encodeDerivedAddress = encodeDerivedAddress;
    exports.encodeMultiAddress = encodeMultiAddress;
    exports.ethereumEncode = ethereumEncode;
    exports.evmToAddress = evmToAddress;
    exports.hdEthereum = hdEthereum;
    exports.hdLedger = hdLedger;
    exports.hdValidatePath = hdValidatePath;
    exports.hmacSha256AsU8a = hmacSha256AsU8a;
    exports.hmacSha512AsU8a = hmacSha512AsU8a;
    exports.hmacShaAsU8a = hmacShaAsU8a;
    exports.isAddress = isAddress;
    exports.isBase32 = isBase32;
    exports.isBase58 = isBase58;
    exports.isBase64 = isBase64;
    exports.isEthereumAddress = isEthereumAddress;
    exports.isEthereumChecksum = isEthereumChecksum;
    exports.jsonDecrypt = jsonDecrypt;
    exports.jsonDecryptData = jsonDecryptData;
    exports.jsonEncrypt = jsonEncrypt;
    exports.jsonEncryptFormat = jsonEncryptFormat;
    exports.keccak256AsU8a = keccak256AsU8a;
    exports.keccak512AsU8a = keccak512AsU8a;
    exports.keccakAsHex = keccakAsHex;
    exports.keccakAsU8a = keccakAsU8a;
    exports.keyExtractPath = keyExtractPath;
    exports.keyExtractSuri = keyExtractSuri;
    exports.keyFromPath = keyFromPath;
    exports.keyHdkdEcdsa = keyHdkdEcdsa;
    exports.keyHdkdEd25519 = keyHdkdEd25519;
    exports.keyHdkdSr25519 = keyHdkdSr25519;
    exports.mnemonicGenerate = mnemonicGenerate;
    exports.mnemonicToEntropy = mnemonicToEntropy;
    exports.mnemonicToLegacySeed = mnemonicToLegacySeed;
    exports.mnemonicToMiniSecret = mnemonicToMiniSecret;
    exports.mnemonicValidate = mnemonicValidate;
    exports.naclDecrypt = naclDecrypt;
    exports.naclEncrypt = naclEncrypt;
    exports.packageInfo = packageInfo;
    exports.pbkdf2Encode = pbkdf2Encode;
    exports.randomAsHex = randomAsHex;
    exports.randomAsNumber = randomAsNumber;
    exports.randomAsU8a = randomAsU8a;
    exports.scryptEncode = scryptEncode;
    exports.scryptFromU8a = scryptFromU8a;
    exports.scryptToU8a = scryptToU8a;
    exports.secp256k1Compress = secp256k1Compress;
    exports.secp256k1Expand = secp256k1Expand;
    exports.secp256k1PairFromSeed = secp256k1PairFromSeed;
    exports.secp256k1PrivateKeyTweakAdd = secp256k1PrivateKeyTweakAdd;
    exports.secp256k1Recover = secp256k1Recover;
    exports.secp256k1Sign = secp256k1Sign;
    exports.secp256k1Verify = secp256k1Verify;
    exports.selectableNetworks = selectableNetworks;
    exports.setSS58Format = setSS58Format;
    exports.sha256AsU8a = sha256AsU8a;
    exports.sha512AsU8a = sha512AsU8a;
    exports.shaAsU8a = shaAsU8a;
    exports.signatureVerify = signatureVerify;
    exports.sortAddresses = sortAddresses;
    exports.sr25519Agreement = sr25519Agreement;
    exports.sr25519DeriveHard = sr25519DeriveHard;
    exports.sr25519DerivePublic = sr25519DerivePublic;
    exports.sr25519DeriveSoft = sr25519DeriveSoft;
    exports.sr25519PairFromSeed = sr25519PairFromSeed;
    exports.sr25519Sign = sr25519Sign;
    exports.sr25519Verify = sr25519Verify;
    exports.sr25519VrfSign = sr25519VrfSign;
    exports.sr25519VrfVerify = sr25519VrfVerify;
    exports.validateAddress = validateAddress;
    exports.xxhashAsHex = xxhashAsHex;
    exports.xxhashAsU8a = xxhashAsU8a;

}));
