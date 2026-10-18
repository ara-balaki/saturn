import { useEffect, useRef } from 'react';

import { EditorPanel } from './chrome/EditorPanel';
import { FolderIcon } from './chrome/icons';
import { Resizer } from './chrome/Resizer';
import { Sidebar } from './chrome/Sidebar';
import { TitleBar } from './chrome/TitleBar';
import { Grid } from './grid/Grid';
import { openTerminal, setEditorHeight, setSidebarWidth } from './workspace/actions';
import { getState, setState, useWorkspace } from './workspace/store';

let loading = false;

const loadProject = async () => {
  // React runs effects twice in development; the first terminal must open only once.
  if (loading) return;
  loading = true;
  const project = await window.saturn.project.get();
  setState((current) => ({ ...current, project, projectLoaded: true }));
  if (project && getState().layout.rows.length === 0) await openTerminal();
};

const Welcome = () => (
  <div className="welcome">
    <div className="sat-dots welcome__dots" />
    <div className="welcome__content">
      <h1 className="welcome__title">Saturn</h1>
      <p className="welcome__hint">Open a project to start your terminals there.</p>
      <button
        type="button"
        className="sat-button sat-button--primary"
        onClick={() => void window.saturn.project.open()}
      >
        <FolderIcon />
        Open project <kbd>⌘O</kbd>
      </button>
    </div>
  </div>
);

const Workbench = () => {
  const sidebarOpen = useWorkspace((state) => state.sidebarOpen);
  const editorOpen = useWorkspace((state) => state.editorOpen);
  const sidebarWidth = useWorkspace((state) => state.sidebarWidth);
  const editorHeight = useWorkspace((state) => state.editorHeight);
  const start = useRef(0);

  // The grid stays at the same place in the tree whatever is toggled, so terminals never remount.
  return (
    <div className="workbench">
      <div className="workbench__side" hidden={!sidebarOpen} style={{ width: sidebarWidth }}>
        <Sidebar />
      </div>
      {sidebarOpen && (
        <Resizer
          axis="col"
          label="Resize sidebar"
          onStart={() => {
            start.current = getState().sidebarWidth;
          }}
          onDrag={(delta) => setSidebarWidth(start.current + delta)}
        />
      )}
      <div className="workbench__main">
        <div className="workbench__editor" hidden={!editorOpen} style={{ height: editorHeight }}>
          <EditorPanel />
        </div>
        {editorOpen && (
          <Resizer
            axis="row"
            label="Resize editor"
            onStart={() => {
              start.current = getState().editorHeight;
            }}
            onDrag={(delta) => setEditorHeight(start.current + delta)}
          />
        )}
        <div className="workbench__grid">
          <Grid />
        </div>
      </div>
    </div>
  );
};

export const App = () => {
  const project = useWorkspace((state) => state.project);
  const loaded = useWorkspace((state) => state.projectLoaded);

  useEffect(() => {
    void loadProject();
    return window.saturn.project.onChanged((next) => {
      setState((current) => ({ ...current, project: next }));
      if (getState().layout.rows.length === 0) void openTerminal();
    });
  }, []);

  return (
    <div className="app sat-root">
      <TitleBar />
      {loaded && (project ? <Workbench /> : <Welcome />)}
    </div>
  );
};
