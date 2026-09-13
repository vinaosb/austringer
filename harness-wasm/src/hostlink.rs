//! The wasmtime linker glue: exposes the three interface-1 host functions
//! (`log`, `read_file`, `write_file`) to the guest, translating between
//! guest memory regions and the behavior in [`crate::hostfns`].
//!
//! Borrow discipline: every `caller.data_mut().host.<method>()` call is a
//! short-lived borrow, never held across a memory access (which re-borrows
//! the caller).

use wasmtime::{Caller, Extern, Linker, Memory};

use crate::hosttypes::{DenyReason, deny_code};
use crate::runtime::StoreData;

fn guest_memory(caller: &mut Caller<'_, StoreData>) -> Option<Memory> {
    caller.get_export("memory").and_then(Extern::into_memory)
}

fn read_region(caller: &mut Caller<'_, StoreData>, ptr: i32, len: i32) -> Option<Vec<u8>> {
    if ptr < 0 || len < 0 {
        return None;
    }
    let memory = guest_memory(caller)?;
    let mut buf = vec![0_u8; len as usize];
    memory.read(&mut *caller, ptr as usize, &mut buf).ok()?;
    Some(buf)
}

/// Register the three interface-1 host functions on `linker`.
pub(crate) fn register(linker: &mut Linker<StoreData>) -> Result<(), wasmtime::Error> {
    linker.func_wrap(
        crate::abi::HOST_MODULE,
        "log",
        |mut caller: Caller<'_, StoreData>, level: i32, ptr: i32, len: i32| {
            let Some(bytes) = read_region(&mut caller, ptr, len) else {
                return;
            };
            let text = String::from_utf8_lossy(&bytes);
            caller.data_mut().host.record_log(level, &text);
        },
    )?;
    linker.func_wrap(
        crate::abi::HOST_MODULE,
        "read_file",
        |mut caller: Caller<'_, StoreData>,
         path_ptr: i32,
         path_len: i32,
         buf_ptr: i32,
         buf_cap: i32| {
            let Some(path_bytes) = read_region(&mut caller, path_ptr, path_len) else {
                return deny_code(DenyReason::BadPointer);
            };
            let path = String::from_utf8_lossy(&path_bytes).into_owned();
            match caller.data_mut().host.read_file(&path) {
                Ok(content) => {
                    let cap = usize::try_from(buf_cap.max(0)).unwrap_or(0);
                    if content.len() > cap {
                        return caller
                            .data_mut()
                            .host
                            .deny_read(&path, DenyReason::TooLarge);
                    }
                    let Some(memory) = guest_memory(&mut caller) else {
                        return deny_code(DenyReason::BadPointer);
                    };
                    if memory
                        .write(&mut caller, buf_ptr.max(0) as usize, &content)
                        .is_err()
                    {
                        return caller
                            .data_mut()
                            .host
                            .deny_read(&path, DenyReason::BadPointer);
                    }
                    caller.data_mut().host.push_read_ok(&path, content.len());
                    content.len() as i32
                }
                Err(reason) => caller.data_mut().host.deny_read(&path, reason),
            }
        },
    )?;
    linker.func_wrap(
        crate::abi::HOST_MODULE,
        "write_file",
        |mut caller: Caller<'_, StoreData>,
         path_ptr: i32,
         path_len: i32,
         data_ptr: i32,
         data_len: i32| {
            let Some(path_bytes) = read_region(&mut caller, path_ptr, path_len) else {
                return deny_code(DenyReason::BadPointer);
            };
            let Some(data) = read_region(&mut caller, data_ptr, data_len) else {
                return deny_code(DenyReason::BadPointer);
            };
            let path = String::from_utf8_lossy(&path_bytes).into_owned();
            match caller.data_mut().host.write_file(&path, &data) {
                Ok(bytes) => {
                    caller.data_mut().host.push_write_ok(&path, bytes);
                    0
                }
                Err(reason) => caller.data_mut().host.deny_write(&path, reason),
            }
        },
    )?;
    Ok(())
}
