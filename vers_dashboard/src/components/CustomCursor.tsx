import { useEffect, useRef } from 'react';

interface Point { x: number; y: number; r: number }

export function CustomCursor() {
  const trailCanvasRef = useRef<HTMLCanvasElement>(null);
  const points = useRef<Point[]>([]);
  const lastPlacedMouse = useRef({ x: -1000, y: -1000 });
  const localMouse = useRef({ x: -1000, y: -1000 });

  useEffect(() => {
    // Listen for mouse moves immediately on the window
    const handleMouseMove = (e: MouseEvent) => {
      localMouse.current = { x: e.clientX, y: e.clientY };
    };
    window.addEventListener('mousemove', handleMouseMove, { passive: true });

    let rafId: number;
    let tctx: CanvasRenderingContext2D | null = null;
    let octx: CanvasRenderingContext2D | null = null;
    let offscreenCanvas: HTMLCanvasElement | null = null;

    const initCanvas = () => {
      const canvas = trailCanvasRef.current;
      if (!canvas) return false;

      tctx = canvas.getContext('2d');
      offscreenCanvas = document.createElement('canvas');
      octx = offscreenCanvas.getContext('2d');
      
      if (!tctx || !octx) return false;
      return true;
    };

    const handleResize = () => {
      const canvas = trailCanvasRef.current;
      if (!canvas || !tctx || !octx || !offscreenCanvas) return;

      const dpr = Math.min(window.devicePixelRatio || 1, 2);
      const width = window.innerWidth;
      const height = window.innerHeight;

      canvas.width = width * dpr;
      canvas.height = height * dpr;
      canvas.style.width = `${width}px`;
      canvas.style.height = `${height}px`;

      offscreenCanvas.width = 400 * dpr;
      offscreenCanvas.height = 400 * dpr;
      octx.setTransform(dpr, 0, 0, dpr, 0, 0);
      tctx.setTransform(dpr, 0, 0, dpr, 0, 0);
    };

    window.addEventListener('resize', handleResize);

    const render = () => {
      rafId = requestAnimationFrame(render);
      
      // If canvas is not initialized yet, try to initialize it
      if (!tctx && !initCanvas()) return;
      if (!tctx || !octx || !offscreenCanvas) return;

      if (document.hidden) return;

      const mx = localMouse.current.x;
      const my = localMouse.current.y;

      if (mx === -1000) return;

      // Mouse Trail logic
      if (lastPlacedMouse.current.x === -1000) {
        lastPlacedMouse.current = { x: mx, y: my };
        handleResize(); // Initial size setup
      }
      
      const dx = mx - lastPlacedMouse.current.x;
      const dy = my - lastPlacedMouse.current.y;
      const dist = Math.sqrt(dx * dx + dy * dy);
      
      if (dist >= 2) {
        const steps = Math.max(1, Math.min(10, Math.floor(dist / 4)));
        for (let i = 1; i <= steps; i++) {
          points.current.unshift({ 
            x: lastPlacedMouse.current.x + dx * (i / steps), 
            y: lastPlacedMouse.current.y + dy * (i / steps), 
            r: 3
          });
        }
        lastPlacedMouse.current = { x: mx, y: my };
      }

      // Decay
      if (points.current.length > 40) points.current.splice(40);
      for (let i = 0; i < 2; i++) if (points.current.length > 0) points.current.pop();

      // --- RENDER ---
      tctx.clearRect(0, 0, window.innerWidth, window.innerHeight);
      octx.clearRect(0, 0, 400, 400);

      octx.save();
      octx.translate(200 - mx, 200 - my);
      octx.fillStyle = '#2e4de6';
      
      for (let i = 0; i < points.current.length; i++) {
        const p = points.current[i];
        octx.globalAlpha = Math.max(0, 1 - i / points.current.length);
        octx.beginPath();
        octx.arc(p.x, p.y, p.r * (1 - (i / points.current.length) * 0.5), 0, Math.PI * 2);
        octx.fill();
      }
      
      octx.globalAlpha = 1;
      octx.beginPath();
      octx.arc(mx, my, 4, 0, Math.PI * 2);
      octx.fill();
      octx.restore();

      tctx.save();
      tctx.filter = 'url(#goo-cursor)';
      tctx.drawImage(offscreenCanvas, mx - 200, my - 200, 400, 400);
      tctx.restore();
    };

    render();

    return () => {
      window.removeEventListener('resize', handleResize);
      window.removeEventListener('mousemove', handleMouseMove);
      cancelAnimationFrame(rafId);
    };
  }, []);

  return (
    <>
      <svg style={{ position: 'absolute', width: 0, height: 0, pointerEvents: 'none' }}>
        <defs>
          <filter id="goo-cursor">
            <feGaussianBlur in="SourceGraphic" stdDeviation="2" result="blur" />
            <feColorMatrix in="blur" mode="matrix" values="1 0 0 0 0  0 1 0 0 0  0 0 1 0 0  0 0 0 18 -7" result="goo" />
          </filter>
        </defs>
      </svg>
      <canvas 
        ref={trailCanvasRef} 
        className="fixed inset-0 pointer-events-none z-[9999] w-full h-full" 
        style={{ pointerEvents: 'none' }}
      />
    </>
  );
}