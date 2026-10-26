import { test, expect } from '@playwright/test';
import { open, editor, caret, state, selectText } from './helpers.js';

test('notes image URL, alt and title can be inserted and edited losslessly',async ({page})=>{
  await open(page,'/notes'); await caret(page,editor(page).locator('[data-content-kind=paragraph]').last());
  await page.keyboard.type('/image'); await expect(page.getByRole('option').filter({hasText:'Image'})).toBeVisible(); await page.keyboard.press('Enter');
  const dialog=page.getByRole('dialog',{name:'Image properties'}); await expect(dialog).toBeVisible();
  await dialog.getByRole('textbox',{name:'Image URL',exact:true}).fill('https://example.com/photo.png');
  await dialog.getByRole('textbox',{name:'Image alt text',exact:true}).fill('Typed alt');
  await dialog.getByRole('textbox',{name:'Image title',exact:true}).fill('Original title');
  await dialog.getByRole('button',{name:'Apply image',exact:true}).click();
  await expect.poll(async ()=>(await state(page)).notes).toContain('![Typed alt](https://example.com/photo.png "Original title")');
  await editor(page).locator('img').click(); await page.getByRole('button',{name:'Edit image URL and title',exact:true}).click();
  await dialog.getByRole('textbox',{name:'Image URL',exact:true}).fill('https://example.com/changed.png');
  await dialog.getByRole('textbox',{name:'Image title',exact:true}).fill('Updated title');
  await dialog.getByRole('button',{name:'Apply image',exact:true}).click();
  await expect.poll(async ()=>(await state(page)).notes).toContain('![Typed alt](https://example.com/changed.png "Updated title")');
});

test('notes link titles survive formatting and an unsafe URL is rejected',async ({page})=>{
  await open(page,'/notes'); await selectText(editor(page).locator('[data-content-kind=paragraph]').first());
  await page.getByRole('button',{name:'Link',exact:true}).click(); const dialog=page.getByRole('dialog',{name:'Edit link'});
  await dialog.getByRole('textbox',{name:'Link URL',exact:true}).fill('javascript:alert(1)'); await dialog.getByRole('button',{name:'Apply link',exact:true}).click();
  await expect(dialog.getByRole('alert')).toBeVisible();
  await dialog.getByRole('textbox',{name:'Link URL',exact:true}).fill('https://example.com/doc');
  await dialog.getByRole('textbox',{name:'Link title',exact:true}).fill('Document title');
  await dialog.getByRole('button',{name:'Apply link',exact:true}).click();
  await expect.poll(async ()=>(await state(page)).notes).toContain('https://example.com/doc "Document title"');
  expect((await state(page)).notes).not.toContain('javascript:');
});

test('notes Mod-K focuses the URL field and Enter applies the selected link', async ({page}) => {
  await open(page, '/notes');
  const paragraph = editor(page).locator('[data-content-kind=paragraph]').first();
  await selectText(paragraph); await page.keyboard.type('Keyboard link'); await selectText(paragraph);
  await page.keyboard.press('ControlOrMeta+k');
  const url = page.getByRole('textbox', {name:'Link URL', exact:true});
  await expect(url).toBeFocused();
  await page.keyboard.type('https://example.com/keyboard'); await url.press('Home');
  await expect(url).toBeFocused();
  expect(await url.evaluate((element:HTMLInputElement)=>element.selectionStart)).toBe(0);
  await url.press('End');
  expect(await url.evaluate((element:HTMLInputElement)=>element.selectionStart)).toBe('https://example.com/keyboard'.length);
  await page.keyboard.press('Enter');
  await expect.poll(async () => (await state(page)).notes).toContain('[Keyboard link](https://example.com/keyboard)');
});
