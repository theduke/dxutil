import { test, expect } from '@playwright/test';
import { open, editor, caret, state, paste } from './helpers.js';

test('composer typing, chips and submit/background keys', async ({page}) => {
  await open(page,'/composer'); await caret(page); await page.keyboard.type('message');
  await page.getByRole('button',{name:'Insert chip',exact:true}).click();
  await expect(editor(page).locator('[data-composer-chip]')).toContainText('Ada');
  await caret(page); await page.keyboard.press('Shift+Enter'); await page.keyboard.type('line');
  await page.keyboard.press('ControlOrMeta+Alt+Enter');
  await expect.poll(async () => (await state(page)).events.join(' ')).toContain('BackgroundSubmit');
  await page.keyboard.press('ControlOrMeta+Enter');
  await expect.poll(async () => (await state(page)).events.join(' ')).toContain('Submit');
});

test('composer suggestion acceptance inserts a chip and Escape remembers dismissal', async ({page}) => {
  await open(page,'/composer'); await caret(page); await page.keyboard.type('/Attach');
  await expect(page.getByRole('listbox')).toBeVisible(); await expect(editor(page)).toBeFocused();
  await expect(page.getByRole('option').filter({hasText:'Attach Ada'})).toBeVisible();
  await page.keyboard.press('Enter'); await expect(editor(page).locator('[data-composer-chip]')).toContainText('Ada');
  await page.keyboard.type(' /Help'); await expect(page.getByRole('listbox')).toBeVisible();
  await page.keyboard.press('Escape'); await expect(page.getByRole('listbox')).toHaveCount(0);
});

test('composer Backspace selects then removes chips, structured paste restores them', async ({page}) => {
  await open(page,'/composer'); await page.getByRole('button',{name:'Insert chip',exact:true}).click();
  await caret(page); await page.keyboard.press('Backspace');
  await expect(editor(page).locator('[data-composer-chip]')).toHaveCount(1);
  await page.keyboard.press('Backspace'); await expect(editor(page).locator('[data-composer-chip]')).toHaveCount(0);
  await paste(page,{'application/x-semantic-composer+json':JSON.stringify({segments:[{Chip:{kind:'entity',label:'Ada',payload:{id:'entity-1'}}}],attachments:[]}), 'text/plain':'Ada'});
  await expect(editor(page).locator('[data-composer-chip]')).toHaveCount(1);
});
