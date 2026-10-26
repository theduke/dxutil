import { describe, expect, it, vi } from 'vitest';
vi.mock('@xterm/xterm', () => ({ Terminal: class {} }));
vi.mock('@xterm/addon-fit', () => ({ FitAddon: class {} }));
vi.mock('@xterm/addon-web-links', () => ({ WebLinksAddon: class {} }));
vi.mock('@xterm/addon-unicode11', () => ({ Unicode11Addon: class {} }));
import { fileLinks } from './index';
describe('terminal file links', () => {
  it('detects compiler paths, parentheses and columns', () => {
    expect(fileLinks('error (src/main.rs:12:4): bad')[0]).toMatchObject({path:'src/main.rs',line:12,column:4,start:7,end:23});
    expect(fileLinks('./foo.rs:3 ../lib.rs:9').map(x => x.path)).toEqual(['./foo.rs','../lib.rs']);
  });
  it('rejects URLs and invalid positions', () => {
    expect(fileLinks('https://example.com:123 src/main.rs:0')).toEqual([]);
    expect(fileLinks('src/main.rs:99999999999')).toEqual([]);
  });
});
