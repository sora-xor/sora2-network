;; Fixed protocol smoke fixture: Core_version returns SCALE RuntimeVersion;
;; Fixture_add stores the result of 40 + 2 and returns its packed pointer/length.
(module
 (memory (export "memory") 1)
 (global (export "__heap_base") i32 (i32.const 1024))
 (func (export "Core_version") (param i32 i32) (result i64)
  i64.const 266287972352)
 (func (export "Fixture_add") (param i32 i32) (result i64)
  i32.const 128 i32.const 40 i32.const 2 i32.add i32.store
  i64.const 17179869312)
 (data (i32.const 0) "\54\73\6f\72\61\2d\65\78\65\63\75\74\6f\72\2d\66\69\78\74\75\72\65\54\73\6f\72\61\2d\65\78\65\63\75\74\6f\72\2d\66\69\78\74\75\72\65\01\00\00\00\01\00\00\00\01\00\00\00\00\01\00\00\00\01"))
