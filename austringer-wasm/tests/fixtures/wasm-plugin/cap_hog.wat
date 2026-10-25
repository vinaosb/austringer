;; austringer-wasm fixture: declares the `process-exec` capability in its
;; manifest. Loading it without an explicit grant must fail with the typed
;; CapabilityNotGranted error (no silent capability escalation).
(module
  (memory (export "memory") 1)
  (data (i32.const 16384) "{\"name\":\"cap-hog-fixture\",\"interface\":1,\"capabilities\":[\"process-exec\"],\"preopens\":[],\"tools\":[{\"name\":\"probe\",\"description\":\"declares process-exec without a grant\",\"input_schema\":{\"type\":\"object\"}}]}\00")
  (func (export "tool_call") (param $tool i32) (param $ilen i32) (result i32)
    i32.const 0))
