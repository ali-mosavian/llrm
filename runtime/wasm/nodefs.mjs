// The OS layer's files on node (runtime/shared/interface.toml, as runtime/shared/posix/os.c answers it): DOS paths
// mapped to the host's, a result below zero is minus the interface's code for the condition, else minus errno.
import fs from 'node:fs';
import { constants } from 'node:os';
import path from 'node:path';

const NOT_FOUND = 2, PATH_NOT_FOUND = 3, DENIED = 5, FILE_EXISTS = 80, NOT_SAME_DEVICE = 17, NO_MORE_FILES = 18;
const ATTR_SUBDIR = 0x10, ENTRY_ATTR = 21, ENTRY_SIZE = 26, ENTRY_NAME = 30, ENTRY_NAME_MAX = 12, INT32_LIMIT = 0x7fffffff;

const failure = (error) => {
  switch (error.code) {
    case 'ENOENT': return -NOT_FOUND;
    case 'ENOTDIR': return -PATH_NOT_FOUND;
    case 'EACCES': case 'EPERM': return -DENIED;
    default: return -(constants.errno[error.code] ?? 5);
  }
};
const attempt = (action) => { try { return action(); } catch (error) { return failure(error); } };

// A DOS path as the host names it: no drive letter, a slash for each backslash.
const native = (name) => (/^.:/.test(name) ? name.slice(2) : name).replaceAll('\\', '/');

// The wildcard of a DOS name: `*` and `?`; `*.*` is every name.
const matcher = (pattern) => {
  const wild = pattern === '*.*' ? '*' : pattern;
  return new RegExp('^' + wild.replace(/[.+^${}()|[\]\\]/g, '\\$&').replace(/\*/g, '.*').replace(/\?/g, '.') + '$');
};

export function nodeFiles() {
  // Node's files have no position of ours to seek: each open file keeps its own.
  const files = new Map();
  const take = (fd) => files.get(fd);
  const opened = (fd) => { files.set(fd, { position: 0 }); return fd; };
  return {
    open: (name, mode) => attempt(() => opened(fs.openSync(native(name), ['r', 'r+', 'r+'][mode] ?? 'r'))),
    create: (name) => attempt(() => opened(fs.openSync(native(name), 'w+'))),
    read: (fd, into) => attempt(() => {
      const file = take(fd);
      const got = fs.readSync(fd, into, 0, into.length, file.position);
      file.position += got;
      return got;
    }),
    write: (fd, data) => attempt(() => {
      const file = take(fd);
      const put = fs.writeSync(fd, data, 0, data.length, file.position);
      file.position += put;
      return put;
    }),
    seek: (fd, offset, origin) => attempt(() => {
      const file = take(fd);
      const base = [0, file.position, fs.fstatSync(fd).size][origin];
      if (base === undefined) return -22;
      const at = base + offset;
      if (at < 0) return -22;
      file.position = at;
      return at;
    }),
    close: (fd) => attempt(() => { files.delete(fd); fs.closeSync(fd); return 0; }),
    remove: (name) => attempt(() => { fs.unlinkSync(native(name)); return 0; }),
    rename: (from, to) => attempt(() => {
      if (fs.existsSync(native(to))) return -FILE_EXISTS;
      fs.renameSync(native(from), native(to));
      return 0;
    }),
    mkdir: (name) => attempt(() => { fs.mkdirSync(native(name)); return 0; }),
    chdir: (name) => attempt(() => { process.chdir(native(name)); return 0; }),
    rmdir: (name) => attempt(() => { fs.rmdirSync(native(name)); return 0; }),
    // The working directory as DOS prints it: no leading slash, a backslash for each slash.
    directory: () => process.cwd().replace(/^\//, '').replaceAll('/', '\\'),
    freeBytes: () => INT32_LIMIT,
    // One entry of `find_first`/`find_next`: DOS's 43-byte search record (attribute at 21, size at 26, name at 30).
    // `index` counts the matches handed out; the matches are listed afresh at index 0 and kept for the next calls.
    find: (() => {
      let matches = [];
      return (pattern, attrs, index, entry) => {
        if (index === 0) {
          const name = native(pattern);
          const directory = path.dirname(name) === '.' && !name.includes('/') ? '.' : path.dirname(name);
          const wanted = matcher(path.basename(name));
          matches = [];
          let listing;
          try { listing = fs.readdirSync(directory); } catch (error) { return failure(error); }
          for (const one of listing.sort()) {
            if (one.startsWith('.') || one.length > ENTRY_NAME_MAX || !wanted.test(one)) continue;
            let info;
            try { info = fs.statSync(path.join(directory, one)); } catch { continue; }
            if (info.isDirectory() && !(attrs & ATTR_SUBDIR)) continue;
            matches.push({ one, info });
          }
          if (!matches.length) return -NOT_FOUND;
        }
        if (index >= matches.length) return index === 0 ? -NOT_FOUND : -NO_MORE_FILES;
        const { one, info } = matches[index];
        entry.fill(0, 0, ENTRY_NAME + ENTRY_NAME_MAX + 1);
        const view = new DataView(entry.buffer, entry.byteOffset);
        view.setUint32(0, index + 1, true);
        entry[ENTRY_ATTR] = info.isDirectory() ? ATTR_SUBDIR : 0;
        view.setUint32(ENTRY_SIZE, info.isDirectory() ? 0 : Math.min(info.size, INT32_LIMIT), true);
        entry.set(Buffer.from(one, 'latin1'), ENTRY_NAME);
        return 0;
      };
    })(),
  };
}
