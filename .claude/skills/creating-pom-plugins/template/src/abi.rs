//! The C ABI v1 boundary `pom-plugin-host` loads (`crates/enterprise-protocol`
//! in the POM repository). Nothing here knows the plugin's domain: `create`,
//! `ingest` and `query` hand bytes to `crate::Plugin`, and every callback
//! catches panics so none crosses the FFI boundary.

use std::ffi::{c_char, c_void};

pub const ABI_VERSION: u32 = 1;
/// NUL-terminated copy of the crate version, reported to the host on `ready`.
pub static VERSION: &str = concat!(env!("CARGO_PKG_VERSION"), "\0");

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct ByteSlice {
    pub ptr: *const u8,
    pub len: usize,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct ByteBuffer {
    pub ptr: *mut u8,
    pub len: usize,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct HostCallbacks {
    pub log: Option<unsafe extern "C" fn(message: ByteSlice)>,
}

pub type PluginHandle = *mut c_void;
pub type CreateFn = unsafe extern "C" fn(HostCallbacks, ByteSlice) -> PluginHandle;
pub type IngestFn = unsafe extern "C" fn(PluginHandle, ByteSlice) -> i32;
pub type QueryFn = unsafe extern "C" fn(PluginHandle, ByteSlice) -> ByteBuffer;
pub type FreeBufferFn = unsafe extern "C" fn(ByteBuffer);
pub type ShutdownFn = unsafe extern "C" fn(PluginHandle);

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct PluginApiV1 {
    pub abi_version: u32,
    pub plugin_version: *const c_char,
    pub capabilities: u64,
    pub create: Option<CreateFn>,
    pub ingest: Option<IngestFn>,
    pub query: Option<QueryFn>,
    pub free_buffer: Option<FreeBufferFn>,
    pub shutdown: Option<ShutdownFn>,
}

unsafe impl Sync for PluginApiV1 {}

const EMPTY: ByteBuffer = ByteBuffer {
    ptr: std::ptr::null_mut(),
    len: 0,
};

unsafe fn input_bytes<'a>(input: ByteSlice) -> Result<&'a [u8], String> {
    if input.len == 0 {
        return Ok(&[]);
    }
    if input.ptr.is_null() {
        return Err("null byte slice".into());
    }
    Ok(std::slice::from_raw_parts(input.ptr, input.len))
}

fn buffer_from_bytes(bytes: Vec<u8>) -> ByteBuffer {
    let mut bytes = bytes.into_boxed_slice();
    let buffer = ByteBuffer {
        ptr: bytes.as_mut_ptr(),
        len: bytes.len(),
    };
    std::mem::forget(bytes);
    buffer
}

unsafe extern "C" fn create(_: HostCallbacks, config: ByteSlice) -> PluginHandle {
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let config = input_bytes(config)?;
        let config = if config.is_empty() {
            serde_json::Value::Null
        } else {
            serde_json::from_slice(config).map_err(|error| error.to_string())?
        };
        let plugin = crate::Plugin::new(&config)?;
        Ok::<_, String>(Box::into_raw(Box::new(plugin)).cast::<c_void>())
    }));
    match result {
        Ok(Ok(handle)) => handle,
        _ => std::ptr::null_mut(),
    }
}

unsafe extern "C" fn ingest(handle: PluginHandle, event: ByteSlice) -> i32 {
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        if handle.is_null() {
            return Err("plugin handle is null".to_owned());
        }
        let event = serde_json::from_slice(input_bytes(event)?).map_err(|e| e.to_string())?;
        (*handle.cast::<crate::Plugin>()).ingest(&event)
    }));
    if matches!(result, Ok(Ok(()))) {
        0
    } else {
        -1
    }
}

/// An `Err` from the plugin becomes an empty buffer: the host reports a
/// generic failure and the node answers 502. A reason the screen must show
/// travels as an `Ok` JSON body with an `error` field instead.
unsafe extern "C" fn query(handle: PluginHandle, request: ByteSlice) -> ByteBuffer {
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        if handle.is_null() {
            return Err("plugin handle is null".to_owned());
        }
        let request = serde_json::from_slice(input_bytes(request)?).map_err(|e| e.to_string())?;
        (*handle.cast::<crate::Plugin>())
            .query(&request)
            .and_then(|value| serde_json::to_vec(&value).map_err(|error| error.to_string()))
    }));
    match result {
        Ok(Ok(bytes)) => buffer_from_bytes(bytes),
        _ => EMPTY,
    }
}

unsafe extern "C" fn free_buffer(buffer: ByteBuffer) {
    if !buffer.ptr.is_null() {
        let slice = std::ptr::slice_from_raw_parts_mut(buffer.ptr, buffer.len);
        drop(Box::from_raw(slice));
    }
}

unsafe extern "C" fn shutdown(handle: PluginHandle) {
    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        if !handle.is_null() {
            drop(Box::from_raw(handle.cast::<crate::Plugin>()));
        }
    }));
}

pub static API: PluginApiV1 = PluginApiV1 {
    abi_version: ABI_VERSION,
    plugin_version: VERSION.as_ptr().cast(),
    capabilities: 0,
    create: Some(create),
    ingest: Some(ingest),
    query: Some(query),
    free_buffer: Some(free_buffer),
    shutdown: Some(shutdown),
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn api_exposes_every_v1_callback() {
        assert_eq!(API.abi_version, ABI_VERSION);
        assert!(API.create.is_some() && API.ingest.is_some() && API.query.is_some());
        assert!(API.free_buffer.is_some() && API.shutdown.is_some());
        assert!(VERSION.ends_with('\0'));
    }

    #[test]
    fn query_round_trips_through_the_c_boundary() {
        unsafe {
            let handle = create(
                HostCallbacks::default(),
                ByteSlice {
                    ptr: [].as_ptr(),
                    len: 0,
                },
            );
            assert!(!handle.is_null());
            let request = br#"{"operation":"host.services"}"#;
            let buffer = query(
                handle,
                ByteSlice {
                    ptr: request.as_ptr(),
                    len: request.len(),
                },
            );
            let reply: serde_json::Value =
                serde_json::from_slice(std::slice::from_raw_parts(buffer.ptr, buffer.len)).unwrap();
            assert!(reply["services"].is_array());
            free_buffer(buffer);

            let unknown = br#"{"operation":"nope"}"#;
            let buffer = query(
                handle,
                ByteSlice {
                    ptr: unknown.as_ptr(),
                    len: unknown.len(),
                },
            );
            assert!(buffer.ptr.is_null() && buffer.len == 0);
            shutdown(handle);
        }
    }
}
