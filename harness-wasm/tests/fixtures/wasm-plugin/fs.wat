;; harness-wasm fixture: filesystem plugin over a read-write preopen scope.
;; Host-fn paths are virtual: "<scope>/<relative>" - "data/note.txt" resolves
;; inside the host-granted "data" scope only.
;; tool 0 `read_note`  -> host read_file("data/note.txt") into the output region
;; tool 1 `write_note` -> host write_file("data/out.txt", input region bytes)
(module
  (import "harness" "read_file" (func $read_file (param i32 i32 i32 i32) (result i32)))
  (import "harness" "write_file" (func $write_file (param i32 i32 i32 i32) (result i32)))
  (memory (export "memory") 1)
  (data (i32.const 0) "data/note.txt")
  (data (i32.const 32) "data/out.txt")
  (data (i32.const 16384) "{\"name\":\"fs-fixture\",\"interface\":1,\"capabilities\":[],\"preopens\":[{\"name\":\"data\",\"mode\":\"rw\"}],\"tools\":[{\"name\":\"read_note\",\"description\":\"reads data/note.txt inside the preopen\",\"input_schema\":{\"type\":\"object\"}},{\"name\":\"write_note\",\"description\":\"writes the input to data/out.txt inside the preopen\",\"input_schema\":{\"type\":\"object\"}}]}\00")
  (func (export "tool_call") (param $tool i32) (param $ilen i32) (result i32)
    (local $r i32)
    (if (i32.eqz (local.get $tool))
      (then (return (call $read_file (i32.const 0) (i32.const 13) (i32.const 8192) (i32.const 4096)))))
    ;; write_note: refuse loudly if the host denied the write
    (local.set $r (call $write_file (i32.const 32) (i32.const 12) (i32.const 4096) (local.get $ilen)))
    (if (i32.ne (local.get $r) (i32.const 0))
      (then (return (local.get $r))))
    (i32.store8 (i32.const 8192) (i32.const 111))
    (i32.store8 (i32.const 8193) (i32.const 107))
    i32.const 2))
