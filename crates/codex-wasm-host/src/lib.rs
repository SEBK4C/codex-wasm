//! What Codex on wasm32-wasip1 needs from the browser tab beyond WASI preview 1: processes (run
//! by the tab's just-bash shell) and HTTP (the tab's `fetch`).
//!
//! Every operation is non-blocking. A call that has nothing yet returns [`PENDING`]; the caller
//! parks its waker on the handle with [`park`], and the tab calls the module's exported
//! `codex_host_wake(handle)` when something happened on that handle. Nothing in here ever
//! blocks the tab: the runtime is driven from JavaScript, one tick at a time.
//!
//! Handles are small positive integers chosen by the tab. Process and HTTP handles share one
//! space.

use std::cell::RefCell;
use std::collections::HashMap;
use std::task::Waker;

/// The operation would block: park the waker and try again when woken.
pub const PENDING: i32 = -1;

#[cfg(target_os = "wasi")]
mod sys {
    #[link(wasm_import_module = "codex_host")]
    extern "C" {
        pub fn proc_spawn(spec: *const u8, spec_len: usize) -> i32;
        pub fn proc_write(h: u32, data: *const u8, len: usize) -> i32;
        pub fn proc_close_stdin(h: u32);
        pub fn proc_read(h: u32, fd: u32, buf: *mut u8, cap: usize) -> i32;
        pub fn proc_wait(h: u32) -> i32;
        pub fn proc_kill(h: u32);
        pub fn proc_drop(h: u32);
        pub fn http_start(spec: *const u8, spec_len: usize, body: *const u8, body_len: usize) -> i32;
        pub fn http_head(h: u32, buf: *mut u8, cap: usize) -> i32;
        pub fn http_read(h: u32, buf: *mut u8, cap: usize) -> i32;
        pub fn http_drop(h: u32);
        pub fn take_message(buf: *mut u8, cap: usize) -> i32;
        pub fn emit(data: *const u8, len: usize);
    }
}

#[cfg(not(target_os = "wasi"))]
#[allow(clippy::missing_safety_doc, unused_variables)]
mod sys {
    // Native builds (cargo check on the host) link against these; they report "unsupported".
    pub unsafe fn proc_spawn(spec: *const u8, spec_len: usize) -> i32 { -38 }
    pub unsafe fn proc_write(h: u32, data: *const u8, len: usize) -> i32 { -38 }
    pub unsafe fn proc_close_stdin(h: u32) {}
    pub unsafe fn proc_read(h: u32, fd: u32, buf: *mut u8, cap: usize) -> i32 { 0 }
    pub unsafe fn proc_wait(h: u32) -> i32 { 127 }
    pub unsafe fn proc_kill(h: u32) {}
    pub unsafe fn proc_drop(h: u32) {}
    pub unsafe fn http_start(spec: *const u8, spec_len: usize, body: *const u8, body_len: usize) -> i32 { -38 }
    pub unsafe fn http_head(h: u32, buf: *mut u8, cap: usize) -> i32 { -38 }
    pub unsafe fn http_read(h: u32, buf: *mut u8, cap: usize) -> i32 { -38 }
    pub unsafe fn http_drop(h: u32) {}
    pub unsafe fn take_message(buf: *mut u8, cap: usize) -> i32 { PENDING_ }
    pub unsafe fn emit(data: *const u8, len: usize) {}
    const PENDING_: i32 = -1;
}

thread_local! {
    static WAKERS: RefCell<HashMap<u32, Vec<Waker>>> = RefCell::new(HashMap::new());
}

/// Wake `waker` the next time the tab reports activity on `handle`.
pub fn park(handle: u32, waker: &Waker) {
    WAKERS.with(|w| {
        let mut w = w.borrow_mut();
        let list = w.entry(handle).or_default();
        if !list.iter().any(|known| known.will_wake(waker)) {
            list.push(waker.clone());
        }
    });
}

/// Called by the module's `codex_host_wake` export: wake everything parked on `handle`.
pub fn wake(handle: u32) {
    let wakers = WAKERS.with(|w| w.borrow_mut().remove(&handle));
    for waker in wakers.into_iter().flatten() {
        waker.wake();
    }
}

fn forget(handle: u32) {
    WAKERS.with(|w| w.borrow_mut().remove(&handle));
}

/// Read a variable-length answer: `f(buf, cap)` returns the full length (copying only when it
/// fits), [`PENDING`], or another negative error.
fn read_sized(mut f: impl FnMut(*mut u8, usize) -> i32) -> Result<Vec<u8>, i32> {
    let mut buf = vec![0u8; 4096];
    loop {
        let n = f(buf.as_mut_ptr(), buf.len());
        if n < 0 {
            return Err(n);
        }
        let n = n as usize;
        if n <= buf.len() {
            buf.truncate(n);
            return Ok(buf);
        }
        buf.resize(n, 0);
    }
}

fn os_error(code: i32) -> std::io::Error {
    match -code {
        2 => std::io::Error::new(std::io::ErrorKind::NotFound, "no such command in the tab"),
        32 => std::io::Error::new(std::io::ErrorKind::BrokenPipe, "process stdin is closed"),
        38 => std::io::Error::new(std::io::ErrorKind::Unsupported, "not supported in the browser tab"),
        _ => std::io::Error::other(format!("host error {code}")),
    }
}

/// A process the tab runs. Dropping the last reference releases it in the tab.
#[derive(Debug)]
pub struct Process {
    handle: u32,
}

impl Process {
    /// `spec` is JSON: `{"program", "args", "env", "cwd"}` (env complete, not a diff).
    pub fn spawn(spec: &[u8]) -> std::io::Result<Process> {
        let h = unsafe { sys::proc_spawn(spec.as_ptr(), spec.len()) };
        if h <= 0 {
            return Err(os_error(if h == 0 { -5 } else { h }));
        }
        Ok(Process { handle: h as u32 })
    }

    pub fn id(&self) -> u32 {
        self.handle
    }

    /// Bytes accepted (all of them), or an error once stdin is closed.
    pub fn write_stdin(&self, data: &[u8]) -> std::io::Result<usize> {
        let n = unsafe { sys::proc_write(self.handle, data.as_ptr(), data.len()) };
        if n < 0 { Err(os_error(n)) } else { Ok(n as usize) }
    }

    pub fn close_stdin(&self) {
        unsafe { sys::proc_close_stdin(self.handle) }
    }

    /// `fd` 1 or 2. `Ok(None)` means nothing yet (waker parked); `Ok(Some(0))` is end of file.
    pub fn read(&self, fd: u32, buf: &mut [u8], waker: &Waker) -> std::io::Result<Option<usize>> {
        let n = unsafe { sys::proc_read(self.handle, fd, buf.as_mut_ptr(), buf.len()) };
        if n == PENDING {
            park(self.handle, waker);
            return Ok(None);
        }
        if n < 0 { Err(os_error(n)) } else { Ok(Some(n as usize)) }
    }

    /// The exit code once the process ended (128 + signal when it was killed).
    pub fn try_wait(&self) -> Option<i32> {
        let code = unsafe { sys::proc_wait(self.handle) };
        if code == PENDING { None } else { Some(code) }
    }

    pub fn poll_wait(&self, waker: &Waker) -> Option<i32> {
        let code = self.try_wait();
        if code.is_none() {
            park(self.handle, waker);
        }
        code
    }

    pub fn kill(&self) {
        unsafe { sys::proc_kill(self.handle) }
    }
}

impl Drop for Process {
    fn drop(&mut self) {
        forget(self.handle);
        unsafe { sys::proc_drop(self.handle) }
    }
}

/// One HTTP exchange through the tab's `fetch`.
#[derive(Debug)]
pub struct Http {
    handle: u32,
}

impl Http {
    /// `spec` is JSON: `{"method", "url", "headers": [[name, value], ...]}`.
    pub fn start(spec: &[u8], body: &[u8]) -> std::io::Result<Http> {
        let h = unsafe { sys::http_start(spec.as_ptr(), spec.len(), body.as_ptr(), body.len()) };
        if h <= 0 {
            return Err(os_error(if h == 0 { -5 } else { h }));
        }
        Ok(Http { handle: h as u32 })
    }

    /// The response head as JSON (`{"status", "headers", "url"}` or `{"error"}`), once it came.
    pub fn poll_head(&self, waker: &Waker) -> Option<Vec<u8>> {
        match read_sized(|buf, cap| unsafe { sys::http_head(self.handle, buf, cap) }) {
            Ok(head) => Some(head),
            Err(PENDING) => {
                park(self.handle, waker);
                None
            }
            Err(code) => Some(format!("{{\"error\":\"host error {code}\"}}").into_bytes()),
        }
    }

    /// Body bytes: `Ok(None)` nothing yet (waker parked), `Ok(Some(0))` the end. A failed body
    /// read is an error.
    pub fn read(&self, buf: &mut [u8], waker: &Waker) -> std::io::Result<Option<usize>> {
        let n = unsafe { sys::http_read(self.handle, buf.as_mut_ptr(), buf.len()) };
        if n == PENDING {
            park(self.handle, waker);
            return Ok(None);
        }
        if n < 0 {
            return Err(std::io::Error::new(std::io::ErrorKind::ConnectionReset, "the response body failed in the tab"));
        }
        Ok(Some(n as usize))
    }
}

impl Drop for Http {
    fn drop(&mut self) {
        forget(self.handle);
        unsafe { sys::http_drop(self.handle) }
    }
}

/// Handle 0 is the inbox: messages the page sends to Codex (JSON, one per call).
pub const INBOX: u32 = 0;

/// The next message from the page, or `None` (waker parked on [`INBOX`]).
pub fn poll_message(waker: &Waker) -> Option<Vec<u8>> {
    match read_sized(|buf, cap| unsafe { sys::take_message(buf, cap) }) {
        Ok(m) => Some(m),
        Err(_) => {
            park(INBOX, waker);
            None
        }
    }
}

/// Send one message (JSON) to the page.
pub fn emit(message: &[u8]) {
    unsafe { sys::emit(message.as_ptr(), message.len()) }
}
