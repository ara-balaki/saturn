import '@fontsource-variable/jetbrains-mono';
import '@fontsource-variable/plus-jakarta-sans';
import '@xterm/xterm/css/xterm.css';
import './design/tokens.css';
import '../../../design/components.css';
import './app.css';
import { StrictMode } from 'react';
import { createRoot } from 'react-dom/client';

import { App } from './App';
import { screenTexts } from './terminal/registry';
import { wireWorkspace } from './workspace/actions';

wireWorkspace();

// Read-only hook for the end-to-end test.
Object.assign(window, { saturnDebug: { screenTexts } });

const root = document.getElementById('root');
if (root) {
  createRoot(root).render(
    <StrictMode>
      <App />
    </StrictMode>,
  );
}
