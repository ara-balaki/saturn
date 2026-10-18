import { FitAddon } from '@xterm/addon-fit';
import { Unicode11Addon } from '@xterm/addon-unicode11';
import { WebLinksAddon } from '@xterm/addon-web-links';
import { WebglAddon } from '@xterm/addon-webgl';
import { Terminal } from '@xterm/xterm';

const theme = {
  background: '#14161a',
  foreground: '#d4d7dd',
  cursor: '#d4d7dd',
  selectionBackground: '#3a4150',
};

/** An xterm.js terminal with the addons every Saturn pane uses. */
export const createTerminal = () => {
  const terminal = new Terminal({
    fontFamily: '"JetBrains Mono", Menlo, monospace',
    fontSize: 13,
    lineHeight: 1.15,
    macOptionIsMeta: true,
    allowProposedApi: true,
    scrollback: 5000,
    theme,
  });
  const fit = new FitAddon();
  terminal.loadAddon(fit);
  terminal.loadAddon(new WebLinksAddon());
  terminal.loadAddon(new Unicode11Addon());
  terminal.unicode.activeVersion = '11';
  return { terminal, fit };
};

/**
 * WebGL is much faster, but Chromium keeps only ~16 contexts per page and drops the oldest. On loss
 * the pane falls back to the DOM renderer, as VS Code does.
 */
export const attachWebgl = (terminal: Terminal, onFallback: () => void) => {
  try {
    const webgl = new WebglAddon();
    webgl.onContextLoss(() => {
      webgl.dispose();
      onFallback();
    });
    terminal.loadAddon(webgl);
    return true;
  } catch {
    onFallback();
    return false;
  }
};
