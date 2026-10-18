// WASI preview 1 for Codex's module, over a memfs-style file system (the browser tab's
// in-memory volume: openSync/readSync/writeSync/fstatSync/statSync/lstatSync/readdirSync/...).
// One preopen, "/" as fd 3; wasi-libc resolves every path against it and emulates the cwd.
// Nothing here blocks: poll_oneoff reports clock subscriptions as due right away, because the
// module only polls with a timeout when its runtime would park, and the runtime never parks
// (it yields back to the tab instead; see codex-host.mjs).

const E = {
  SUCCESS: 0, ACCES: 2, AGAIN: 6, BADF: 8, EXIST: 20, INVAL: 28, IO: 29, ISDIR: 31, LOOP: 32,
  NAMETOOLONG: 37, NOENT: 44, NOSYS: 52, NOTDIR: 54, NOTEMPTY: 55, NOTSUP: 58, PERM: 63, SPIPE: 70,
  XDEV: 75, NOTCAPABLE: 76,
};
const CODE = {
  ENOENT: E.NOENT, EEXIST: E.EXIST, ENOTDIR: E.NOTDIR, EISDIR: E.ISDIR, ENOTEMPTY: E.NOTEMPTY,
  EACCES: E.ACCES, EPERM: E.PERM, EBADF: E.BADF, EINVAL: E.INVAL, ELOOP: E.LOOP, EXDEV: E.XDEV,
  ENAMETOOLONG: E.NAMETOOLONG, ENOSYS: E.NOSYS, ENOTSUP: E.NOTSUP,
};
const FT = { UNKNOWN: 0, CHAR: 2, DIR: 3, FILE: 4, LINK: 7 };
// Node/memfs open flags (Linux values).
const O = { RDONLY: 0, WRONLY: 1, RDWR: 2, CREAT: 0o100, EXCL: 0o200, TRUNC: 0o1000, APPEND: 0o2000 };
const RIGHT_READ = 1n << 1n, RIGHT_WRITE = 1n << 6n;
const ALL_RIGHTS = (1n << 30n) - 1n;

export class WasiExit extends Error {
  constructor(code) { super(`exit ${code}`); this.code = code; }
}

const errno = (e) => CODE[e?.code] ?? (e instanceof WasiExit ? null : E.IO);

function normalize(path) {
  const out = [];
  for (const part of path.split("/")) {
    if (!part || part === ".") continue;
    if (part === "..") out.pop(); else out.push(part);
  }
  return "/" + out.join("/");
}

const kindOf = (st) => (st.isDirectory() ? FT.DIR : st.isSymbolicLink() ? FT.LINK : st.isFile() ? FT.FILE : st.isCharacterDevice?.() ? FT.CHAR : FT.UNKNOWN);
const ns = (ms) => BigInt(Math.round(Number(ms) * 1e6));

/**
 * @param {object} o
 * @param {object} o.fs memfs-style fs (sync API)
 * @param {Record<string,string>} o.env
 * @param {(bytes: Uint8Array) => void} o.stdout
 * @param {(bytes: Uint8Array) => void} o.stderr
 */
export function createWasi({ fs, env = {}, stdout = () => {}, stderr = () => {} }) {
  let memory = null;
  const view = () => new DataView(memory.buffer);
  const u8 = () => new Uint8Array(memory.buffer);
  const text = new TextDecoder(), enc = new TextEncoder();
  const str = (ptr, len) => text.decode(u8().subarray(ptr, ptr + len));

  // fd table. 0-2 stdio, 3 the root preopen; files keep a memfs fd and our own position.
  const fds = new Map([
    [0, { kind: "stdin" }], [1, { kind: "out", write: stdout }], [2, { kind: "out", write: stderr }],
    [3, { kind: "dir", path: "/", preopen: "/" }],
  ]);
  let nextFd = 4;
  const fdOf = (fd) => fds.get(fd);
  const resolve = (dirfd, ptr, len) => {
    const d = fdOf(dirfd);
    if (!d || d.kind !== "dir") return null;
    const rel = str(ptr, len);
    return normalize(rel.startsWith("/") ? rel : `${d.path}/${rel}`);
  };
  const writeFilestat = (buf, st) => {
    const v = view();
    v.setBigUint64(buf, BigInt(st.dev ?? 0), true);
    v.setBigUint64(buf + 8, BigInt(st.ino ?? 0), true);
    v.setUint8(buf + 16, kindOf(st));
    v.setBigUint64(buf + 24, BigInt(st.nlink ?? 1), true);
    v.setBigUint64(buf + 32, BigInt(st.size ?? 0), true);
    v.setBigUint64(buf + 40, ns(st.atimeMs ?? 0), true);
    v.setBigUint64(buf + 48, ns(st.mtimeMs ?? 0), true);
    v.setBigUint64(buf + 56, ns(st.ctimeMs ?? 0), true);
  };
  const iovs = (ptr, n) => {
    const v = view(), out = [];
    for (let i = 0; i < n; i++) out.push([v.getUint32(ptr + i * 8, true), v.getUint32(ptr + i * 8 + 4, true)]);
    return out;
  };
  const guard = (f) => (...args) => {
    try { return f(...args); } catch (e) {
      if (e instanceof WasiExit) throw e;
      const n = errno(e);
      return n ?? E.IO;
    }
  };
  const times = (st, atim, mtim, flags) => {
    const now = Date.now() / 1000;
    const a = flags & 2 ? now : flags & 1 ? Number(atim) / 1e9 : Number(st.atimeMs) / 1000;
    const m = flags & 8 ? now : flags & 4 ? Number(mtim) / 1e9 : Number(st.mtimeMs) / 1000;
    return [a, m];
  };

  const imports = {
    args_sizes_get(argc, bufsize) { const v = view(); v.setUint32(argc, 0, true); v.setUint32(bufsize, 0, true); return 0; },
    args_get() { return 0; },
    environ_sizes_get(count, bufsize) {
      const entries = Object.entries(env).map(([k, v]) => enc.encode(`${k}=${v}\0`));
      const v = view();
      v.setUint32(count, entries.length, true);
      v.setUint32(bufsize, entries.reduce((n, e) => n + e.length, 0), true);
      return 0;
    },
    environ_get(environ, buf) {
      const v = view(), m = u8();
      for (const [k, val] of Object.entries(env)) {
        const e = enc.encode(`${k}=${val}\0`);
        v.setUint32(environ, buf, true); environ += 4;
        m.set(e, buf); buf += e.length;
      }
      return 0;
    },
    clock_res_get(id, out) { view().setBigUint64(out, 1000n, true); return 0; },
    clock_time_get(id, precision, out) {
      const t = id === 0 ? BigInt(Date.now()) * 1000000n + BigInt(Math.round((performance.now() % 1) * 1e6))
        : BigInt(Math.round(performance.now() * 1e6));
      view().setBigUint64(out, t, true);
      return 0;
    },
    random_get(buf, len) {
      const m = u8();
      for (let o = 0; o < len; o += 65536) crypto.getRandomValues(m.subarray(buf + o, buf + Math.min(len, o + 65536)));
      return 0;
    },
    sched_yield() { return 0; },
    proc_exit(code) { throw new WasiExit(code); },
    proc_raise() { return E.NOSYS; },
    poll_oneoff(inp, out, n, nevents) {
      const v = view();
      let k = 0;
      for (let i = 0; i < n; i++) {
        const s = inp + i * 48, e = out + k * 32;
        const tag = v.getUint8(s + 8);
        v.setBigUint64(e, v.getBigUint64(s, true), true);
        v.setUint16(e + 8, 0, true);
        v.setUint8(e + 10, tag);
        v.setBigUint64(e + 16, 0n, true);
        v.setUint16(e + 24, 0, true);
        k++;
      }
      v.setUint32(nevents, k, true);
      return 0;
    },
    sock_accept() { return E.NOTSUP; },
    sock_recv() { return E.NOTSUP; },
    sock_send() { return E.NOTSUP; },
    sock_shutdown() { return E.NOTSUP; },

    fd_prestat_get(fd, buf) {
      const d = fdOf(fd);
      if (!d?.preopen) return E.BADF;
      const v = view();
      v.setUint8(buf, 0);
      v.setUint32(buf + 4, enc.encode(d.preopen).length, true);
      return 0;
    },
    fd_prestat_dir_name(fd, ptr, len) {
      const d = fdOf(fd);
      if (!d?.preopen) return E.BADF;
      u8().set(enc.encode(d.preopen).subarray(0, len), ptr);
      return 0;
    },
    fd_fdstat_get(fd, buf) {
      const d = fdOf(fd);
      if (!d) return E.BADF;
      const v = view();
      const type = d.kind === "dir" ? FT.DIR : d.kind === "file" ? FT.FILE : FT.CHAR;
      v.setUint8(buf, type);
      v.setUint16(buf + 2, d.append ? 1 : 0, true);
      v.setBigUint64(buf + 8, ALL_RIGHTS, true);
      v.setBigUint64(buf + 16, ALL_RIGHTS, true);
      return 0;
    },
    fd_fdstat_set_flags(fd, flags) { const d = fdOf(fd); if (!d) return E.BADF; if (d.kind === "file") d.append = Boolean(flags & 1); return 0; },
    fd_fdstat_set_rights() { return 0; },
    fd_advise() { return 0; },
    fd_allocate: guard((fd, offset, len) => {
      const d = fdOf(fd); if (d?.kind !== "file") return E.BADF;
      const need = Number(offset) + Number(len);
      if (fs.fstatSync(d.fd).size < need) fs.ftruncateSync(d.fd, need);
      return 0;
    }),
    fd_close: guard((fd) => {
      const d = fdOf(fd);
      if (!d) return E.BADF;
      if (d.kind === "file") fs.closeSync(d.fd);
      fds.delete(fd);
      return 0;
    }),
    fd_datasync() { return 0; },
    fd_sync() { return 0; },
    fd_filestat_get: guard((fd, buf) => {
      const d = fdOf(fd);
      if (!d) return E.BADF;
      if (d.kind === "file") writeFilestat(buf, fs.fstatSync(d.fd));
      else if (d.kind === "dir") writeFilestat(buf, fs.statSync(d.path));
      else { view().setBigUint64(buf, 0n, true); writeFilestat(buf, { isDirectory: () => false, isSymbolicLink: () => false, isFile: () => false, isCharacterDevice: () => true }); }
      return 0;
    }),
    fd_filestat_set_size: guard((fd, size) => {
      const d = fdOf(fd); if (d?.kind !== "file") return E.BADF;
      fs.ftruncateSync(d.fd, Number(size));
      return 0;
    }),
    fd_filestat_set_times: guard((fd, atim, mtim, flags) => {
      const d = fdOf(fd); if (!d) return E.BADF;
      const path = d.kind === "file" ? d.path : d.path;
      const [a, m] = times(fs.statSync(path), atim, mtim, flags);
      fs.utimesSync(path, a, m);
      return 0;
    }),
    fd_read: guard((fd, iov, n, nread) => {
      const d = fdOf(fd);
      if (!d) return E.BADF;
      if (d.kind === "stdin") { view().setUint32(nread, 0, true); return 0; }
      if (d.kind !== "file") return E.BADF;
      let total = 0;
      for (const [ptr, len] of iovs(iov, n)) {
        if (!len) continue;
        const got = fs.readSync(d.fd, u8(), ptr, len, d.pos);
        d.pos += got; total += got;
        if (got < len) break;
      }
      view().setUint32(nread, total, true);
      return 0;
    }),
    fd_pread: guard((fd, iov, n, offset, nread) => {
      const d = fdOf(fd); if (d?.kind !== "file") return E.BADF;
      let pos = Number(offset), total = 0;
      for (const [ptr, len] of iovs(iov, n)) {
        const got = fs.readSync(d.fd, u8(), ptr, len, pos);
        pos += got; total += got;
        if (got < len) break;
      }
      view().setUint32(nread, total, true);
      return 0;
    }),
    fd_write: guard((fd, iov, n, nwritten) => {
      const d = fdOf(fd);
      if (!d) return E.BADF;
      let total = 0;
      if (d.kind === "out") {
        for (const [ptr, len] of iovs(iov, n)) { d.write(u8().slice(ptr, ptr + len)); total += len; }
      } else if (d.kind === "file") {
        if (d.append) d.pos = Number(fs.fstatSync(d.fd).size);
        for (const [ptr, len] of iovs(iov, n)) {
          if (!len) continue;
          const put = fs.writeSync(d.fd, u8().slice(ptr, ptr + len), 0, len, d.pos);
          d.pos += put; total += put;
        }
      } else return E.BADF;
      view().setUint32(nwritten, total, true);
      return 0;
    }),
    fd_pwrite: guard((fd, iov, n, offset, nwritten) => {
      const d = fdOf(fd); if (d?.kind !== "file") return E.BADF;
      let pos = Number(offset), total = 0;
      for (const [ptr, len] of iovs(iov, n)) {
        const put = fs.writeSync(d.fd, u8().slice(ptr, ptr + len), 0, len, pos);
        pos += put; total += put;
      }
      view().setUint32(nwritten, total, true);
      return 0;
    }),
    fd_seek: guard((fd, offset, whence, out) => {
      const d = fdOf(fd);
      if (!d) return E.BADF;
      if (d.kind !== "file") return E.SPIPE;
      const base = whence === 0 ? 0 : whence === 1 ? d.pos : Number(fs.fstatSync(d.fd).size);
      const pos = base + Number(offset);
      if (pos < 0) return E.INVAL;
      d.pos = pos;
      view().setBigUint64(out, BigInt(pos), true);
      return 0;
    }),
    fd_tell(fd, out) { const d = fdOf(fd); if (d?.kind !== "file") return E.BADF; view().setBigUint64(out, BigInt(d.pos), true); return 0; },
    fd_renumber(from, to) {
      const d = fdOf(from); if (!d) return E.BADF;
      const old = fdOf(to); if (old?.kind === "file") try { fs.closeSync(old.fd); } catch {}
      fds.set(to, d); fds.delete(from);
      return 0;
    },
    fd_readdir: guard((fd, buf, len, cookie, bufused) => {
      const d = fdOf(fd);
      if (d?.kind !== "dir") return E.BADF;
      const entries = [[".", FT.DIR], ["..", FT.DIR]];
      for (const e of fs.readdirSync(d.path, { withFileTypes: true })) {
        entries.push([String(e.name), e.isDirectory() ? FT.DIR : e.isSymbolicLink() ? FT.LINK : e.isFile() ? FT.FILE : FT.UNKNOWN]);
      }
      const v = view(), m = u8();
      let off = 0;
      for (let i = Number(cookie); i < entries.length && off < len; i++) {
        const name = enc.encode(entries[i][0]);
        const head = new Uint8Array(24), hv = new DataView(head.buffer);
        hv.setBigUint64(0, BigInt(i + 1), true);
        hv.setBigUint64(8, BigInt(i + 1), true);
        hv.setUint32(16, name.length, true);
        hv.setUint8(20, entries[i][1]);
        const rec = new Uint8Array(24 + name.length);
        rec.set(head); rec.set(name, 24);
        const take = Math.min(rec.length, len - off);
        m.set(rec.subarray(0, take), buf + off);
        off += take;
      }
      v.setUint32(bufused, off, true);
      return 0;
    }),

    path_open: guard((dirfd, dirflags, ptr, len, oflags, rightsBase, rightsInh, fdflags, out) => {
      const path = resolve(dirfd, ptr, len);
      if (!path) return E.BADF;
      const follow = Boolean(dirflags & 1);
      let st = null;
      try { st = follow ? fs.statSync(path) : fs.lstatSync(path); } catch (e) { if (e?.code !== "ENOENT") throw e; }
      if (oflags & 2 /* DIRECTORY */) {
        if (!st) return E.NOENT;
        if (!st.isDirectory()) return E.NOTDIR;
      }
      if (st && (oflags & 1) && (oflags & 4)) return E.EXIST;
      if (st?.isDirectory()) {
        if (rightsBase & RIGHT_WRITE && !(oflags & 2)) {
          // wasi-libc asks for write rights on O_RDWR opens; a directory cannot be written.
          if ((oflags & 8) || (fdflags & 1)) return E.ISDIR;
        }
        const fd = nextFd++;
        fds.set(fd, { kind: "dir", path });
        view().setUint32(out, fd, true);
        return 0;
      }
      if (!st && !(oflags & 1)) return E.NOENT;
      const read = Boolean(rightsBase & RIGHT_READ), write = Boolean(rightsBase & RIGHT_WRITE);
      let flags = write ? (read ? O.RDWR : O.WRONLY) : O.RDONLY;
      if (oflags & 1) flags |= O.CREAT;
      if (oflags & 4) flags |= O.EXCL;
      if (oflags & 8) flags |= O.TRUNC;
      const handle = fs.openSync(path, flags, 0o644);
      const fd = nextFd++;
      fds.set(fd, { kind: "file", fd: handle, path, pos: 0, append: Boolean(fdflags & 1) });
      view().setUint32(out, fd, true);
      return 0;
    }),
    path_filestat_get: guard((dirfd, flags, ptr, len, buf) => {
      const path = resolve(dirfd, ptr, len);
      if (!path) return E.BADF;
      writeFilestat(buf, flags & 1 ? fs.statSync(path) : fs.lstatSync(path));
      return 0;
    }),
    path_filestat_set_times: guard((dirfd, flags, ptr, len, atim, mtim, fst) => {
      const path = resolve(dirfd, ptr, len);
      if (!path) return E.BADF;
      const [a, m] = times(fs.statSync(path), atim, mtim, fst);
      fs.utimesSync(path, a, m);
      return 0;
    }),
    path_create_directory: guard((dirfd, ptr, len) => {
      const path = resolve(dirfd, ptr, len);
      if (!path) return E.BADF;
      fs.mkdirSync(path);
      return 0;
    }),
    path_remove_directory: guard((dirfd, ptr, len) => {
      const path = resolve(dirfd, ptr, len);
      if (!path) return E.BADF;
      fs.rmdirSync(path);
      return 0;
    }),
    path_unlink_file: guard((dirfd, ptr, len) => {
      const path = resolve(dirfd, ptr, len);
      if (!path) return E.BADF;
      if (fs.lstatSync(path).isDirectory()) return E.ISDIR;
      fs.unlinkSync(path);
      return 0;
    }),
    path_rename: guard((fd1, p1, l1, fd2, p2, l2) => {
      const a = resolve(fd1, p1, l1), b = resolve(fd2, p2, l2);
      if (!a || !b) return E.BADF;
      fs.renameSync(a, b);
      return 0;
    }),
    path_link: guard((fd1, flags, p1, l1, fd2, p2, l2) => {
      const a = resolve(fd1, p1, l1), b = resolve(fd2, p2, l2);
      if (!a || !b) return E.BADF;
      fs.linkSync(a, b);
      return 0;
    }),
    path_symlink: guard((p1, l1, fd, p2, l2) => {
      const target = str(p1, l1), link = resolve(fd, p2, l2);
      if (!link) return E.BADF;
      fs.symlinkSync(target, link);
      return 0;
    }),
    path_readlink: guard((dirfd, ptr, len, buf, buflen, used) => {
      const path = resolve(dirfd, ptr, len);
      if (!path) return E.BADF;
      if (!fs.lstatSync(path).isSymbolicLink()) return E.INVAL;
      const target = enc.encode(String(fs.readlinkSync(path)));
      const n = Math.min(target.length, buflen);
      u8().set(target.subarray(0, n), buf);
      view().setUint32(used, n, true);
      return 0;
    }),
  };

  return {
    imports,
    setMemory(m) { memory = m; },
  };
}
