import type { SaturnApi } from '@shared/ipc';

declare global {
  interface Window {
    saturn: SaturnApi;
  }
}
