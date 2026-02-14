import { useState, useCallback } from 'react';

export interface WindowInstance {
  id: string;
  type: string;
  title: string;
  x: number;
  y: number;
  zIndex: number;
}

export function useWindowManager(initialZIndex = 100) {
  const [windows, setWindows] = useState<WindowInstance[]>([]);
  const [nextZ, setNextZ] = useState(initialZIndex);

  const openWindow = useCallback((item: { id: string, label: string }, x: number, y: number) => {
    const id = `${item.id}-${Date.now()}`;
    setWindows(prev => [...prev, {
      id,
      type: item.id,
      title: item.label,
      x: Math.max(0, x - 400),
      y: Math.max(0, y - 20),
      zIndex: nextZ
    }]);
    setNextZ(z => z + 1);
  }, [nextZ]);

  const closeWindow = useCallback((id: string) => {
    setWindows(prev => prev.filter(w => w.id !== id));
  }, []);

  const focusWindow = useCallback((id: string) => {
    setWindows(prev => prev.map(w => w.id === id ? { ...w, zIndex: nextZ } : w));
    setNextZ(z => z + 1);
  }, [nextZ]);

  return { windows, openWindow, closeWindow, focusWindow };
}
