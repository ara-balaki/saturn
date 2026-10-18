import { useWorkspace } from '../workspace/store';
import { FolderIcon } from './icons';

/** The project at the top; the file tree and changes list arrive in Phase 3. */
export const Sidebar = () => {
  const project = useWorkspace((state) => state.project);

  return (
    <aside className="sat-panel sidebar" aria-label="Project">
      <div className="sidebar__project">
        <span className="sat-icon-tile sat-icon-tile--gradient">
          <FolderIcon />
        </span>
        <div className="sidebar__name">
          <div className="sidebar__title">{project?.name}</div>
          <div className="sidebar__path" title={project?.path}>
            {project?.path}
          </div>
        </div>
      </div>
      <div className="sat-pills" role="tablist" aria-label="Sidebar view">
        <button type="button" className="sat-pill" role="tab" aria-selected="true">
          Files
        </button>
        <button type="button" className="sat-pill" role="tab" aria-selected="false" disabled>
          Changes
        </button>
      </div>
      <div className="sidebar__empty">
        <div className="sat-dots sidebar__dots" />
        <p>The file tree comes in Phase 3.</p>
      </div>
    </aside>
  );
};
