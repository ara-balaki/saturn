import { FileIcon } from './icons';

/** The editor area; files open here from Phase 3. */
export const EditorPanel = () => (
  <section className="sat-panel editor" aria-label="Editor">
    <div className="editor__empty">
      <span className="sat-icon-tile">
        <FileIcon />
      </span>
      <div className="editor__title">No file open</div>
      <div className="editor__hint">
        Opening and editing files comes in Phase 3. ⌘J hides this panel.
      </div>
    </div>
  </section>
);
