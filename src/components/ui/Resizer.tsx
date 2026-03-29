import React from 'react';
import clsx from 'clsx';
import { Orientation } from './AppProperties';

interface ResizerProps {
  direction: Orientation;
  onMouseDown: React.MouseEventHandler<HTMLDivElement>;
}

const Resizer = ({ direction, onMouseDown }: ResizerProps) => (
  <div
    className={clsx('shrink-0 bg-transparent z-10', {
      'w-2 cursor-col-resize': direction === Orientation.Vertical,
      'h-2 cursor-row-resize': direction === Orientation.Horizontal,
    })}
    onMouseDown={onMouseDown}
  />
);

export default Resizer;
