//! Processes on wasm32-wasip1 in a browser tab: the tab's shell runs them (codex-wasm-host).
//!
//! `std::process::Command` cannot spawn on WASI, so `Command::spawn` hands the program, its
//! arguments, the environment and the working directory to the tab instead. Its stdio
//! settings are opaque (`std::process::Stdio` cannot be inspected), so all three streams are
//! always pipes: output nobody reads is dropped with the process, and the tab starts the
//! command once stdin is closed or someone waits for output or exit.

use crate::io::{AsyncRead, AsyncWrite, ReadBuf};
use crate::process::kill::Kill;
use crate::process::SpawnedChild;

use codex_wasm_host::Process;
use std::fmt::Write as _;
use std::future::Future;
use std::io;
use std::pin::Pin;
use std::process::{Command as StdCommand, ExitStatus, Stdio};
use std::sync::Arc;
use std::task::{Context, Poll};

fn json_str(out: &mut String, s: &str) {
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => {
                let _ = write!(out, "\\u{:04x}", c as u32);
            }
            c => out.push(c),
        }
    }
    out.push('"');
}

fn spec(cmd: &StdCommand) -> String {
    let lossy = |s: &std::ffi::OsStr| s.to_string_lossy().into_owned();
    let mut out = String::from("{\"program\":");
    json_str(&mut out, &lossy(cmd.get_program()));
    out.push_str(",\"args\":[");
    for (i, a) in cmd.get_args().enumerate() {
        if i > 0 {
            out.push(',');
        }
        json_str(&mut out, &lossy(a));
    }
    // The child's full environment: ours, then the command's changes.
    let mut env: std::collections::BTreeMap<String, String> = std::env::vars().collect();
    for (k, v) in cmd.get_envs() {
        match v {
            Some(v) => env.insert(lossy(k), lossy(v)),
            None => env.remove(&lossy(k)),
        };
    }
    out.push_str("],\"env\":{");
    for (i, (k, v)) in env.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        json_str(&mut out, k);
        out.push(':');
        json_str(&mut out, v);
    }
    out.push_str("},\"cwd\":");
    match cmd.get_current_dir() {
        Some(d) => json_str(&mut out, &d.to_string_lossy()),
        None => json_str(
            &mut out,
            &std::env::current_dir()
                .map(|d| d.to_string_lossy().into_owned())
                .unwrap_or_else(|_| "/".into()),
        ),
    }
    out.push('}');
    out
}

pub(crate) fn build_child_wasi(cmd: &StdCommand) -> io::Result<SpawnedChild> {
    let process = Arc::new(Process::spawn(spec(cmd).as_bytes())?);
    Ok(SpawnedChild {
        child: Child { process: process.clone(), status: None },
        stdin: Some(ChildStdio { process: process.clone(), fd: 0, closed: false }),
        stdout: Some(ChildStdio { process: process.clone(), fd: 1, closed: false }),
        stderr: Some(ChildStdio { process, fd: 2, closed: false }),
    })
}

fn exit_status(code: i32) -> ExitStatus {
    use std::os::wasi::process::ExitStatusExt;
    ExitStatus::from_raw(code)
}

#[derive(Debug)]
pub(crate) struct Child {
    process: Arc<Process>,
    status: Option<ExitStatus>,
}

impl Child {
    pub(crate) fn id(&self) -> u32 {
        self.process.id()
    }

    pub(crate) fn try_wait(&mut self) -> io::Result<Option<ExitStatus>> {
        if self.status.is_none() {
            self.status = self.process.try_wait().map(exit_status);
        }
        Ok(self.status)
    }
}

impl Kill for Child {
    fn kill(&mut self) -> io::Result<()> {
        self.process.kill();
        Ok(())
    }
}

impl Future for Child {
    type Output = io::Result<ExitStatus>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.get_mut();
        if let Some(s) = this.status {
            return Poll::Ready(Ok(s));
        }
        match this.process.poll_wait(cx.waker()) {
            Some(code) => {
                let s = exit_status(code);
                this.status = Some(s);
                Poll::Ready(Ok(s))
            }
            None => Poll::Pending,
        }
    }
}

#[derive(Debug)]
pub(crate) struct ChildStdio {
    process: Arc<Process>,
    fd: u32,
    closed: bool,
}

impl Drop for ChildStdio {
    fn drop(&mut self) {
        if self.fd == 0 && !self.closed {
            self.process.close_stdin();
        }
    }
}

impl AsyncRead for ChildStdio {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        if self.fd == 0 {
            return Poll::Ready(Err(io::Error::new(io::ErrorKind::Unsupported, "stdin is not readable")));
        }
        let dst = buf.initialize_unfilled();
        match self.process.read(self.fd, dst, cx.waker()) {
            Ok(Some(n)) => {
                buf.advance(n);
                Poll::Ready(Ok(()))
            }
            Ok(None) => Poll::Pending,
            Err(e) => Poll::Ready(Err(e)),
        }
    }
}

impl AsyncWrite for ChildStdio {
    fn poll_write(self: Pin<&mut Self>, _cx: &mut Context<'_>, buf: &[u8]) -> Poll<io::Result<usize>> {
        if self.fd != 0 || self.closed {
            return Poll::Ready(Err(io::Error::new(io::ErrorKind::BrokenPipe, "not writable")));
        }
        Poll::Ready(self.process.write_stdin(buf))
    }

    fn poll_flush(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Poll::Ready(Ok(()))
    }

    fn poll_shutdown(mut self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        if self.fd == 0 && !self.closed {
            self.closed = true;
            self.process.close_stdin();
        }
        Poll::Ready(Ok(()))
    }
}

pub(crate) fn stdio<T>(_io: T) -> io::Result<ChildStdio> {
    Err(io::Error::new(io::ErrorKind::Unsupported, "std child pipes do not exist on wasm32-wasip1"))
}

pub(crate) fn convert_to_stdio(_child_stdio: ChildStdio) -> io::Result<Stdio> {
    Err(io::Error::new(io::ErrorKind::Unsupported, "a tab pipe cannot become a std Stdio"))
}
