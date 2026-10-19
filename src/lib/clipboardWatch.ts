import { useEffect, useRef, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { getCurrentWebviewWindow } from '@tauri-apps/api/webviewWindow';
import { useStore } from '@/store/useStore';

/**
 * One Bilibili video found on the clipboard (mirrors `site::clipboard::ClipboardHit`).
 *
 * Any judgement about *whether* to offer it was made in Rust: this hook only
 * polls, remembers what was already dismissed, and surfaces the hit.
 */
export interface ClipboardHit {
  url: string;
  /**
   * The video id, or `null` for a `b23.tv` short link.
   *
   * A short link's path is an opaque code (`xyz789`), not a video id, so the pill
   * falls back to showing the link itself rather than a plausible-looking lie.
   */
  id: string | null;
  text: string;
}

/**
 * How often to look while the window is focused, and while it is not.
 *
 * The unfocused rate is the important one: the user copies a link **from their
 * browser**, so MonoSub is in the background exactly when the detection has to
 * happen. A focus-only poll would look tidy and would never fire in the real
 * case. The unfocused interval is slower because nothing is waiting on it — the
 * pill sits there until the user comes back.
 */
const POLL_INTERVAL_FOCUSED_MS = 1500;
const POLL_INTERVAL_BACKGROUND_MS = 2500;

/**
 * Watch the clipboard for a single Bilibili link.
 *
 * Design notes that matter for not annoying the user:
 *
 * * **Polling in the background too, but slower.** The user copies the link from
 *   their *browser*, so the app is unfocused at the moment of the copy. A
 *   focus-only poll (the first implementation) therefore never fired in the case
 *   the feature exists for. The pill appears while unfocused and is simply
 *   waiting when the user switches back.
 * * **A hit is cleared when the clipboard moves on**, so the pill never offers a
 *   link the user copied and then replaced.
 * * **Dismissal is remembered per text.** Dismissing one link must not suppress
 *   the next one, and re-showing the same dismissal would be a nag.
 * * **A hit is dropped once it is imported**, so the pill cannot reappear for a
 *   job the user already started.
 * * **No hit while settings are open.** The confirm pill would sit on top of the
 *   panel; the user is clearly doing something else.
 */
export function useClipboardVideo(isBlocked: boolean) {
  const enabled = useStore((state) => state.settings.clipboardWatchEnabled);
  const [hit, setHit] = useState<ClipboardHit | null>(null);
  const dismissedRef = useRef<Set<string>>(new Set());
  const lastSeenRef = useRef<string | null>(null);

  useEffect(() => {
    if (isBlocked || !enabled) {
      setHit(null);
      return;
    }

    let disposed = false;
    let timer: number | undefined;

    const poll = async () => {
      if (disposed) return;

      try {
        const found = await invoke<ClipboardHit | null>('read_clipboard_video');
        if (disposed) return;

        if (!found) {
          // The clipboard changed away from a Bilibili link.
          lastSeenRef.current = null;
          setHit(null);
          return;
        }

        if (dismissedRef.current.has(found.text)) {
          setHit(null);
          return;
        }

        // A queued/imported URL must not be offered again.
        if (isAlreadyImported(found.url)) {
          setHit(null);
          return;
        }

        lastSeenRef.current = found.text;
        setHit(found);
      } catch (error) {
        // A clipboard read can fail (locked screen, another app holding the
        // pasteboard). That is not worth a console error on every poll.
        console.debug('[clipboard] read failed:', error);
      }
    };

    // Slow while unfocused, fast while focused. Re-armed on every focus change
    // so a user who switches to the browser and back is not waiting out the slow
    // interval.
    const schedule = () => {
      const interval = document.hasFocus()
        ? POLL_INTERVAL_FOCUSED_MS
        : POLL_INTERVAL_BACKGROUND_MS;
      if (timer !== undefined) window.clearInterval(timer);
      timer = window.setInterval(() => {
        void poll();
        schedule();
      }, interval);
    };

    const onFocus = () => {
      void poll();
      schedule();
    };
    window.addEventListener('focus', onFocus);
    document.addEventListener('visibilitychange', onFocus);

    void poll();
    schedule();

    return () => {
      disposed = true;
      if (timer !== undefined) window.clearInterval(timer);
      window.removeEventListener('focus', onFocus);
      document.removeEventListener('visibilitychange', onFocus);
    };
  }, [isBlocked, enabled]);

  const dismiss = () => {
    if (hit) dismissedRef.current.add(hit.text);
    setHit(null);
  };

  /** Accept the hit: mark it so it cannot come back, then hand it to the caller. */
  const accept = (): ClipboardHit | null => {
    if (!hit) return null;
    dismissedRef.current.add(hit.text);
    setHit(null);
    return hit;
  };

  return { hit, dismiss, accept };
}

/**
 * Whether this URL is already in the queue or in flight.
 *
 * Compared on the normalised URL, which is what both sides store.
 */
const isAlreadyImported = (url: string): boolean => {
  const state = useStore.getState();
  const inQueue = state.files.some((file) => file.sourceUrl === url);
  const inFlight = Object.values(state.urlImports).some((item) => item.url === url);
  return inQueue || inFlight;
};

/**
 * Ask the backend to re-read the clipboard right now.
 *
 * Used after the window regains focus, so the pill appears without waiting out
 * the poll interval. Returns the hit, or `null`.
 */
export const readClipboardNow = async (): Promise<ClipboardHit | null> => {
  try {
    return await invoke<ClipboardHit | null>('read_clipboard_video');
  } catch {
    return null;
  }
};

/**
 * Whether the window is currently focused.
 *
 * Exported so the pill can hide itself immediately on blur rather than after the
 * next poll.
 */
export const attachFocusRefresh = (onFocus: () => void): (() => void) => {
  let unlisten: (() => void) | null = null;
  let disposed = false;

  void getCurrentWebviewWindow()
    .onFocusChanged(({ payload: focused }) => {
      if (focused) onFocus();
    })
    .then((remove) => {
      if (disposed) remove();
      else unlisten = remove;
    });

  return () => {
    disposed = true;
    unlisten?.();
  };
};
