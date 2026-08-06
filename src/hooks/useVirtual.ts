import { useCallback, useEffect, useMemo, useRef, useState } from "react";

export interface VirtualList {
  containerRef: React.RefObject<HTMLDivElement>;
  totalHeight: number;
  offsetY: number;
  visibleStart: number;
  visibleEnd: number;
  onScroll: () => void;
  scrollTo: (index: number) => void;
}

const OVERSCAN = 12;

/**
 * Virtualization for fixed-height rows. The caller renders an inner div with
 * `height: totalHeight` and only renders rows in [visibleStart, visibleEnd),
 * absolutely positioned at `index * rowHeight`. `onNearEnd` fires when the
 * user scrolls within one screen of the bottom (used for incremental loading).
 */
export function useVirtual(
  count: number,
  rowHeight: number,
  onNearEnd?: () => void,
): VirtualList {
  const containerRef = useRef<HTMLDivElement>(null);
  const [scrollTop, setScrollTop] = useState(0);
  const [viewport, setViewport] = useState(600);
  const nearEndFired = useRef(false);
  const onNearEndRef = useRef(onNearEnd);
  onNearEndRef.current = onNearEnd;

  // Measure the viewport on mount and when it resizes.
  useEffect(() => {
    const el = containerRef.current;
    if (!el) return;
    const measure = () => setViewport(el.clientHeight);
    measure();
    const observer = new ResizeObserver(measure);
    observer.observe(el);
    return () => observer.disconnect();
  }, []);

  const onScroll = useCallback(() => {
    const el = containerRef.current;
    if (!el) return;
    setScrollTop(el.scrollTop);
    const nearBottom = el.scrollTop + el.clientHeight > el.scrollHeight - rowHeight * 20;
    if (nearBottom && !nearEndFired.current) {
      nearEndFired.current = true;
      onNearEndRef.current?.();
    } else if (!nearBottom) {
      nearEndFired.current = false;
    }
  }, [rowHeight]);

  const scrollTo = useCallback(
    (index: number) => {
      const el = containerRef.current;
      if (el) el.scrollTop = Math.max(0, index * rowHeight - 80);
    },
    [rowHeight],
  );

  return useMemo(() => {
    const start = Math.max(0, Math.floor(scrollTop / rowHeight) - OVERSCAN);
    const visible = Math.max(0, Math.ceil(viewport / rowHeight) + OVERSCAN * 2);
    return {
      containerRef,
      totalHeight: count * rowHeight,
      offsetY: start * rowHeight,
      visibleStart: start,
      visibleEnd: Math.min(count, start + visible),
      onScroll,
      scrollTo,
    };
  }, [scrollTop, viewport, rowHeight, count, onScroll, scrollTo]);
}
