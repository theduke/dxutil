import { test, expect } from '@playwright/test';
import { open, editor, caret } from './helpers.js';

for (const route of ['/blocks', '/composer']) {
  test(`${route}: outside popover controls retain focus and receive clicks`, async ({ page }) => {
    await open(page, route);
    await caret(page);
    await page.evaluate(() => {
      const popup = document.createElement('div');
      popup.setAttribute('popover', 'manual');
      popup.setAttribute('role', 'dialog');
      popup.setAttribute('aria-label', 'Host settings');
      popup.innerHTML = '<input aria-label="Host search"><button type="button">Host option</button>';
      popup.querySelector('button')!.addEventListener('click', () => {
        popup.dataset.selected = 'true';
      });
      document.body.append(popup);
      document.addEventListener('focusin', event => {
        if (!popup.contains(event.target as Node)) popup.hidePopover();
      });
      popup.showPopover();
      popup.querySelector('input')!.focus();
    });

    const popup = page.getByRole('dialog', { name: 'Host settings' });
    const search = popup.getByRole('textbox', { name: 'Host search' });
    await search.click();
    await search.fill('model');
    await expect(search).toBeFocused();
    await popup.getByRole('button', { name: 'Host option' }).click();
    await expect(popup).toHaveAttribute('data-selected', 'true');
    await expect(popup).toBeVisible();
    await expect(editor(page)).not.toBeFocused();

    // Keyboard navigation between outside controls also leaves focus with them.
    await search.focus();
    await page.keyboard.press('Tab');
    await expect(popup.getByRole('button', { name: 'Host option' })).toBeFocused();
  });
}
