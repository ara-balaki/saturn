// End-to-end smoke test: launches the built app on this repo, drives the grid through its
// commands, and saves screenshots to e2e/output/. Run with `pnpm e2e` (builds first).
import { mkdirSync, mkdtempSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';

import { _electron as electron, type Page } from 'playwright';

const root = resolve(import.meta.dirname, '..');
const output = join(root, 'e2e', 'output');
mkdirSync(output, { recursive: true });

const userData = mkdtempSync(join(tmpdir(), 'saturn-e2e-'));
writeFileSync(join(userData, 'settings.json'), JSON.stringify({ lastProject: root }));

const app = await electron.launch({
  args: [join(root, 'out', 'main', 'index.js')],
  env: { ...process.env, SATURN_USER_DATA: userData },
});
const page = await app.firstWindow();
await app.evaluate(({ BrowserWindow }) => BrowserWindow.getAllWindows()[0]?.setSize(1600, 1000));

const failures: string[] = [];
const check = (condition: boolean, message: string) => {
  console.log(`${condition ? '✓' : '✗'} ${message}`);
  if (!condition) failures.push(message);
};
const command = async (name: string) => {
  await app.evaluate(
    ({ BrowserWindow }, value) =>
      BrowserWindow.getAllWindows()[0]?.webContents.send('command', value),
    name,
  );
  await page.waitForTimeout(400);
};
const count = (page: Page, selector: string) => page.locator(selector).count();
const screenList = () =>
  page.evaluate(() =>
    Object.values(
      (
        window as unknown as { saturnDebug: { screenTexts: () => Record<string, string> } }
      ).saturnDebug.screenTexts(),
    ),
  );
const screens = async () => (await screenList()).join('\n');
/** Input sent while a shell is still starting can be dropped, so wait until every one shows a prompt. */
const waitForPrompts = async () => {
  for (let tries = 0; tries < 75; tries++) {
    if ((await screenList()).every((text) => text.trim() !== '')) break;
    await page.waitForTimeout(200);
  }
  await page.waitForTimeout(400);
};
const shot = (name: string) => page.screenshot({ path: join(output, `${name}.png`) });
const typeLine = async (text: string) => {
  await page.keyboard.type(text);
  await page.keyboard.press('Enter');
  // Let the shell run it and its output reach the app before the test moves on.
  await page.waitForTimeout(700);
};

await page.locator('.cell').first().waitFor();
// The test window never gets the OS focus; tell the app it has it, as it would for a person.
await page.evaluate(() => window.dispatchEvent(new Event('focus')));
await page.waitForTimeout(500);
await waitForPrompts();
check((await count(page, '.cell')) === 1, 'opens one terminal in the project');

await typeLine('echo saturn-$((40+2))');
await page.waitForTimeout(500);
check((await screens()).includes('saturn-42'), 'runs a command in the terminal');

for (const step of ['split-right', 'split-right', 'split-down', 'split-right', 'new-tab']) {
  await command(step);
  await page.waitForTimeout(700);
}
await waitForPrompts();
check((await count(page, '.grid-row')) === 2, 'split down adds a second row');
check((await count(page, '.cell')) === 5, 'splits make five cells');
check((await count(page, '.sat-tab')) === 6, 'the new tab joins the focused cell');

// Pretend to be Claude in three panes: one asks for permission, one finished, one works.
await command('focus-cell-2');
await page.waitForTimeout(300);
await typeLine(
  `printf '\\033]0;ABC-236\\007\\033]7501;state=blocked:app=claude-code:kind=permission:msg=${Buffer.from('approve Bash: pnpm test').toString('base64')}\\007'; sleep 30`,
);
await command('focus-cell-3');
await page.waitForTimeout(300);
await typeLine(
  `printf '\\033]0;✳ review-221\\007\\033]7501;state=done:app=claude-code\\007'; sleep 30`,
);
await command('focus-cell-1');
await page.waitForTimeout(300);
await typeLine(
  `printf '\\033]0;ABC-240\\007\\033]7501;state=working:app=claude-code:msg=${Buffer.from('Running pnpm typecheck').toString('base64')}\\007'; sleep 30`,
);
await page.waitForTimeout(800);

check(
  await page.getByText('ABC-236', { exact: true }).isVisible(),
  "a tab takes Claude's session name from the title",
);
check(
  await page.getByText('review-221', { exact: true }).isVisible(),
  'the spinner glyph is dropped from the title',
);
check(
  (await page.locator('.cell[data-attention="needs-you"]').count()) === 0,
  'a pane you are looking at does not glow',
);
await command('focus-cell-4');
// This one asks a moment later, once the user has moved on to another pane.
await typeLine(
  `sleep 1.5; printf '\\033]7501;state=blocked:app=claude-code:kind=permission\\007'; sleep 30`,
);
await command('focus-cell-5');
await page.waitForTimeout(2000);
check(
  (await page.locator('.cell[data-attention="needs-you"]').count()) === 1,
  'a pane that needs you glows when you look away',
);
check(await page.getByText('2 need you').isVisible(), 'the title bar counts agents that need you');
await shot('grid');

await command('next-attention');
await page.waitForTimeout(400);
check(
  (await page.locator('.cell.is-focused[data-attention="needs-you"]').count()) === 0,
  'jumping to it clears the glow',
);

const tabs = page.locator('.sat-tab');
const source = tabs.nth(4);
const sourceName = await source.innerText();
await source.dragTo(page.locator('.cell-body').first());
await page.waitForTimeout(600);
check(
  (await page.locator('.cell').first().locator('.sat-tab').count()) === 2,
  `dragging a tab (${sourceName.trim()}) moves it into another cell`,
);

await command('toggle-maximize');
await page.waitForTimeout(400);
check((await page.locator('.cell:visible').count()) === 1, 'maximise shows one pane');
await shot('maximized');
await command('toggle-maximize');

await command('toggle-sidebar');
await command('toggle-editor');
await page.waitForTimeout(500);
check(
  !(await page.locator('.sidebar').isVisible()) && !(await page.locator('.editor').isVisible()),
  '⌘B and ⌘J hide the sidebar and editor',
);
check(
  (await screens()).includes('saturn-42'),
  'terminals keep their output through layout changes',
);
await shot('grid-only');

await page.locator('.cell').first().locator('.sat-tab').first().dblclick();
await page.keyboard.type('renamed');
await page.keyboard.press('Enter');
check(
  await page.getByText('renamed', { exact: true }).isVisible(),
  'double-clicking a tab renames it',
);

await app.close();
console.log(failures.length === 0 ? '\nAll checks passed.' : `\n${failures.length} failed.`);
process.exit(failures.length === 0 ? 0 : 1);
