// Generates the renderer's CSS variables and the xterm.js theme from design/tokens.json, the one
// source of every design value (the same file as the Saturn Design System artifact).
// `--check` fails instead of writing when the generated files are out of date.
import { readFileSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';

const root = join(import.meta.dirname, '..');
const tokens = JSON.parse(readFileSync(join(root, 'design', 'tokens.json'), 'utf8'));
const header =
  '/* Generated from design/tokens.json by scripts/build-tokens.mjs. Do not edit. */\n';

const colors = new Map(tokens.color.tokens.map((token) => [token.name, token.value]));
const alias = (value) => /^\{(.+)\}$/.exec(value)?.[1];
const resolve = (value) => {
  const target = alias(value);
  return target ? resolve(colors.get(target)) : value;
};
const cssValue = (value) => {
  const target = alias(value);
  return target ? `var(--${target})` : value;
};

const declarations = [
  ...tokens.color.tokens.map((token) => `  --${token.name}: ${cssValue(token.value)};`),
  ...['spacing', 'radius', 'shadow'].flatMap((family) =>
    tokens[family].tokens.map((token) => `  --${token.name}: ${token.value};`),
  ),
  ...Object.entries(tokens.type.families).map(([key, stack]) => `  --font-${key}: ${stack};`),
];
const css = `${header}:root {\n${declarations.join('\n')}\n}\n`;

const color = (name) => {
  const value = colors.get(name);
  if (!value) throw new Error(`design/tokens.json has no colour token "${name}"`);
  return resolve(value);
};
const theme = {
  background: color('surface'),
  foreground: color('term-white'),
  cursor: color('ink'),
  cursorAccent: color('surface'),
  selectionBackground: `${color('purple')}66`,
  black: color('term-black'),
  red: color('term-red'),
  green: color('term-green'),
  yellow: color('term-yellow'),
  blue: color('term-blue'),
  magenta: color('term-magenta'),
  cyan: color('term-cyan'),
  white: color('term-white'),
  brightBlack: color('ink-faint'),
  brightRed: color('term-red'),
  brightGreen: color('term-green'),
  brightYellow: color('term-yellow'),
  brightBlue: color('term-blue'),
  brightMagenta: color('term-magenta'),
  brightCyan: color('term-cyan'),
  brightWhite: color('ink'),
};
const ts = `${header}import type { ITheme } from '@xterm/xterm';

export const terminalTheme: ITheme = ${JSON.stringify(theme, null, 2)
  .replace(/"([a-zA-Z]+)":/g, '$1:')
  .replaceAll('"', "'")};

export const terminalFonts = {
  family: ${JSON.stringify(tokens.type.families.mono)},
} as const;
`;

const outputs = [
  [join(root, 'src/renderer/src/design/tokens.css'), css],
  [join(root, 'src/renderer/src/design/terminal-theme.ts'), ts],
];

if (process.argv.includes('--check')) {
  const stale = outputs.filter(([path, text]) => {
    try {
      return readFileSync(path, 'utf8') !== text;
    } catch {
      return true;
    }
  });
  if (stale.length > 0) {
    console.error(`Out of date, run pnpm tokens: ${stale.map(([path]) => path).join(', ')}`);
    process.exit(1);
  }
} else {
  for (const [path, text] of outputs) writeFileSync(path, text);
}
