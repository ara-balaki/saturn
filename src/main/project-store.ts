import { readFileSync, statSync, writeFileSync } from 'node:fs';
import { basename, join } from 'node:path';

import { app } from 'electron';

import type { Project } from '@shared/ipc';

type Settings = { lastProject?: string };

const settingsFile = () => join(app.getPath('userData'), 'settings.json');

const readSettings = (): Settings => {
  try {
    return JSON.parse(readFileSync(settingsFile(), 'utf8')) as Settings;
  } catch {
    return {};
  }
};

const isDirectory = (path: string) => {
  try {
    return statSync(path).isDirectory();
  } catch {
    return false;
  }
};

export const toProject = (path: string): Project => ({ path, name: basename(path) });

/** The project Saturn had open last time, if its folder still exists. */
export const lastProject = (): Project | undefined => {
  const path = readSettings().lastProject;
  return path && isDirectory(path) ? toProject(path) : undefined;
};

export const rememberProject = (project: Project) => {
  writeFileSync(settingsFile(), JSON.stringify({ ...readSettings(), lastProject: project.path }));
};
