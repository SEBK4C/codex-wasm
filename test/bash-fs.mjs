// just-bash IFileSystem over a memfs fs (from cc-in-just-bash src/vfs.mjs).
import path from "node:path";

const toStat = (s) => ({
  isFile: s.isFile(),
  isDirectory: s.isDirectory(),
  isSymbolicLink: s.isSymbolicLink(),
  mode: Number(s.mode),
  size: Number(s.size),
  mtime: s.mtime,
  dev: s.dev,
  ino: s.ino,
});

const enc = (o) => (typeof o === "string" ? o : o?.encoding) || "utf8";

// just-bash IFileSystem over a memfs fs.
export function bashAdapter(fs) {
  const p = fs.promises;
  return {
    async readFile(f, o) { return p.readFile(f, { encoding: enc(o) }); },
    async readFileBuffer(f) { return new Uint8Array(await p.readFile(f)); },
    async writeFile(f, c, o) { await p.writeFile(f, c, typeof c === "string" ? { encoding: enc(o) } : undefined); },
    async appendFile(f, c, o) { await p.appendFile(f, c, typeof c === "string" ? { encoding: enc(o) } : undefined); },
    async exists(f) { try { await p.lstat(f); return true; } catch { return false; } },
    async stat(f) { return toStat(await p.stat(f)); },
    async lstat(f) { return toStat(await p.lstat(f)); },
    async mkdir(f, o) { await p.mkdir(f, { recursive: !!o?.recursive }); },
    async createExclusive(f, o) {
      if (o.directory) return p.mkdir(f, { mode: o.mode });
      const h = await p.open(f, "wx", o.mode); await h.close();
    },
    async readdir(f) { return (await p.readdir(f)).map(String); },
    async readdirWithFileTypes(f) {
      return (await p.readdir(f, { withFileTypes: true })).map((d) => ({
        name: String(d.name), isFile: d.isFile(), isDirectory: d.isDirectory(), isSymbolicLink: d.isSymbolicLink(),
      }));
    },
    async rm(f, o) { await p.rm(f, { recursive: !!o?.recursive, force: !!o?.force }); },
    async cp(a, b, o) {
      const s = await p.lstat(a);
      if (!s.isDirectory()) return p.copyFile(a, b);
      if (!o?.recursive) throw Object.assign(new Error(`cp: -r not specified; omitting directory '${a}'`), { code: "EISDIR" });
      await p.mkdir(b, { recursive: true });
      for (const n of await p.readdir(a)) await this.cp(path.posix.join(a, String(n)), path.posix.join(b, String(n)), o);
    },
    async mv(a, b) { await p.rename(a, b); },
    resolvePath(base, f) { return path.posix.resolve(base, f); },
    getAllPaths() {
      const out = [];
      const walk = (d) => {
        for (const n of fs.readdirSync(d)) {
          const f = path.posix.join(d, String(n)); out.push(f);
          try { if (fs.lstatSync(f).isDirectory()) walk(f); } catch {}
        }
      };
      walk("/"); return out;
    },
    async chmod(f, m) { await p.chmod(f, m); },
    async symlink(t, l) { await p.symlink(t, l); },
    async link(a, b) { await p.link(a, b); },
    async readlink(f) { return String(await p.readlink(f)); },
    async realpath(f) { return String(await p.realpath(f)); },
    async utimes(f, a, m) { await p.utimes(f, a, m); },
  };
}
