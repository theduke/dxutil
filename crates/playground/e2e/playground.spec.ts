import { expect, test, type Page } from '@playwright/test';

async function openStory(page: Page, id: string, title: string) {
  await page.goto(`?story=${id}`);
  await expect(page.getByRole('heading', { name: title, exact: true, level: 1 })).toBeVisible();
}

test('crate navigation, shareable routes, history, search, theme, and reset', async ({ page }) => {
  const errors: string[] = [];
  page.on('pageerror', error => errors.push(error.message));
  await page.goto('./');
  await expect(page.getByRole('navigation', { name: 'Component stories' }).getByRole('link')).toHaveCount(12);
  await page.getByRole('button', { name: 'Primary', exact: true }).click();
  await expect(page.getByRole('status')).toContainText('Total clicks: 1');
  await page.getByRole('button', { name: 'Reset story' }).click();
  await expect(page.getByRole('status')).toContainText('Total clicks: 0');
  await page.getByRole('link', { name: 'Graph layouts', exact: true }).click();
  await expect(page).toHaveURL(/\/dxutil\/\?story=graph$/);
  await page.reload();
  await expect(page.getByRole('heading', { name: 'Graph layouts', level: 1 })).toBeVisible();
  await page.goBack();
  await expect(page.getByRole('heading', { name: 'Buttons', level: 1 })).toBeVisible();
  await page.getByLabel('Find a story').fill('dxeditor');
  await expect(page.getByRole('navigation').getByRole('link')).toHaveCount(2);
  await page.getByLabel('Find a story').fill('no-such-story');
  await expect(page.getByRole('status').filter({ hasText: 'No stories match' })).toBeVisible();
  await page.getByLabel('Find a story').fill('');
  await page.getByRole('button', { name: 'Dark theme' }).click();
  await expect(page.locator('html')).toHaveAttribute('data-theme', 'dark');
  await page.getByText('Story source · Rust', { exact: true }).click();
  await expect(page.locator('.source-panel pre')).toContainText('pub fn Buttons');
  expect(errors).toEqual([]);
});

test('button controls and live input state', async ({ page }) => {
  await openStory(page, 'buttons', 'Buttons');
  await page.getByLabel('Size', { exact: true }).selectOption('lg');
  await expect(page.getByRole('button', { name: 'Primary', exact: true })).toHaveAttribute('data-size', 'lg');
  await page.getByLabel('Disabled', { exact: true }).check();
  await expect(page.getByRole('button', { name: 'Primary', exact: true })).toBeDisabled();
  await openStory(page, 'inputs', 'Inputs & feedback');
  await page.getByLabel('Project name').fill('Ship it');
  await expect(page.locator('.state-card')).toContainText('Ship it');
  await page.getByRole('switch').click();
  await expect(page.getByRole('switch')).toHaveAttribute('aria-checked', 'false');
  const slider = page.getByRole('slider');
  await slider.focus();
  await page.keyboard.press('ArrowRight');
  await expect(page.locator('.state-card')).toContainText('66%');
});

test('dialog keyboard dismissal and confirmation', async ({ page }) => {
  await openStory(page, 'dialog', 'Dialog');
  await page.getByRole('button', { name: 'Open dialog' }).click();
  await expect(page.getByRole('dialog')).toBeVisible();
  await page.keyboard.press('Escape');
  await expect(page.getByRole('dialog')).toBeHidden();
  await page.getByRole('button', { name: 'Open dialog' }).click();
  await page.getByRole('button', { name: 'Confirm share' }).click();
  await expect(page.getByRole('status')).toContainText('Confirmed');
});

test('virtualized list jumps without mounting all rows', async ({ page }) => {
  await openStory(page, 'virtual-list', 'Virtual list');
  await page.getByRole('button', { name: 'Jump to row 9,000' }).click();
  const row = page.getByRole('button').filter({ hasText: 'Project activity 9000' });
  await expect(row).toBeVisible();
  await row.click();
  await expect(page.getByRole('status')).toContainText('Selected row 9000');
  expect(await page.locator('.virtual-row').count()).toBeLessThan(100);
});

test('Markdown controlled replacement and composer submission', async ({ page }) => {
  await openStory(page, 'markdown', 'Markdown notes');
  await expect(page.locator('[data-testid="markdown-editor"] [contenteditable="true"]').first()).toBeVisible();
  await expect(page.locator('.editor-surface strong')).toHaveText('rich text');
  await expect(page.locator('.editor-surface').getByRole('link', { name: 'Dioxus', exact: true })).toHaveAttribute('href', 'https://dioxuslabs.com');
  await page.getByRole('button', { name: 'Load checklist' }).click();
  await expect(page.locator('.editor-surface')).toContainText('Release checklist');
  await openStory(page, 'composer', 'Message composer');
  const composer = page.getByRole('textbox', { name: 'Compose a message' });
  await composer.fill('A message from the playground');
  await composer.press('Control+Enter');
  await expect(page.locator('.message')).toContainText('A message from the playground');
});

test('graph layout controls, node insertion, and keyboard selection', async ({ page }) => {
  await openStory(page, 'graph', 'Graph layouts');
  await expect(page.locator('.project-node')).toHaveCount(9);
  await page.getByLabel('Layout', { exact: true }).selectOption('radial');
  await page.getByRole('button', { name: 'Add node' }).click();
  await expect(page.locator('.project-node')).toHaveCount(10);
  await page.locator('.controls-strip').getByRole('button', { name: 'Fit graph', exact: true }).click();
  const root = page.getByRole('button', { name: 'Your next project', exact: true });
  await root.focus();
  await root.press('Space');
  await expect(page.getByRole('status').filter({ hasText: 'Selected: project' })).toBeVisible();
});

test('form validation prevents invalid submission and reset clears saved state', async ({ page }) => {
  await openStory(page, 'form', 'Validation & submission');
  await page.getByLabel('Display name').focus();
  await page.getByLabel('Email address').focus();
  await expect(page.getByText('Enter a name with at least 2 characters.', { exact: true })).toBeVisible();
  await page.getByRole('button', { name: 'Save profile' }).click();
  await expect(page.getByText('Enter a name with at least 2 characters.', { exact: true })).toBeVisible();
  await page.getByLabel('Display name').fill('Ada Lovelace');
  await page.getByLabel('Email address').fill('ada@example.org');
  await page.getByRole('button', { name: 'Save profile' }).click();
  await expect(page.getByRole('status')).toContainText('Saved locally');
  await expect(page.getByRole('status')).toContainText('Ada Lovelace');
  await page.getByRole('button', { name: 'Reset form' }).click();
  await expect(page.getByLabel('Display name')).toHaveValue('');
  await expect(page.getByRole('status')).toContainText('No profile saved yet');
});

test('terminal writes and read-only code highlighting', async ({ page }) => {
  const errors: string[] = [];
  page.on('pageerror', error => errors.push(error.message));
  await openStory(page, 'terminal', 'ANSI terminal');
  await expect(page.locator('.dxterm')).toBeVisible();
  await page.getByRole('button', { name: 'Send sample log' }).click();
  await expect(page.locator('.state-line[role="status"]')).toContainText('Sample log sent');
  await page.locator('.dxterm-input').focus();
  await page.keyboard.type('hello');
  await expect(page.locator('.state-line[role="status"]')).toContainText('Echoed');
  await page.getByLabel('Font size').selectOption('18');
  await page.getByRole('button', { name: 'Narrow preview' }).click();
  await expect(page.locator('.preview')).toHaveClass(/narrow/);
  await openStory(page, 'code', 'Code viewer');
  await expect(page.locator('.dxcode')).toBeVisible();
  await expect(page.locator('.dxcode-line').first()).toBeVisible();
  await expect(page.locator('.dxcode .tok-keyword').first()).toBeVisible();
  await page.getByLabel('Language').selectOption('app.ts');
  await expect(page.locator('.dxcode')).toContainText('interface Project');
  await expect(page.locator('.dxcode .tok-type').first()).toBeVisible();
  await page.getByLabel('Language').selectOption('package.json');
  await expect(page.locator('.dxcode')).toContainText('playground');
  await expect(page.locator('.dxcode .tok-string').first()).toBeVisible();
  await page.getByRole('button', { name: 'Go to line 3' }).click();
  await expect(page.locator('.dxcode-line[data-line="3"]')).toHaveAttribute('data-target', 'true');
  await page.getByRole('link', { name: 'Buttons', exact: true }).click();
  expect(errors).toEqual([]);
});

test('responsive preview and keyboard skip link', async ({ page }, testInfo) => {
  await page.goto('./');
  await expect(page.getByRole('heading', { name: 'Buttons', level: 1 })).toBeVisible();
  await page.keyboard.press('Tab');
  await expect(page.getByRole('link', { name: 'Skip to story' })).toBeFocused();
  await page.keyboard.press('Enter');
  await expect(page.locator('main')).toBeFocused();
  await page.screenshot({ path: testInfo.outputPath('desktop.png'), fullPage: true });
  await page.setViewportSize({ width: 390, height: 844 });
  await expect(page.getByRole('button', { name: 'Primary', exact: true })).toBeVisible();
  expect(await page.evaluate(() => document.documentElement.scrollWidth)).toBeLessThanOrEqual(390);
  await page.screenshot({ path: testInfo.outputPath('mobile.png'), fullPage: true });
});

test('form fields bind labels, drafts, and errors', async ({ page }) => {
  await openStory(page, 'form-fields', 'Form fields');
  const name = page.getByRole('textbox', { name: 'Name', exact: true });
  const age = page.getByLabel('Age', { exact: true });
  await name.focus();
  await age.focus();
  await expect(page.getByText('Enter your name.', { exact: true })).toBeVisible();
  await expect(name).toHaveAttribute('aria-invalid', 'true');
  await expect(name).toHaveAttribute('aria-describedby', /signup-name-errors/);
  await age.fill('1x');
  await name.focus();
  await expect(age).toHaveValue('1x');
  await expect(age).toHaveAttribute('aria-invalid', 'true');
  await expect(page.locator('#signup-age-errors')).toContainText('invalid number:');
  const password = page.getByLabel('Password', { exact: true });
  await password.fill('a password');
  await page.getByRole('button', { name: 'Show password', exact: true }).click();
  await expect(password).toHaveAttribute('type', 'text');
  await expect(password).toHaveValue('a password');
  await page.getByText('Send me updates', { exact: true }).click();
  await expect(page.getByRole('checkbox', { name: 'Send me updates' })).toHaveAttribute('aria-checked', 'true');
  await page.getByLabel('Plan', { exact: true }).selectOption('1');
  await expect(page.locator('.state-card')).toContainText('Team');
  await expect(page.locator('.dx-select-trigger')).toContainText('Team');
  await expect(page.locator('.dx-select-trigger')).toHaveAttribute('aria-labelledby', /dx-field-.*-label dx-field-/);
  await name.fill('Ada Lovelace');
  await age.fill('37');
  await page.getByRole('button', { name: 'Save account', exact: true }).click();
  await expect(page.getByRole('status')).toContainText('Saved locally: Ada Lovelace (Team)');
  await age.fill('bad');
  await page.getByRole('button', { name: 'Reset fields', exact: true }).click();
  await expect(name).toHaveValue('');
  await expect(age).toHaveValue('');
  await expect(password).toHaveValue('');
  await expect(page.locator('.dx-field-errors li')).toHaveCount(0);
  await expect(name).not.toHaveAttribute('aria-invalid', 'true');
});
