import { test, expect } from '@playwright/test';
import { open, editor, caret, state, blocks, selectText } from './helpers.js';

for(const kind of ['paragraph','heading','bulleted_list_item','numbered_list_item','todo','toggle','quote','callout','code_block']) {
  test(`clicking ${kind} after block selection restores text editing`,async ({page})=>{
    await open(page,'/blocks?fixture=all'); const content=editor(page).locator(`[data-content-kind=${kind}]`).first();
    await caret(page,content); await page.keyboard.press('Escape'); await content.click(); await page.keyboard.type('X');
    await expect(content).toContainText('X');
  });
}

test('block duplicate refreshes identities and undo restores the original',async ({page})=>{
  await open(page); await caret(page); await page.keyboard.type('Duplicate me');
  const first=blocks((await state(page)).document)[0].id;
  await editor(page).locator('[data-content-kind=paragraph]').hover(); await page.getByRole('button',{name:'Block actions',exact:true}).click();
  await page.getByRole('menuitem',{name:'Duplicate',exact:true}).click();
  await expect.poll(async ()=>blocks((await state(page)).document).length).toBe(2);
  const ids=blocks((await state(page)).document).map((block:any)=>block.id); expect(new Set(ids).size).toBe(2); expect(ids[0]).toBe(first);
  await page.getByRole('button',{name:'Undo',exact:true}).click(); await expect.poll(async ()=>blocks((await state(page)).document).length).toBe(1);
});

test('plain table menu toggles both headers and edits explicit axes',async ({page})=>{
  await open(page,'/blocks?fixture=table'); await caret(page,editor(page).locator('td,th').first());
  await page.getByRole('button',{name:'Row 1 actions',exact:true}).click();
  await page.getByRole('menuitemcheckbox',{name:/^Header column\b/}).click();
  await expect.poll(async ()=>blocks((await state(page)).document)[0].content[0].attrs.header_column).toBe(true);
  await page.getByRole('button',{name:'Row 1 actions',exact:true}).click();
  await page.getByRole('menuitem',{name:'Insert below',exact:true}).click(); await expect(editor(page).locator('tr')).toHaveCount(3);
  await page.getByRole('button',{name:'Undo',exact:true}).click(); await expect(editor(page).locator('tr')).toHaveCount(2);
});

test('table row drag reorders rows in one undoable transaction',async ({page})=>{
  await open(page,'/blocks?fixture=table'); await caret(page,editor(page).locator('td,th').first());
  const before=blocks((await state(page)).document)[0].content[0].content.map((row:any)=>row.id);
  const source=page.getByRole('button',{name:'Row 1 actions',exact:true});
  const target=editor(page).locator('tr').last(); const bounds=(await target.boundingBox())!;
  const transfer=await page.evaluateHandle(()=>new DataTransfer());
  await source.dispatchEvent('dragstart',{dataTransfer:transfer});
  await target.dispatchEvent('dragover',{dataTransfer:transfer,clientX:bounds.x+20,clientY:bounds.y+bounds.height-1});
  await target.dispatchEvent('drop',{dataTransfer:transfer,clientX:bounds.x+20,clientY:bounds.y+bounds.height-1});
  await expect.poll(async ()=>blocks((await state(page)).document)[0].content[0].content.map((row:any)=>row.id)).toEqual([before[1],before[0]]);
  await page.getByRole('button',{name:'Undo',exact:true}).click();
  expect(blocks((await state(page)).document)[0].content[0].content.map((row:any)=>row.id)).toEqual(before);
});

test('image width presets and alt text persist through public changes',async ({page})=>{
  await open(page,'/blocks?fixture=image'); await editor(page).locator('img').click();
  await expect(page.getByRole('toolbar',{name:'Image controls'})).toBeVisible();
  await page.getByRole('button',{name:'50%',exact:true}).click();
  await expect.poll(async ()=>blocks((await state(page)).document)[0].content[0].attrs.width).toBe(50);
  const alt = page.getByRole('textbox',{name:'Image alt text'});
  await alt.fill('Updated alt'); await alt.press('ArrowLeft');
  await expect(alt).toBeFocused();
  expect(await alt.evaluate((element:HTMLInputElement)=>element.selectionStart)).toBe('Updated alt'.length-1);
  await alt.press('Tab');
  await expect.poll(async ()=>blocks((await state(page)).document)[0].content[0].attrs.alt).toBe('Updated alt');
  await alt.fill('Enter alt'); await alt.press('Enter');
  await expect.poll(async ()=>blocks((await state(page)).document)[0].content[0].attrs.alt).toBe('Enter alt');
  await expect(editor(page)).toBeFocused();
});

test('copy emits internal, semantic HTML and plain text carriers',async ({page})=>{
  await open(page); await caret(page); await page.keyboard.type('copy me');
  await selectText(editor(page).locator('[data-content-kind=paragraph]'));
  const data=await editor(page).evaluate(element=>{
    const transfer=new DataTransfer();const event=new ClipboardEvent('copy',{bubbles:true,cancelable:true,clipboardData:transfer});Object.defineProperty(event,'clipboardData',{value:transfer});element.dispatchEvent(event);
    return {internal:transfer.getData('application/x-semantic-dxeditor+json'),html:transfer.getData('text/html'),text:transfer.getData('text/plain')};
  });
  expect(data.text).toContain('copy me'); expect(data.html).toContain('copy me'); expect(JSON.parse(data.internal).version).toBe(3);
});
