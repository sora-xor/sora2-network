;; Source for the 144-byte test fixture beside this file. No signing or networking.
(module
  (import "env" "ext_storage_exists_version_1" (func $exists (param i64) (result i32)))
  (memory (export "memory") 2)
  (global (export "__heap_base") i32 (i32.const 65536))
  (func (export "probe") (param $ptr i32) (param $len i32) (result i64)
    (i32.store (i32.const 1024)
      (call $exists
        (i64.or (i64.extend_i32_u (local.get $ptr))
          (i64.shl (i64.extend_i32_u (local.get $len)) (i64.const 32)))))
    (i64.const 17179870208)))
