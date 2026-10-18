#!/usr/bin/env python3
"""Patch the toolchain's std sources (rust-src) for wasm32-wasip1 in a browser tab.

On WASI preview 1 std cannot run processes, and its stand-in `ExitStatus` always reports exit
code 0. Codex's processes run in the tab's shell (the tokio fork), so their real exit code has
to fit into `std::process::ExitStatus`:

- `sys/process/unsupported.rs`: `ExitStatus` carries an `i32`; `code()`, `exit_ok()` and
  `ExitStatusError` use it.
- `os/wasi/process.rs` (new): `ExitStatusExt::from_raw`, like `std::os::unix::process`.
- `sys/pal/wasi/os.rs`: Unix behavior instead of panics and errors: `split_paths`/`join_paths`
  on ':', `home_dir` from HOME, `temp_dir` from TMPDIR (else /tmp), `getpid` 1, and
  `current_exe()` names the module (`CODEX_WASM_EXE`, else /usr/bin/codex); Codex's config
  loader places its packaged defaults next to it.
- `sys/thread/unix.rs`: `available_parallelism()` is 1 instead of an error.
- `sys/fs/unix.rs`: `File::lock`/`try_lock`/`lock_shared`/`unlock` succeed on WASI (one process
  per file system in a tab) instead of failing.

Built with `-Z build-std` (RUSTC_BOOTSTRAP=1), so these sources are what the module links.
"""
import pathlib
import re
import subprocess
import sys

sysroot = subprocess.check_output(["rustc", "--print", "sysroot"], text=True).strip()
std = pathlib.Path(sysroot) / "lib/rustlib/src/rust/library/std/src"

unsupported = std / "sys/process/unsupported.rs"
s = unsupported.read_text()
if "codex-wasm" not in s:
    start = s.index("#[derive(PartialEq, Eq, Clone, Copy, Debug, Default)]\n#[non_exhaustive]\npub struct ExitStatus();")
    end_marker = "impl ExitStatusError {\n    pub fn code(self) -> Option<NonZero<i32>> {\n        self.0\n    }\n}\n"
    end = s.index(end_marker) + len(end_marker)
    s = s[:start] + '''// codex-wasm: the exit code a browser tab's shell reported for the process.
#[derive(PartialEq, Eq, Clone, Copy, Debug, Default)]
#[non_exhaustive]
pub struct ExitStatus(pub i32);

impl ExitStatus {
    pub fn exit_ok(&self) -> Result<(), ExitStatusError> {
        match NonZero::new(self.0) {
            None => Ok(()),
            Some(code) => Err(ExitStatusError(code)),
        }
    }

    pub fn code(&self) -> Option<i32> {
        Some(self.0)
    }
}

impl From<i32> for ExitStatus {
    fn from(raw: i32) -> ExitStatus {
        ExitStatus(raw)
    }
}

impl fmt::Display for ExitStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "exit status: {}", self.0)
    }
}

#[derive(PartialEq, Eq, Clone, Copy, Debug)]
pub struct ExitStatusError(NonZero<i32>);

impl Into<ExitStatus> for ExitStatusError {
    fn into(self) -> ExitStatus {
        ExitStatus(self.0.get())
    }
}

impl ExitStatusError {
    pub fn code(self) -> Option<NonZero<i32>> {
        Some(self.0)
    }
}
''' + s[end:]
    unsupported.write_text(s)

wasi_mod = std / "os/wasi/mod.rs"
m = wasi_mod.read_text()
if "pub mod process;" not in m:
    m = m.replace("pub mod io;\n", "pub mod io;\n\n// codex-wasm: exit codes of processes the browser tab runs.\n#[cfg(all(target_os = \"wasi\", target_env = \"p1\"))]\npub mod process;\n", 1)
    wasi_mod.write_text(m)

(std / "os/wasi/process.rs").write_text('''//! codex-wasm: process extensions for wasm32-wasip1 in a browser tab, whose shell runs the
//! processes and reports their exit codes.
#![stable(feature = "codex_wasi_exit_status", since = "1.95.0")]

use crate::process;
use crate::sys::FromInner;

/// Build an `ExitStatus` from the exit code the tab reported.
#[stable(feature = "codex_wasi_exit_status", since = "1.95.0")]
pub trait ExitStatusExt {
    /// An `ExitStatus` with exit code `raw` (128 + signal for a killed process).
    #[stable(feature = "codex_wasi_exit_status", since = "1.95.0")]
    fn from_raw(raw: i32) -> Self;
}

#[stable(feature = "codex_wasi_exit_status", since = "1.95.0")]
impl ExitStatusExt for process::ExitStatus {
    fn from_raw(raw: i32) -> Self {
        process::ExitStatus::from_inner(From::from(raw))
    }
}
''')
os_rs = std / "sys/pal/wasi/os.rs"
o = os_rs.read_text()
if "codex-wasm" not in o:
    a = o.index("pub struct SplitPaths<'a>(!, PhantomData<&'a ()>);")
    marker = "pub fn getpid() -> u32 {\n    panic!(\"unsupported\");\n}\n"
    b = o.index(marker) + len(marker)
    o = o[:a] + """// codex-wasm: Unix behavior for a browser tab's module: PATH lists split and join on ':',
// HOME and TMPDIR come from the environment, and the module names itself.
pub struct SplitPaths<'a> {
    iter: crate::iter::Map<crate::slice::Split<'a, u8, fn(&u8) -> bool>, fn(&'a [u8]) -> PathBuf>,
}

pub fn split_paths(unparsed: &OsStr) -> SplitPaths<'_> {
    fn bytes_to_path(b: &[u8]) -> PathBuf {
        PathBuf::from(<OsStr as OsStrExt>::from_bytes(b))
    }
    fn is_colon(b: &u8) -> bool {
        *b == b':'
    }
    let unparsed = unparsed.as_bytes();
    SplitPaths {
        iter: unparsed
            .split(is_colon as fn(&u8) -> bool)
            .map(bytes_to_path as fn(&[u8]) -> PathBuf),
    }
}

impl<'a> Iterator for SplitPaths<'a> {
    type Item = PathBuf;
    fn next(&mut self) -> Option<PathBuf> {
        self.iter.next()
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.iter.size_hint()
    }
}

#[derive(Debug)]
pub struct JoinPathsError;

pub fn join_paths<I, T>(paths: I) -> Result<OsString, JoinPathsError>
where
    I: Iterator<Item = T>,
    T: AsRef<OsStr>,
{
    let mut joined = Vec::new();
    for (i, path) in paths.enumerate() {
        let path = path.as_ref().as_bytes();
        if i > 0 {
            joined.push(b':')
        }
        if path.contains(&b':') {
            return Err(JoinPathsError);
        }
        joined.extend_from_slice(path);
    }
    Ok(OsStringExt::from_vec(joined))
}

impl fmt::Display for JoinPathsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        "path segment contains separator `:`".fmt(f)
    }
}

impl crate::error::Error for JoinPathsError {}

pub fn current_exe() -> io::Result<PathBuf> {
    Ok(crate::env::var_os("CODEX_WASM_EXE").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("/usr/bin/codex")))
}

#[allow(dead_code)]
pub fn page_size() -> usize {
    unsafe { libc::sysconf(libc::_SC_PAGESIZE) as usize }
}

pub fn temp_dir() -> PathBuf {
    crate::env::var_os("TMPDIR").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("/tmp"))
}

pub fn home_dir() -> Option<PathBuf> {
    crate::env::var_os("HOME").map(PathBuf::from)
}

pub fn getpid() -> u32 {
    1
}
""" + o[b:]
    os_rs.write_text(o)

thread_rs = std / "sys/thread/unix.rs"
t = thread_rs.read_text()
old = "pub fn available_parallelism() -> io::Result<NonZero<usize>> {\n"
if old in t and "codex-wasm" not in t:
    t = t.replace(old, old + """    // codex-wasm: one thread in a browser tab's module.
    #[cfg(target_os = "wasi")]
    return Ok(NonZero::<usize>::MIN);
    #[allow(unreachable_code)]
""", 1)
    thread_rs.write_text(t)

fs_rs = std / "sys/fs/unix.rs"
f = fs_rs.read_text()
if "codex-wasm" not in f:
    for name in ["lock()", "lock_shared()", "unlock()"]:
        old = f'Err(io::const_error!(io::ErrorKind::Unsupported, "{name} not supported"))'
        assert old in f, name
        f = f.replace(old, "// codex-wasm: one process per file system in a browser tab; nothing to lock against.\n"
                      f"        if cfg!(target_os = \"wasi\") {{ return Ok(()); }}\n        {old}", 1)
    for name in ["try_lock()", "try_lock_shared()"]:
        old = f'        Err(TryLockError::Error(io::const_error!(\n            io::ErrorKind::Unsupported,\n            "{name} not supported"\n        )))'
        assert old in f, name
        f = f.replace(old, "        // codex-wasm: see lock().\n        if cfg!(target_os = \"wasi\") {\n            return Ok(());\n        }\n" + old, 1)
    fs_rs.write_text(f)

print("std patched:", std, file=sys.stderr)
