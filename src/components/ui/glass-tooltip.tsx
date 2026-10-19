import {
  useCallback,
  useEffect,
  useId,
  useLayoutEffect,
  useRef,
  useState,
  type ReactNode,
} from 'react';
import { createPortal } from 'react-dom';

import { cn } from '@/lib/utils';

/**
 * Hover/focus/click tooltip for the app.
 *
 * Why not `title=`? The app runs in a WKWebView (Tauri on macOS), and WebKit
 * does not surface native `title` tooltips there, so every hint written as a
 * `title` attribute was invisible. This renders its own bubble, in a portal so
 * the scroll container in Settings cannot clip it, and flips above/below when
 * there is no room.
 *
 * - hover: show
 * - click / tap: pin it open (again, or a click outside, closes it)
 * - Escape / scroll: close
 */

const VIEWPORT_MARGIN = 8;
const GAP = 8;

export interface GlassTooltipProps {
  /** Tooltip body. Text only in practice — keep it one or two sentences. */
  content: ReactNode;
  /** The trigger. Rendered inline; keep it an inline element or a span. */
  children: ReactNode;
  /** Preferred placement; flips automatically when it does not fit. */
  side?: 'top' | 'bottom';
  /** Max width of the bubble in px. */
  maxWidth?: number;
  className?: string;
}

export function GlassTooltip({
  content,
  children,
  side = 'top',
  maxWidth = 260,
  className,
}: GlassTooltipProps) {
  const id = useId();
  const triggerRef = useRef<HTMLSpanElement>(null);
  const bubbleRef = useRef<HTMLDivElement>(null);

  const [hovered, setHovered] = useState(false);
  const [focused, setFocused] = useState(false);
  const [pinned, setPinned] = useState(false);
  const [position, setPosition] = useState({ top: 0, left: 0, placement: side });

  const open = hovered || focused || pinned;

  const reposition = useCallback(() => {
    const trigger = triggerRef.current;
    if (!trigger) return;

    const rect = trigger.getBoundingClientRect();
    const bubble = bubbleRef.current?.getBoundingClientRect();
    const width = bubble?.width ?? maxWidth;
    const height = bubble?.height ?? 0;

    let placement = side;
    if (placement === 'top' && rect.top - height - GAP < VIEWPORT_MARGIN) {
      placement = 'bottom';
    } else if (
      placement === 'bottom' &&
      rect.bottom + height + GAP > window.innerHeight - VIEWPORT_MARGIN
    ) {
      placement = 'top';
    }

    const top = placement === 'top' ? rect.top - height - GAP : rect.bottom + GAP;
    const centered = rect.left + rect.width / 2 - width / 2;
    const left = Math.min(
      Math.max(centered, VIEWPORT_MARGIN),
      Math.max(window.innerWidth - width - VIEWPORT_MARGIN, VIEWPORT_MARGIN),
    );

    setPosition({ top, left, placement });
  }, [side, maxWidth]);

  // Measure after the bubble mounts so the first paint is already in place.
  useLayoutEffect(() => {
    if (open) reposition();
  }, [open, reposition]);

  useEffect(() => {
    if (!open) return;

    const close = () => setPinned(false);
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === 'Escape') {
        setPinned(false);
        setHovered(false);
        setFocused(false);
      }
    };
    // Dismiss on scroll/resize rather than tracking: the bubble is decorative,
    // and re-anchoring is cheaper than a rAF loop for a hint.
    const onScroll = () => close();
    const onPointerDown = (event: PointerEvent) => {
      if (!triggerRef.current?.contains(event.target as Node)) close();
    };

    window.addEventListener('keydown', onKeyDown);
    window.addEventListener('scroll', onScroll, true);
    window.addEventListener('resize', onScroll);
    if (pinned) document.addEventListener('pointerdown', onPointerDown);
    return () => {
      window.removeEventListener('keydown', onKeyDown);
      window.removeEventListener('scroll', onScroll, true);
      window.removeEventListener('resize', onScroll);
      document.removeEventListener('pointerdown', onPointerDown);
    };
  }, [open, pinned]);

  return (
    <>
      <span
        ref={triggerRef}
        className={cn(
          'inline-flex cursor-help rounded-sm',
          'focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-primary/40',
          className,
        )}
        tabIndex={0}
        aria-describedby={open ? id : undefined}
        onPointerEnter={() => setHovered(true)}
        onPointerLeave={() => setHovered(false)}
        onFocus={() => setFocused(true)}
        onBlur={() => setFocused(false)}
        onClick={(event) => {
          event.stopPropagation();
          setPinned((value) => !value);
        }}
      >
        {children}
      </span>

      {open
        ? createPortal(
            <div
              ref={bubbleRef}
              id={id}
              role="tooltip"
              style={{ top: position.top, left: position.left, maxWidth }}
              className={cn(
                'fixed z-[100] rounded-xl px-2.5 py-1.5 text-xxs leading-relaxed',
                'backdrop-blur-xl border shadow-glass-md animate-fade-in',
                'bg-white/95 border-divider text-text-primary',
                'dark:bg-zinc-800/95 dark:border-zinc-700 dark:text-gray-200',
                'pointer-events-none',
              )}
            >
              {content}
            </div>,
            document.body,
          )
        : null}
    </>
  );
}
