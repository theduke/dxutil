import { test, expect } from '@playwright/test';
import { AxeBuilder } from '@axe-core/playwright';
import { open, editor, state, blocks, caret, paste, selectText } from './helpers.js';

test('typing, Enter, Backspace and history publish immediately', async ({ page }) => {
  const errors = await open(page);
  await caret(page);
  await page.keyboard.type('first'); await page.keyboard.press('Enter'); await page.keyboard.type('second');
  await expect.poll(async () => blocks((await state(page)).document).length).toBe(2);
  await page.keyboard.press('Home'); await page.keyboard.press('Backspace');
  await expect.poll(async () => blocks((await state(page)).document).length).toBe(1);
  await expect(editor(page)).toContainText('firstsecond');
  await page.keyboard.press('ControlOrMeta+z');
  await expect.poll(async () => blocks((await state(page)).document).length).toBe(2);
  expect(errors).toEqual([]);
});

test('format shortcuts, hard breaks and Unicode grapheme deletion', async ({ page }) => {
  await open(page); await caret(page);
  await page.keyboard.press('ControlOrMeta+b'); await page.keyboard.type('bold');
  await expect(editor(page).locator('strong')).toHaveText('bold');
  await page.keyboard.press('ControlOrMeta+b'); await page.keyboard.press('Shift+Enter');
  await page.keyboard.insertText('👨‍👩‍👧‍👦'); await page.keyboard.press('Backspace');
  await expect(editor(page)).not.toContainText('👨');
  await expect(editor(page).locator('br')).toHaveCount(1);
});

test('block selection replaces text and Escape then Enter restores a caret', async ({ page }) => {
  await open(page); await caret(page); await page.keyboard.type('original');
  await page.keyboard.press('Escape');
  await expect(editor(page).locator('.dx-block[data-selected]')).toHaveCount(1);
  await page.keyboard.press('Enter'); await page.keyboard.type(' tail');
  await expect(editor(page)).toHaveText('original tail');
  await page.keyboard.press('Escape'); await page.keyboard.type('replacement');
  await expect(editor(page)).toHaveText('replacement');
});

test('slash filtering and keyboard acceptance keep editor focus', async ({ page }) => {
  await open(page); await caret(page); await page.keyboard.type('/heading');
  const list = page.getByRole('listbox'); await expect(list).toBeVisible();
  await expect(editor(page)).toBeFocused();
  await expect.poll(async () => list.evaluate(element => {
    const paragraph = document.querySelector('[data-content-kind=paragraph]')!;
    const walker = document.createTreeWalker(paragraph, NodeFilter.SHOW_TEXT);
    let text: Node | null = null;
    while (walker.nextNode()) text = walker.currentNode;
    const range = document.createRange();
    range.setStart(text!, text!.textContent!.length); range.collapse(true);
    const caret = range.getBoundingClientRect(), popup = element.getBoundingClientRect();
    return Math.max(Math.abs(popup.left - caret.left), Math.abs(popup.top - caret.bottom));
  })).toBeLessThan(24);
  await page.keyboard.press('ArrowDown'); await page.keyboard.press('ArrowUp'); await page.keyboard.press('Enter');
  await expect(editor(page).locator('[data-content-kind=heading]')).toHaveCount(1);
  await expect(editor(page)).not.toContainText('/heading');
});

test('Escape dismisses a mention until the query changes', async ({ page }) => {
  await open(page); await caret(page); await page.keyboard.type('@Ad');
  await expect(page.getByRole('listbox')).toBeVisible();
  await page.keyboard.press('Escape'); await expect(page.getByRole('listbox')).toHaveCount(0);
  await page.evaluate(() => document.dispatchEvent(new Event('selectionchange')));
  await expect(page.getByRole('listbox')).toHaveCount(0);
  await page.keyboard.type('a'); await expect(page.getByRole('listbox')).toBeVisible();
  await expect(page.getByRole('option',{name:'Ada Harness entity',exact:true})).toBeVisible();
  await page.keyboard.press('Enter');
  await expect(editor(page).locator('[data-semantic-mention]')).toContainText('Ada');
});

test('todo is a native checkbox, remains outside inline content, and is undoable', async ({ page }) => {
  await open(page, '/blocks?fixture=todo');
  const check = editor(page).getByRole('checkbox');
  await expect(check).not.toBeChecked(); await check.check(); await expect(check).toBeChecked();
  await page.getByRole('button', { name: 'Undo', exact: true }).click(); await expect(check).not.toBeChecked();
  await page.getByRole('button', { name: 'Toggle read only' }).click();
  await expect(page.locator('.dxeditor-engine__content').getByRole('checkbox')).toBeDisabled();
});

test('toggle disclosure controls children without persisting collapsed state', async ({ page }) => {
  await open(page, '/blocks?fixture=toggle');
  const toggle = editor(page).locator('.dx-block-native-control button');
  await expect(toggle).toHaveAttribute('aria-expanded', 'true');
  await toggle.click(); await expect(toggle).toHaveAttribute('aria-expanded', 'false');
  await expect(editor(page).getByText('Nested child')).not.toBeVisible();
  expect((await state(page)).changes).toBe(0);
});

test('table Enter is inline, Tab from the last cell adds one row', async ({ page }) => {
  await open(page, '/blocks?fixture=table');
  const cells = editor(page).locator('td,th');
  await caret(page, cells.last()); await page.keyboard.press('Enter'); await page.keyboard.type('more');
  await expect(cells.last()).toContainText('more');
  await expect(editor(page).locator('tr')).toHaveCount(2);
  await page.keyboard.press('Tab'); await expect(editor(page).locator('tr')).toHaveCount(3);
});

test('TSV paste grows tables in a single undo step', async ({ page }) => {
  await open(page, '/blocks?fixture=table'); await caret(page, editor(page).locator('td,th').last());
  await paste(page, { 'text/plain': 'a\tb\tc\nd\te\tf' });
  await expect(editor(page).locator('tr')).toHaveCount(3);
  await expect(editor(page).locator('tr').last().locator('td,th')).toHaveCount(4);
  await page.getByRole('button', { name: 'Undo', exact: true }).click();
  await expect(editor(page).locator('tr')).toHaveCount(2);
  await expect(editor(page).locator('td,th')).toHaveCount(4);
});

test('HTML paste replaces selected text and sanitizes unsafe links', async ({ page }) => {
  await open(page); await caret(page); await page.keyboard.type('replace me');
  await selectText(editor(page).locator('[data-content-kind=paragraph]'));
  await paste(page, { 'text/html': '<p><strong>Safe</strong><script>alert(1)</script><a href="javascript:alert(1)"> link</a></p>', 'text/plain': 'Safe link' });
  await expect(editor(page)).toContainText('Safe'); await expect(editor(page)).not.toContainText('replace me');
  await expect(editor(page).locator('a[href^="javascript:"]')).toHaveCount(0);
  await expect(editor(page).locator('script')).toHaveCount(0);
});

test('multiline plain text preserves order and a selected URL becomes a link', async ({ page }) => {
  await open(page); await caret(page);
  await paste(page, { 'text/plain': 'first\nsecond\nthird' });
  await expect(editor(page)).toContainText('first');
  const first = editor(page).locator('[data-content-kind=paragraph]').first();
  await selectText(first); await paste(page, { 'text/plain': 'https://example.com' });
  await expect(first.locator('a')).toHaveAttribute('href', 'https://example.com');
});

test('file paste creates an upload placeholder and request', async ({ page }) => {
  await open(page); await caret(page);
  await editor(page).evaluate(element => {
    const transfer = new DataTransfer(); transfer.items.add(new File(['image-data'], 'upload.png', {type:'image/png'}));
    const event = new ClipboardEvent('paste', {bubbles:true,cancelable:true,clipboardData:transfer});
    Object.defineProperty(event, 'clipboardData', {value:transfer});
    element.dispatchEvent(event);
  });
  await expect(editor(page).locator('figure')).toHaveCount(1);
  await expect.poll(async () => (await state(page)).events.some((event:any) => event.kind === 'filesPasted')).toBe(true);
});

test('island adoption preserves focused input through attribute save and undo', async ({ page }) => {
  await open(page, '/islands');
  const island = editor(page).getByTestId('island'); await expect(island).toBeVisible();
  const input = island.getByRole('textbox', {name:'Island input'}); await input.fill('retained');
  await island.getByRole('button', {name:'Update island'}).click();
  await expect(island.getByRole('heading')).toHaveText('Updated island');
  await expect(input).toHaveValue('retained');
  await page.getByRole('button', {name:'Undo',exact:true}).click();
  await expect(island.getByRole('heading')).toHaveText('Island'); await expect(input).toHaveValue('retained');
});

test('notes preserve nested lists, code info and opaque source on neighbor edit', async ({ page }) => {
  await open(page, '/notes');
  await caret(page, editor(page).locator('[data-content-kind=paragraph]').first()); await page.keyboard.type(' changed');
  await expect.poll(async () => (await state(page)).notes).toContain('changed');
  const notes = (await state(page)).notes;
  expect(notes).toContain('rust edition=2024'); expect(notes).toContain('<section>opaque</section>'); expect(notes).toContain('- [x] done');
  await expect(page.locator('[role=status]')).not.toContainText('externally');
});

test('reset/preserve replacements do not emit changes; reset clears history', async ({ page }) => {
  await open(page); await caret(page); await page.keyboard.type('typed');
  const before = (await state(page)).changes;
  await page.getByRole('button', {name:'Reset document',exact:true}).click();
  await expect(editor(page)).toContainText('Replaced');
  await page.getByRole('button', {name:'Undo',exact:true}).click(); await expect(editor(page)).toContainText('Replaced');
  expect((await state(page)).changes).toBe(before);
  await page.getByRole('button', {name:'Preserve document',exact:true}).click(); await expect(editor(page)).toContainText('Preserved');
  expect((await state(page)).changes).toBe(before);
});

test('base states satisfy axe and expose accessible editing semantics', async ({ page }) => {
  for (const route of ['/blocks', '/blocks?fixture=todo', '/composer', '/notes']) {
    await open(page, route);
    await expect(page.locator('.dxeditor-engine__content')).toHaveAttribute('role', 'textbox');
    await expect(page.locator('.dxeditor-engine__content')).toHaveAttribute('aria-multiline', 'true');
    const report = await new AxeBuilder({page}).analyze(); expect(report.violations).toEqual([]);
  }
});

test('mobile and scaled wrappers keep contextual surfaces on screen', async ({ page }) => {
  await page.setViewportSize({width:390,height:800}); await open(page);
  await page.locator('.dxeditor').evaluate(element => (element as HTMLElement).style.transform = 'scale(.8)');
  await caret(page); await page.keyboard.type('/table');
  const list = page.getByRole('listbox'); await expect(list).toBeVisible();
  const bounds = await list.boundingBox(); expect(bounds).not.toBeNull();
  expect(bounds!.x).toBeGreaterThanOrEqual(-1); expect(bounds!.x+bounds!.width).toBeLessThanOrEqual(391);
  await page.keyboard.press('Enter'); await expect(editor(page).locator('table')).toHaveCount(1);
});

for (const route of ['/blocks', '/composer']) {
  test(`select all covers every paragraph and stays selected in ${route}`, async ({ page }) => {
    await open(page, route); await caret(page);
    await page.keyboard.type('first');
    await page.keyboard.press(route === '/composer' ? 'Shift+Enter' : 'Enter');
    await page.keyboard.type('second');
    for (let i = 0; i < 3; i++) {
      await page.keyboard.press('ControlOrMeta+a');
      await expect.poll(() => page.evaluate(() => window.getSelection()?.toString().replace(/\s+/g, ' ').trim())).toBe('first second');
    }
    await page.keyboard.type('replacement');
    await expect(editor(page)).toHaveText('replacement');
  });
}

test('slash list commands show markers on empty and populated items', async ({ page }) => {
  await open(page); await caret(page);
  await page.keyboard.type('/bullet'); await page.keyboard.press('Enter');
  const item = editor(page).locator('.dx-block[data-list=bullet]');
  await expect(item).toHaveCount(1);
  await expect.poll(() => item.evaluate(el => getComputedStyle(el, '::before').content)).toBe('"•"');
  await page.keyboard.type('List item');
  await expect.poll(() => item.evaluate(el => getComputedStyle(el, '::before').content)).toBe('"•"');
  await page.keyboard.press('ControlOrMeta+Alt+0');
  await expect(editor(page).locator('.dx-block[data-list]')).toHaveCount(0);
  await page.keyboard.press('ControlOrMeta+z');
  await expect(item).toHaveCount(1);
  await page.keyboard.press('ControlOrMeta+Shift+7');
  const numbered = editor(page).locator('.dx-block[data-list=numbered]');
  await expect(numbered).toHaveAttribute('data-number', '1.');
  await page.keyboard.press('Enter'); await page.keyboard.type('Second item');
  await expect(numbered.nth(1)).toHaveAttribute('data-number', '2.');
  await caret(page, numbered.first());
  await page.keyboard.press('ControlOrMeta+Alt+0');
  await expect(numbered).toHaveCount(1);
  await expect(numbered).toHaveAttribute('data-number', '1.');
});

for (const collapsed of [false, true]) {
  test(`select all preserves nested blocks when copied, replaced and undone (collapsed=${collapsed})`, async ({ page }) => {
    await open(page, '/blocks?fixture=toggle');
    await caret(page, editor(page).getByText('After toggle'));
    await page.keyboard.press('Escape'); await page.keyboard.press('Backspace');
    if (collapsed) await editor(page).locator('.dx-block-native-control button').click();
    await caret(page, editor(page).locator('[data-content-kind=toggle]'));
    const before = (await state(page)).document;
    for (let i = 0; i < 3; i++) {
      await page.keyboard.press('ControlOrMeta+a');
      await expect(editor(page).locator('.dx-block[data-selected]')).toHaveCount(2);
    }
    const copied = await editor(page).evaluate(element => {
      const transfer = new DataTransfer();
      const event = new ClipboardEvent('copy', { bubbles: true, cancelable: true, clipboardData: transfer });
      Object.defineProperty(event, 'clipboardData', { value: transfer });
      element.dispatchEvent(event);
      return JSON.parse(transfer.getData('application/x-semantic-dxeditor+json')).slice;
    });
    expect(copied.content).toHaveLength(1);
    const children = copied.content[0].content.find((node: any) => node.kind === 'block_children');
    expect(children.content).toHaveLength(1);
    await page.keyboard.type('X');
    await expect(editor(page)).toHaveText('X');
    await page.keyboard.press('ControlOrMeta+z');
    await expect.poll(async () => (await state(page)).document).toEqual(before);
  });
}
