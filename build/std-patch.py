#!/usr/bin/env python3
"""Patch the toolchain's std sources (rust-src) for wasm32-wasip1 in a browser tab.

On WASI preview 1 std cannot run processes, and its stand-in `ExitStatus` always reports exit
code 0. Codex's processes run in the tab's shell (the tokio fork), so their real exit code has
to fit into `std::process::ExitStatus`:

- `sys/process/unsupported.rs`: `ExitStatus` carries an `i32`; `code()`, `exit_ok()` and
  `ExitStatusError` use it.
- `os/wasi/process.rs` (new): `ExitStatusExt::from_raw`, like `std::os::unix::process`.

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
print("std patched:", std, file=sys.stderr)
