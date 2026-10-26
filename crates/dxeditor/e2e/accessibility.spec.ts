import { test, expect } from '@playwright/test';
import { open, editor, caret, selectText } from './helpers.js';

test('formatting, image and table toolbars support roving keyboard focus', async ({page}) => {
  for (const kind of ['text', 'image', 'table']) {
    await open(page, kind === 'text' ? '/blocks' : `/blocks?fixture=${kind}`);
    if (kind === 'text') {
      await caret(page);
      await page.keyboard.type('Format me');
      await selectText(editor(page).locator('[data-content-kind=paragraph]'));
    } else if (kind === 'image') {
      await editor(page).locator('img').click();
    } else {
      await caret(page, editor(page).locator('td,th').first());
    }
    const toolbar = page.getByRole('toolbar', {name: kind === 'text' ? 'Text formatting' : kind === 'image' ? 'Image controls' : 'Table editing', exact: true});
    await expect(toolbar).toBeVisible();
    await expect(toolbar.locator('[tabindex="0"]:visible')).toHaveCount(1);
    const buttons = toolbar.getByRole('button');
    await buttons.first().focus();
    await expect(buttons.first()).toBeFocused();
    await page.keyboard.press('ArrowRight');
    await expect(buttons.nth(1)).toBeFocused();
    await expect(toolbar.locator('[tabindex="0"]:visible')).toHaveCount(1);
    await page.keyboard.press('ArrowLeft');
    await expect(buttons.first()).toBeFocused();
    await expect(toolbar.locator('[tabindex="0"]:visible')).toHaveCount(1);
  }
});
