import { test, expect, type Page } from '@playwright/test';
import { open, editor, caret, state, blocks } from './helpers.js';

async function setup(page: Page, count = 4) {
  await open(page);
  await caret(page);
  for (let index = 0; index < count; index++) {
    if (index) await page.keyboard.press('Enter');
    await page.keyboard.type(`Block ${index + 1}`);
  }
  return blocks((await state(page)).document).map((block: any) => block.id) as string[];
}

const block = (page: Page, uid: string) => editor(page).locator(`.dx-block[data-uid="${uid}"]`);
const grip = (page: Page) => page.getByRole('button', { name: 'Block actions', exact: true });
const indicator = (page: Page) => page.locator('.dx-block-drop-indicator');
const order = async (page: Page) => blocks((await state(page)).document).map((block: any) => block.id);

async function startDrag(page: Page, uid: string) {
  await block(page, uid).locator(':scope > :first-child').hover();
  await expect(grip(page)).toHaveAttribute('data-dxeditor-drag', uid);
  const bounds = (await grip(page).boundingBox())!;
  await page.mouse.move(bounds.x + bounds.width / 2, bounds.y + bounds.height / 2);
  await page.mouse.down();
  await page.mouse.move(bounds.x + bounds.width / 2 + 10, bounds.y + bounds.height / 2, { steps: 5 });
}

async function moveTo(page: Page, uid: string, after: boolean, gutter = false) {
  const bounds = (await block(page, uid).boundingBox())!;
  const x = bounds.x + (gutter ? -16 : 24);
  const y = bounds.y + (after ? bounds.height - 2 : 2);
  await page.mouse.move(x, y, { steps: 12 });
  await page.mouse.move(x + 1, y);
  await expect(indicator(page)).toBeVisible();
  const line = (await indicator(page).boundingBox())!;
  expect(Math.abs(line.x - bounds.x)).toBeLessThan(2);
  expect(Math.abs(line.width - bounds.width)).toBeLessThan(2);
  expect(Math.abs(line.y + line.height / 2 - (bounds.y + (after ? bounds.height : 0)))).toBeLessThan(2);
}

test('compact handles and real pointer dragging reorder blocks with a full-width indicator and undo', async ({ page }, testInfo) => {
  const ids = await setup(page);
  await block(page, ids[0]).hover();
  const style = await grip(page).evaluate(element => {
    const style = getComputedStyle(element);
    return { width: style.width, height: style.height, border: style.borderWidth, shadow: style.boxShadow, background: style.backgroundColor, cursor: style.cursor };
  });
  expect(style).toEqual({ width: '24px', height: '28px', border: '0px', shadow: 'none', background: 'rgba(0, 0, 0, 0)', cursor: 'grab' });
  await testInfo.attach('compact-handles', { body: await page.screenshot(), contentType: 'image/png' });
  await startDrag(page, ids[0]);
  await moveTo(page, ids[2], true);
  await expect(block(page, ids[0])).toHaveAttribute('data-dragging', 'true');
  await testInfo.attach('drag-insertion-indicator', { body: await page.screenshot(), contentType: 'image/png' });
  await page.mouse.up();
  await expect.poll(() => order(page)).toEqual([ids[1], ids[2], ids[0], ids[3]]);
  await expect(indicator(page)).toHaveCount(0);
  await expect(page.locator('.dx-block-drag-ghost,[data-dragging],[data-block-dragging]')).toHaveCount(0);
  await expect(page.getByRole('menu', { name: 'Block actions' })).toHaveCount(0);
  await page.getByRole('button', { name: 'Undo', exact: true }).click();
  await expect.poll(() => order(page)).toEqual(ids);
  await page.getByRole('button', { name: 'Redo', exact: true }).click();
  await expect.poll(() => order(page)).toEqual([ids[1], ids[2], ids[0], ids[3]]);
});

test('drag upward through the gutter and before the first block', async ({ page }) => {
  const ids = await setup(page);
  await startDrag(page, ids[3]);
  await moveTo(page, ids[0], false, true);
  await page.mouse.up();
  await expect.poll(() => order(page)).toEqual([ids[3], ...ids.slice(0, 3)]);
});

test('dragging selected blocks preserves their order and moves them in one undo step', async ({ page }) => {
  const ids = await setup(page);
  await caret(page, block(page, ids[0]).locator('[data-inline-container]'));
  await page.keyboard.press('Escape');
  await page.keyboard.press('Shift+ArrowDown');
  await startDrag(page, ids[0]);
  await moveTo(page, ids[3], true);
  await page.mouse.up();
  await expect.poll(() => order(page)).toEqual([ids[2], ids[3], ids[0], ids[1]]);
  await page.getByRole('button', { name: 'Undo', exact: true }).click();
  await expect.poll(() => order(page)).toEqual(ids);
});

test('Escape and dropping outside cancel without changing the document', async ({ page }) => {
  const ids = await setup(page);
  await startDrag(page, ids[0]);
  await moveTo(page, ids[2], true);
  await page.keyboard.press('Escape');
  await page.mouse.up();
  await expect(indicator(page)).toHaveCount(0);
  await expect(page.locator('.dx-block-drag-ghost,[data-dragging],[data-block-dragging]')).toHaveCount(0);
  expect(await order(page)).toEqual(ids);
  await startDrag(page, ids[0]);
  await moveTo(page, ids[2], true);
  await page.mouse.move(8, 8, { steps: 8 });
  await expect(indicator(page)).toHaveCount(0);
  await page.mouse.up();
  expect(await order(page)).toEqual(ids);
  await expect(page.locator('.dx-block-drag-ghost,[data-dragging],[data-block-dragging]')).toHaveCount(0);
});

test('a parent cannot be dropped into its own descendants', async ({ page }) => {
  await open(page, '/blocks?fixture=toggle');
  const before = (await state(page)).document;
  const parent = blocks(before)[0].id;
  const child = blocks(before)[0].content[1].content[0].id;
  await startDrag(page, parent);
  const bounds = (await block(page, child).boundingBox())!;
  await page.mouse.move(bounds.x + 30, bounds.y + bounds.height / 2, { steps: 12 });
  await expect(indicator(page)).toHaveCount(0);
  await page.mouse.up();
  expect((await state(page)).document).toEqual(before);
});

test('a whole subtree moves intact and remains undoable', async ({ page }) => {
  await open(page, '/blocks?fixture=toggle');
  const before = (await state(page)).document;
  const ids = blocks(before).map((node: any) => node.id);
  await startDrag(page, ids[0]);
  await moveTo(page, ids[1], true);
  await page.mouse.up();
  await expect.poll(() => order(page)).toEqual([ids[1], ids[0]]);
  expect(blocks((await state(page)).document)[1]).toEqual(blocks(before)[0]);
  await page.getByRole('button', { name: 'Undo', exact: true }).click();
  await expect.poll(async () => (await state(page)).document).toEqual(before);
});

test('scaled editors keep drag targets and indicators aligned', async ({ page }) => {
  const ids = await setup(page);
  await page.locator('.dxeditor').evaluate(element => (element as HTMLElement).style.transform = 'scale(.8)');
  await startDrag(page, ids[3]);
  await moveTo(page, ids[0], false);
  await page.mouse.up();
  await expect.poll(() => order(page)).toEqual([ids[3], ...ids.slice(0, 3)]);
});

test('nesting draws an indented line below the parent content', async ({ page }) => {
  await open(page, '/blocks?fixture=toggle');
  const before = (await state(page)).document;
  const ids = blocks(before).map((node: any) => node.id);
  await startDrag(page, ids[1]);
  const header = (await block(page, ids[0]).locator(':scope > :first-child').boundingBox())!;
  const x = header.x + header.width * .75;
  const y = header.y + header.height / 2;
  await page.mouse.move(x, y, { steps: 12 });
  await page.mouse.move(x + 1, y);
  await expect(indicator(page)).toBeVisible();
  const line = (await indicator(page).boundingBox())!;
  expect(Math.abs(line.x - header.x - 24)).toBeLessThan(2);
  expect(Math.abs(line.y + line.height / 2 - header.y - header.height)).toBeLessThan(2);
  await page.mouse.up();
  await expect.poll(async () => blocks((await state(page)).document).length).toBe(1);
  const children = blocks((await state(page)).document)[0].content[1].content;
  expect(children[0].id).toBe(ids[1]);
  expect(children[1]).toEqual(blocks(before)[0].content[1].content[0]);
  await page.getByRole('button', { name: 'Undo', exact: true }).click();
  await expect.poll(async () => (await state(page)).document).toEqual(before);
});

test('dropping on the source leaves content and change notifications untouched', async ({ page }) => {
  const ids = await setup(page);
  const changes = (await state(page)).changes;
  await startDrag(page, ids[0]);
  const bounds = (await block(page, ids[0]).boundingBox())!;
  await page.mouse.move(bounds.x + 24, bounds.y + bounds.height / 2, { steps: 8 });
  await expect(indicator(page)).toHaveCount(0);
  await page.mouse.up();
  expect(await order(page)).toEqual(ids);
  expect((await state(page)).changes).toBe(changes);
});

test('dragging near an editor scroll edge scrolls the container and allows a drop', async ({ page }) => {
  const ids = await setup(page, 18);
  await caret(page, block(page, ids[0]).locator('[data-inline-container]'));
  const scroller = page.locator('.dxeditor__document');
  await scroller.evaluate(element => {
    (element as HTMLElement).style.cssText += ';max-height:180px;min-height:0;overflow:auto';
    element.scrollTop = 0;
  });
  await startDrag(page, ids[0]);
  const bounds = (await scroller.boundingBox())!;
  await page.mouse.move(bounds.x + 80, bounds.y + bounds.height - 6, { steps: 12 });
  await page.mouse.move(bounds.x + 81, bounds.y + bounds.height - 6);
  await expect.poll(() => scroller.evaluate(element => element.scrollTop)).toBeGreaterThan(0);
  await expect(indicator(page)).toBeVisible();
  await page.mouse.up();
  await expect.poll(() => order(page)).not.toEqual(ids);
  await page.getByRole('button', { name: 'Undo', exact: true }).click();
  await expect.poll(() => order(page)).toEqual(ids);
});

test('dropping into the bottom padding appends after the last block', async ({ page }) => {
  const ids = await setup(page);
  await startDrag(page, ids[0]);
  const last = (await block(page, ids[3]).boundingBox())!;
  await page.mouse.move(last.x + 24, last.y + last.height + 12, { steps: 12 });
  await page.mouse.move(last.x + 25, last.y + last.height + 12);
  await expect(indicator(page)).toBeVisible();
  await page.mouse.up();
  await expect.poll(() => order(page)).toEqual([...ids.slice(1), ids[0]]);
});

test('read-only documents expose no drag handles and reject internal drag events', async ({ page }) => {
  const ids = await setup(page);
  await page.getByRole('button', { name: 'Toggle read only' }).click();
  await page.locator('.dxeditor-engine__content .dx-block').first().hover();
  await expect(grip(page)).toHaveCount(0);
  const before = (await state(page)).document;
  await page.locator('.dxeditor-engine__content').evaluate((element, uid) => {
    const source = document.createElement('button');
    source.dataset.dxeditorDrag = uid;
    element.closest('.dxeditor')!.appendChild(source);
    const data = new DataTransfer();
    const event = new DragEvent('dragstart', { bubbles: true, cancelable: true, dataTransfer: data });
    source.dispatchEvent(event);
    source.remove();
  }, ids[0]);
  expect((await state(page)).document).toEqual(before);
  await expect(page.locator('.dx-block-drag-ghost,[data-dragging],[data-block-dragging]')).toHaveCount(0);
});

test('mobile handles fit in the gutter and remain usable for block actions', async ({ page }, testInfo) => {
  await page.setViewportSize({ width: 390, height: 844 });
  const ids = await setup(page);
  await block(page, ids[0]).hover();
  const addBounds = (await page.getByRole('button', { name: 'Insert block', exact: true }).boundingBox())!;
  const gripBounds = (await grip(page).boundingBox())!;
  const contentBounds = (await block(page, ids[0]).boundingBox())!;
  expect(addBounds.x).toBeGreaterThanOrEqual(0);
  expect(gripBounds.y).toBe(addBounds.y);
  expect(gripBounds.x + gripBounds.width).toBeLessThan(contentBounds.x);
  await testInfo.attach('mobile-handles', { body: await page.screenshot(), contentType: 'image/png' });
  await grip(page).click();
  await expect(page.getByRole('menu', { name: 'Block actions' })).toBeVisible();
});
