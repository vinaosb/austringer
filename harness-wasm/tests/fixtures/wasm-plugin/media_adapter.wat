;; harness-wasm fixture: interface-2 media-adapter plugin (M6 seam). It
;; declares the media-adapter capability and targets interface 2 of the
;; ladder. v0.2 ships SPEC + manifest validation ONLY - no read_media/
;; write_media host impls exist yet, so this fixture deliberately imports
;; nothing beyond the interface-1 ABI table (a plugin importing the
;; interface-2 host fns cannot link today; that is the documented v0.2
;; boundary).
(module
  (memory (export "memory") 1)
  (data (i32.const 16384) "{\"name\":\"media-adapter-fixture\",\"interface\":2,\"capabilities\":[\"media-adapter\"],\"preopens\":[],\"tools\":[{\"name\":\"meta\",\"description\":\"interface-2 media adapter declaration fixture\",\"input_schema\":{\"type\":\"object\"}}]}\00")
  (func (export "tool_call") (param $tool i32) (param $ilen i32) (result i32)
    i32.const 0))
