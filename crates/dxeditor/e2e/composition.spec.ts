import { test, expect } from '@playwright/test';
import { open, editor, caret } from './helpers.js';

for (const [name,route,selector] of [
  ['paragraph','/blocks','[data-content-kind=paragraph]'],
  ['list','/blocks?fixture=all','[data-content-kind=bulleted_list_item]'],
  ['cell','/blocks?fixture=table','td'],
  ['caption','/blocks?fixture=image','figcaption'],
  ['composer','/composer','[data-content-kind=paragraph]'],
]) {
  test(`composition commits once in ${name}`, async ({page,browserName}) => {
    test.skip(browserName !== 'chromium', 'CDP IME simulation is Chromium-specific; common editing flows cover all projects.');
    await open(page,route); const target=editor(page).locator(selector).first(); await caret(page,target);
    const session=await page.context().newCDPSession(page);
    await session.send('Input.imeSetComposition',{text:'に',selectionStart:1,selectionEnd:1});
    await session.send('Input.imeSetComposition',{text:'日本語',selectionStart:3,selectionEnd:3});
    await session.send('Input.insertText',{text:'日本語'});
    await expect(target).toContainText('日本語');
    await expect(page.locator('[role=status]')).not.toContainText('invalid');
    await page.keyboard.press('ControlOrMeta+z'); await expect(target).not.toContainText('日本語');
    await session.detach();
  });
}
