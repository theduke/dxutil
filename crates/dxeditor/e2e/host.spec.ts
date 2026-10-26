import { test, expect } from '@playwright/test';
import { open, editor, caret, state, selectText, blocks } from './helpers.js';

test('entity suggestions accept an application candidate and preview/open it', async ({page}) => {
  await open(page); await caret(page); await page.keyboard.type('@Ad');
  await expect(page.getByRole('option',{name:'Ada Harness entity',exact:true})).toBeVisible();
  await page.keyboard.press('Enter');
  const mention=editor(page).locator('[data-semantic-mention]'); await expect(mention).toContainText('Ada');
  await mention.hover(); await expect(page.getByRole('dialog',{name:'Entity preview'})).toContainText('Harness preview');
  await mention.click(); await expect.poll(async ()=>(await state(page)).events.some((event:any)=>event.opened==='entity-1')).toBe(true);
});

test('deleted entity previews preserve source labels and prevent opening',async ({page})=>{
  await open(page,'/blocks?fixture=deleted-mention'); const mention=editor(page).locator('[data-semantic-mention]');
  await mention.hover(); await expect(page.getByRole('dialog',{name:'Entity preview'})).toContainText('deleted');
  await expect(mention).toContainText('Ada'); await mention.click();
  expect((await state(page)).events.some((event:any)=>event.opened)).toBe(false);
  expect((await state(page)).changes).toBe(0);
});

test('providerless mentions remain intact without being marked deleted',async ({page})=>{
  await open(page,'/blocks?fixture=no-provider'); const mention=editor(page).locator('[data-semantic-mention]');
  await mention.hover(); await expect(mention).not.toHaveAttribute('data-deleted','true');
  await expect(mention).toContainText('Ada'); await expect(page.getByRole('dialog',{name:'Entity preview'})).toHaveCount(0);
});

test('create from selection captures the range through host completion',async ({page})=>{
  await open(page); await caret(page); await page.keyboard.type('Create this title');
  await selectText(editor(page).locator('[data-content-kind=paragraph]'));
  await page.getByRole('button',{name:'Create from selection…',exact:true}).click();
  await expect.poll(async ()=>(await state(page)).events.some((event:any)=>event.kind==='entityCreateRequest' && event.trigger==='selection' && event.query==='Create this title')).toBe(true);
  await page.getByRole('button',{name:'Complete entity creation',exact:true}).click();
  await expect(editor(page).locator('[data-semantic-mention]')).toContainText('Created entity');
  await expect(editor(page)).not.toContainText('Create this title');
});

test('link selected text through the searchable entity picker', async ({page}) => {
  await open(page); await caret(page); await page.keyboard.type('Linked title');
  await selectText(editor(page).locator('[data-content-kind=paragraph]'));
  await page.getByRole('button', {name:'Link to entity…', exact:true}).click();
  const search = page.getByRole('searchbox', {name:'Search entities', exact:true});
  await expect(search).toBeFocused();
  await search.fill('Ada');
  await expect(page.getByRole('option', {name:'Ada Harness entity', exact:true})).toBeVisible();
  await search.press('Enter');
  await expect(editor(page)).toBeFocused();
  await expect.poll(async () => blocks((await state(page)).document)[0].content[0].content.some((node:any) =>
    node.text === 'Linked title' && node.marks?.some((mark:any) => mark.kind === 'entity_link' && mark.attrs.entity_id === 'entity-1')
  )).toBe(true);
});

test('host-owned mention picker inserts its selected entity',async ({page})=>{
  await open(page,'/blocks?fixture=host-picker'); await caret(page); await page.keyboard.type('@Ad');
  await expect.poll(async ()=>(await state(page)).events.some((event:any)=>event.kind==='entityPickerRequest')).toBe(true);
  await page.getByRole('button',{name:'Complete entity picker',exact:true}).click();
  await expect(editor(page).locator('[data-semantic-mention]')).toContainText('Picked entity');
});

test('task creation promotes children and undo restores the original hierarchy',async ({page})=>{
  await open(page,'/blocks?fixture=task-children');
  await editor(page).locator('[data-content-kind=todo]').hover();
  await page.getByRole('button',{name:'Block actions',exact:true}).click();
  await page.getByRole('menuitem',{name:'Convert to task',exact:true}).click();
  await expect.poll(async ()=>(await state(page)).events.some((event:any)=>event.kind==='entityCreateRequest' && event.trigger==='task' && event.checked===false)).toBe(true);
  const uid=blocks((await state(page)).document)[0].id;
  await page.getByRole('button',{name:'Complete entity creation',exact:true}).click();
  await expect.poll(async ()=>blocks((await state(page)).document)[0].content[0].kind).toBe('entity_embed');
  expect(blocks((await state(page)).document)[0].id).toBe(uid);
  expect(blocks((await state(page)).document)[1].content[0].content[0].text).toBe('Promoted child');
  await page.getByRole('button',{name:'Undo',exact:true}).click();
  await expect(editor(page).locator('[data-content-kind=todo]')).toBeVisible();
  expect(blocks((await state(page)).document)[0].content[1].kind).toBe('block_children');
});

test('slash page creation preserves the empty block identity',async ({page})=>{
  await open(page); const uid=blocks((await state(page)).document)[0].id;
  await caret(page); await page.keyboard.type('/page'); await expect(page.getByRole('listbox')).toBeVisible(); await page.keyboard.press('Enter');
  await expect.poll(async ()=>(await state(page)).events.some((event:any)=>event.kind==='entityCreateRequest' && event.trigger==='page')).toBe(true);
  await page.getByRole('button',{name:'Complete entity creation',exact:true}).click();
  await expect.poll(async ()=>blocks((await state(page)).document)[0].content[0].kind).toBe('entity_embed');
  expect(blocks((await state(page)).document)[0].id).toBe(uid);
});
