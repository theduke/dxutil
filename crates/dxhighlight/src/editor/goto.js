const { id, target } = await dioxus.recv();
const element = document.getElementById(id);
if (element) {
  const lines = element.value.split('\n');
  const line = Math.min(Math.max(target.line - 1, 0), lines.length - 1);
  const start = lines.slice(0, line).reduce((offset, text) => offset + text.length + 1, 0);
  const column = Math.min(Math.max((target.column ?? 1) - 1, 0), lines[line].length);
  element.focus();
  element.setSelectionRange(start + column, start + column);
  const lineHeight = parseFloat(getComputedStyle(element).lineHeight) || 20;
  element.scrollTop = Math.max(0, line * lineHeight - element.clientHeight / 2);
}
