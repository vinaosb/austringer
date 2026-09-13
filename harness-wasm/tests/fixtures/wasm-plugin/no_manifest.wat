;; harness-wasm fixture: valid module that never writes a manifest.
;; Zero-initialized memory yields an empty manifest region - the host must
;; report ManifestMissing.
(module
  (memory (export "memory") 1)
  (func (export "tool_call") (param $tool i32) (param $ilen i32) (result i32)
    i32.const 0))
