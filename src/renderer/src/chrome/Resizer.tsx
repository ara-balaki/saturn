import type { PointerEvent as ReactPointerEvent } from 'react';

type ResizerProps = {
  /** `col` drags left and right, `row` drags up and down. */
  axis: 'col' | 'row';
  /** Called on every move with the distance from where the drag started, in pixels. */
  onDrag: (delta: number) => void;
  onStart?: () => void;
  label: string;
};

/** A thin invisible handle in the gap between two panels. */
export const Resizer = ({ axis, onDrag, onStart, label }: ResizerProps) => {
  const begin = (event: ReactPointerEvent<HTMLDivElement>) => {
    event.preventDefault();
    const handle = event.currentTarget;
    const origin = axis === 'col' ? event.clientX : event.clientY;
    handle.setPointerCapture(event.pointerId);
    onStart?.();
    const move = (moveEvent: PointerEvent) =>
      onDrag((axis === 'col' ? moveEvent.clientX : moveEvent.clientY) - origin);
    const end = () => {
      handle.removeEventListener('pointermove', move);
      handle.removeEventListener('pointerup', end);
      document.body.classList.remove('is-resizing');
    };
    document.body.classList.add('is-resizing');
    handle.addEventListener('pointermove', move);
    handle.addEventListener('pointerup', end);
  };

  return (
    <div
      className={`resizer resizer--${axis}`}
      role="separator"
      aria-orientation={axis === 'col' ? 'vertical' : 'horizontal'}
      aria-label={label}
      onPointerDown={begin}
    />
  );
};
