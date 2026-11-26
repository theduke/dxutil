const id = await dioxus.recv();
const element = document.getElementById(id);
dioxus.send(element ? {
  start_line: element.value.slice(0, element.selectionStart).split('\n').length,
  end_line: element.value.slice(0, element.selectionEnd).split('\n').length,
  text: element.value.slice(element.selectionStart, element.selectionEnd),
} : null);
