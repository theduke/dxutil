import { test, expect, type Page, type Locator } from '@playwright/test';

declare global { interface Window { __dxeditorHarness: any } }
const errorsByPage = new WeakMap<Page, string[]>();
test.afterEach(async ({page}) => {
  expect(errorsByPage.get(page) || [], 'No uncaught browser errors').toEqual([]);
});
export const editor = (page: Page) => page.locator('[contenteditable=true].dxeditor-engine__content').first();
export async function open(page: Page, route = '/blocks') {
  let errors = errorsByPage.get(page);
  if (!errors) {
    errors = [];
    errorsByPage.set(page, errors);
    const observed = errors;
    page.on('pageerror', error => observed.push(error.message));
  }
  await page.goto(route);
  await expect(page.locator('.dxeditor-engine__content')).toBeVisible();
  await expect.poll(() => page.evaluate(() => !!window.__dxeditorHarness)).toBe(true);
  return errors;
}
export async function state(page: Page) { return page.evaluate(() => window.__dxeditorHarness); }
export function blocks(doc: any): any[] { return doc.root.content[0].content; }
export async function caret(page: Page, target?: Locator, end = true) {
  const locator = target || editor(page);
  await locator.evaluate((element, end) => {
    (element.closest('[contenteditable]') as HTMLElement)?.focus();
    const range = document.createRange(); range.selectNodeContents(element); range.collapse(!end);
    const selection = window.getSelection()!; selection.removeAllRanges(); selection.addRange(range);
    document.dispatchEvent(new Event('selectionchange'));
  }, end);
}
export async function paste(page: Page, types: Record<string, string>) {
  await editor(page).evaluate((element, types) => {
    const transfer = new DataTransfer();
    for (const [type, data] of Object.entries(types)) transfer.setData(type, data);
    const event = new ClipboardEvent('paste', { bubbles: true, cancelable: true, clipboardData: transfer });
    // Firefox discards clipboardData supplied to untrusted constructor events.
    // Supply the synthetic payload explicitly; real paste events carry it natively.
    Object.defineProperty(event, 'clipboardData', { value: transfer });
    element.dispatchEvent(event);
  }, types);
}
export async function selectText(locator: Locator) {
  await locator.evaluate(element => {
    (element.closest('[contenteditable]') as HTMLElement)?.focus();
    const range = document.createRange(); range.selectNodeContents(element);
    const selection = window.getSelection()!; selection.removeAllRanges(); selection.addRange(range);
    document.dispatchEvent(new Event('selectionchange'));
  });
}
