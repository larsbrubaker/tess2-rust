// Lightweight persistence for demo control values across page refreshes.
// Uses sessionStorage keyed by route name + element id.

const PREFIX = 'tess2:';

function storeKey(route: string, id: string): string {
  return `${PREFIX}${route}:${id}`;
}

/**
 * Restore saved values for all select/checkbox/input controls inside `container`,
 * then auto-save whenever they change. Call this after setting up the DOM but
 * before the first render() so restored values are visible immediately.
 *
 * Returns a cleanup function that removes the change listeners.
 */
export function persistControls(route: string, container: HTMLElement): () => void {
  const controls = container.querySelectorAll<HTMLSelectElement | HTMLInputElement>(
    'select[id], input[id]',
  );
  const listeners: [EventTarget, string, EventListener][] = [];

  for (const el of controls) {
    const key = storeKey(route, el.id);

    // Restore
    const saved = sessionStorage.getItem(key);
    if (saved !== null) {
      if (el instanceof HTMLInputElement && el.type === 'checkbox') {
        el.checked = saved === 'true';
      } else {
        el.value = saved;
      }
    }

    // Auto-save on change
    const handler = () => {
      if (el instanceof HTMLInputElement && el.type === 'checkbox') {
        sessionStorage.setItem(key, String(el.checked));
      } else {
        sessionStorage.setItem(key, el.value);
      }
    };
    el.addEventListener('change', handler);
    listeners.push([el, 'change', handler]);
  }

  return () => {
    for (const [target, event, handler] of listeners) {
      target.removeEventListener(event, handler);
    }
  };
}
