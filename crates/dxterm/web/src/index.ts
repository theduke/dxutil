import { Terminal } from '@xterm/xterm';
import { FitAddon } from '@xterm/addon-fit';
import { WebLinksAddon } from '@xterm/addon-web-links';
import { Unicode11Addon } from '@xterm/addon-unicode11';
import '@xterm/xterm/css/xterm.css';

type Send = (event: object) => void;
type Theme = { background: string; foreground: string; cursor: string; selectionBackground: string };
type Session = { terminal: Terminal; fit: FitAddon; observer: ResizeObserver; appearanceObserver: MutationObserver; theme: Partial<Theme>; font?: string; size?: number; destroyed: boolean };
const sessions = new Map<string, Session>();

// Canvas fonts cannot parse CSS var(); use the same concrete stack for glyphs
// and xterm's DOM cell measurement.
function resolvedFont(styles: CSSStyleDeclaration): string {
  return styles.getPropertyValue('--semantic-font-mono').trim()
    || styles.getPropertyValue('--font-mono').trim()
    || 'ui-monospace, SFMono-Regular, Consolas, Liberation Mono, Menlo, monospace';
}

// Only path:line(:column) links. URL handling is provided by WebLinksAddon.
export function fileLinks(text: string) {
  const matches: { start: number; end: number; path: string; line: number; column?: number }[] = [];
  const expression = /(?:^|[\s("'])([^\s<>"'():]+):(\d+)(?::(\d+))?/g;
  for (const match of text.matchAll(expression)) {
    const path = match[1];
    const line = Number(match[2]);
    const column = match[3] ? Number(match[3]) : undefined;
    if (text.slice(0, match.index! + match[0].indexOf(path)).endsWith('://') || path.includes('://') || !line || line > 0xffffffff || (column && column > 0xffffffff)) continue;
    const start = match.index! + match[0].indexOf(path);
    matches.push({ start, end: start + path.length + 1 + match[2].length + (match[3] ? match[3].length + 1 : 0), path, line, column });
  }
  return matches;
}
export function mount(id: string, host: HTMLElement, theme: Partial<Theme>, send: Send) {
  if (!host || sessions.has(id)) return;
  const styles = getComputedStyle(host);
  const terminal = new Terminal({ cursorBlink: true, scrollback: 10000, allowProposedApi: true, theme: {
    background: styles.getPropertyValue('--terminal-background').trim() || '#111820',
    foreground: styles.getPropertyValue('--terminal-foreground').trim() || '#dce5ed',
    cursor: styles.getPropertyValue('--terminal-cursor').trim() || '#dce5ed',
    selectionBackground: styles.getPropertyValue('--terminal-selection').trim() || '#45556b', ...theme,
  }, fontFamily: resolvedFont(styles), fontSize: 13 });
  const fit = new FitAddon(); terminal.loadAddon(fit);
  terminal.loadAddon(new Unicode11Addon()); terminal.unicode.activeVersion = '11';
  terminal.loadAddon(new WebLinksAddon((_event, url) => send({ kind: 'url', url })));
  terminal.open(host);
  terminal.registerLinkProvider({ provideLinks(y, callback) {
    const line = terminal.buffer.active.getLine(y - 1)?.translateToString(true) || '';
    callback(fileLinks(line).map(link => ({ text: line.slice(link.start, link.end), range: { start: { x: link.start + 1, y }, end: { x: link.end, y } }, activate: () => send({ kind: 'file', path: link.path, line: link.line, column: link.column }) })));
  } });
  terminal.onData(data => send({ kind: 'input', data }));
  terminal.onResize(({ cols, rows }) => send({ kind: 'resize', cols, rows }));
  terminal.onSelectionChange(() => send({ kind: 'selection', text: terminal.getSelection() }));
  terminal.attachCustomKeyEventHandler(event => {
    if (event.type === 'keydown' && (event.ctrlKey || event.metaKey) && event.key.toLowerCase() === 'c' && terminal.hasSelection()) {
      event.preventDefault(); navigator.clipboard?.writeText(terminal.getSelection()).catch(() => {}); return false;
    }
    // Let workspace terminal toggle shortcut reach the window listener.
    if ((event.ctrlKey || event.metaKey) && event.key === '`') return false;
    return true;
  });
  const observer = new ResizeObserver(() => { if (host.clientWidth && host.clientHeight) fit.fit(); });
  observer.observe(host);
  const appearanceObserver = new MutationObserver(() => {
    const session = sessions.get(id); if (session) setAppearance(id, session.theme, session.font, session.size);
  });
  appearanceObserver.observe(document.documentElement, { attributes: true, attributeFilter: ['data-theme', 'class', 'style'] });
  sessions.set(id, { terminal, fit, observer, appearanceObserver, theme, destroyed: false });
  fit.fit(); terminal.focus();

}
export function setAppearance(id: string, theme: Partial<Theme>, font?: string, size?: number) {
  const session = sessions.get(id); if (!session) return;
  session.theme = theme; session.font = font; session.size = size;
  const host = session.terminal.element; if (!host) return;
  const styles = getComputedStyle(host);
  session.terminal.options.theme = {
    background: styles.getPropertyValue('--terminal-background').trim() || '#111820',
    foreground: styles.getPropertyValue('--terminal-foreground').trim() || '#dce5ed',
    cursor: styles.getPropertyValue('--terminal-cursor').trim() || '#dce5ed',
    selectionBackground: styles.getPropertyValue('--terminal-selection').trim() || '#45556b', ...theme,
  };
  session.terminal.options.fontFamily = font || resolvedFont(styles);
  session.terminal.options.fontSize = size || 13;
  session.fit.fit();
}
export function write(id: string, dataBase64: string): Promise<void> {
  const session = sessions.get(id);
  if (!session) return Promise.reject(new Error('Terminal not mounted'));
  const data = Uint8Array.from(atob(dataBase64), char => char.charCodeAt(0));
  return new Promise(resolve => session.terminal.write(data, resolve));
}
export function reset(id: string) {
  sessions.get(id)?.terminal.reset();
}
export function destroy(id: string) {
  const session = sessions.get(id); if (!session) return;
  session.destroyed = true;
  session.observer.disconnect(); session.appearanceObserver.disconnect(); session.terminal.dispose(); sessions.delete(id);
}
