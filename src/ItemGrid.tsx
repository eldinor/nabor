import { useCallback, useEffect, useRef, useState } from 'react';
import { useVirtualizer } from '@tanstack/react-virtual';
import { Check, FileImage, Layers3 } from 'lucide-react';
import { imageUrl } from './api';
import type { Summary } from './types';

export type InboxLayout = 'grid' | 'masonry';
const GAP = 18;

function CardPreview({ source, title, missingOriginal }: { source?: string; title: string; missingOriginal: boolean }) {
  const [failedSource, setFailedSource] = useState<string | undefined>();
  return source && failedSource !== source ? <img src={imageUrl(source)} alt={title} loading="lazy" onError={() => setFailedSource(source)} /> : <div className="image-placeholder"><FileImage size={34} strokeWidth={1.1} /><span>{missingOriginal ? 'Add an original' : failedSource ? 'Preview unavailable' : 'Preparing preview'}</span></div>;
}

export default function ItemGrid({ items, previews, layout, activeId, selected, onOpen, onToggle, filtered }: {
  items: Summary[]; previews: Record<string, string>; layout: InboxLayout; activeId?: string;
  selected: Set<string>; onOpen: (id: string) => void; onToggle: (id: string) => void; filtered: boolean;
}) {
  const scroll = useRef<HTMLDivElement>(null);
  const [width, setWidth] = useState(0);
  useEffect(() => {
    const element = scroll.current;
    if (!element) return;
    const update = () => setWidth(element.clientWidth);
    update();
    const observer = new ResizeObserver(update);
    observer.observe(element);
    return () => observer.disconnect();
  }, []);
  const columns = Math.max(1, Math.floor(width / 240));
  const columnWidth = Math.max(1, (width - 4 - GAP * (columns - 1)) / columns);
  const getKey = useCallback((index: number) => items[index].id, [items]);
  const virtual = useVirtualizer({
    count: items.length,
    getScrollElement: () => scroll.current,
    getItemKey: getKey,
    estimateSize: () => layout === 'masonry' ? columnWidth + 126 : 310,
    lanes: columns,
    laneAssignmentMode: 'measured',
    gap: GAP,
    overscan: 2,
  });
  return <div className={`grid-scroll layout-${layout}`} ref={scroll}>
    {items.length ? <div className="virtual-grid" style={{ height: virtual.getTotalSize() }}>
      {virtual.getVirtualItems().map(cell => {
        const card = items[cell.index];
        return <article className={`item-card ${activeId === card.id ? 'current' : ''} ${selected.has(card.id) ? 'selected' : ''}`}
          key={cell.key} data-index={cell.index} ref={virtual.measureElement} data-drop-item={card.id}
          style={{ position: 'absolute', top: 0, left: cell.lane * (columnWidth + GAP), width: columnWidth, transform: `translateY(${cell.start}px)` }}>
          <div className="card-image" role="button" tabIndex={0} aria-label={`Open ${card.title}`} onClick={() => onOpen(card.id)} onKeyDown={e => { if (e.key === 'Enter' || e.key === ' ') { e.preventDefault(); onOpen(card.id); } }}>
            <CardPreview source={card.thumbnailAsset ? previews[card.thumbnailAsset] : undefined} title={card.title} missingOriginal={card.missingOriginal} />
            {card.missingFiles && <span className="missing-badge">Missing file</span>}
          </div>
          <input className="card-checkbox" type="checkbox" aria-label={`Select ${card.title}`} checked={selected.has(card.id)} onChange={() => onToggle(card.id)} />
          <button className="card-description" onClick={() => onOpen(card.id)}><strong>{card.title || 'Untitled item'}</strong><span>{card.caption || 'No caption yet'}</span><div className="card-footer"><span><Layers3 size={12} />{card.assetCount} {card.assetCount === 1 ? 'attachment' : 'attachments'}</span>{!card.emptyCaption && <span className="caption-check"><Check size={12} />Caption</span>}</div></button>
        </article>;
      })}
    </div> : <div className="empty-inbox"><h2>{filtered ? 'No matching items' : 'No items'}</h2>{!filtered && <p>Drop files here or use Add files.</p>}</div>}
  </div>;
}
