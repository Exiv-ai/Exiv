import { useEffect, useRef, useState } from 'react';
import { Suspense, lazy } from 'react';
import { useNavigate } from 'react-router-dom';
import { Activity, Database, MessageSquare, Puzzle, Settings } from 'lucide-react';
import { InteractiveGrid } from '../components/InteractiveGrid';
import { GlassWindow } from '../components/GlassWindow';
import { CustomCursor } from '../components/CustomCursor';
import { PluginManifest } from '../types';

const StatusCore = lazy(() => import('../components/StatusCore').then(m => ({ default: (m as any).StatusCore })));

const MemoryCore = lazy(() => import('../components/MemoryCore').then(m => ({ default: m.MemoryCore })));
const SandboxCore = lazy(() => import('../components/AgentTerminal').then(m => ({ default: m.AgentTerminal })));
const VersPluginManager = lazy(() => import('../components/VersPluginManager').then(m => ({ default: m.VersPluginManager })));

interface WindowInstance {
  id: string;
  type: string;
  title: string;
  x: number;
  y: number;
  zIndex: number;
}

export function Home() {
  const containerRef = useRef<HTMLDivElement>(null);
  const realMouse = useRef({ x: -1000, y: -1000 });
  const navigate = useNavigate();

  // Window Management
  const [windows, setWindows] = useState<WindowInstance[]>([]);
  const [nextZ, setNextZ] = useState(100);
  const [activeMainView, setActiveMainView] = useState<string | null>(null);
  const [isCursorActive, setIsCursorActive] = useState(true);

  useEffect(() => {
    // カーネルからプラグイン状態を取得してカーソルのオンオフを判定
    fetch('/plugins')
      .then(res => res.json())
      .then((data: PluginManifest[]) => {
        const cursorPlugin = data.find(p => p.name.includes("Neural Cursor"));
        if (cursorPlugin) {
          setIsCursorActive(cursorPlugin.is_active);
        }
      })
      .catch(err => console.error("Failed to sync cursor plugin:", err));
  }, []);

  useEffect(() => {
    if (isCursorActive) {
      document.body.classList.add('neural-cursor-active');
    } else {
      document.body.classList.remove('neural-cursor-active');
    }
    return () => {
      document.body.classList.remove('neural-cursor-active');
    };
  }, [isCursorActive]);

  // Drag Logic
  const dragRef = useRef<{
    active: boolean;
    startX: number;
    startY: number;
    item: any;
    hasDetached: boolean;
  }>({ active: false, startX: 0, startY: 0, item: null, hasDetached: false });
  
  const [ghostPos, setGhostPos] = useState<{x: number, y: number} | null>(null);

  const menuItems = [
    { id: 'status', label: 'STATUS', path: '/status', icon: Activity, disabled: false },
    { id: 'memory', label: 'MEMORY', path: '/dashboard', icon: Database, disabled: false },
    { id: 'sandbox', label: 'VERS', path: '#', icon: MessageSquare, disabled: false },
    { id: 'plugin', label: 'PLUGIN', path: '#', icon: Puzzle, disabled: false },
    { id: 'system', label: 'SYSTEM', path: '#', icon: Settings, disabled: false },
  ];

  const handleItemMouseDown = (e: MouseEvent, item: any) => {
    if (item.disabled) return;
    if (e.button !== 0) return;
    
    dragRef.current = { 
      active: true, 
      startX: e.clientX, 
      startY: e.clientY, 
      item, 
      hasDetached: false 
    };
  };

  const openWindow = (item: any, x: number, y: number) => {
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
  };

  const closeWindow = (id: string) => {
    setWindows(prev => prev.filter(w => w.id !== id));
  };

  const focusWindow = (id: string) => {
    setWindows(prev => prev.map(w => w.id === id ? { ...w, zIndex: nextZ } : w));
    setNextZ(z => z + 1);
  };

  useEffect(() => {
    const handleMouseMove = (e: MouseEvent) => {
      realMouse.current = { x: e.clientX, y: e.clientY };
      
      if (dragRef.current.active) {
        const dist = Math.hypot(e.clientX - dragRef.current.startX, e.clientY - dragRef.current.startY);
        if (dist > 30 && !dragRef.current.hasDetached) {
          dragRef.current.hasDetached = true;
        }

        if (dragRef.current.hasDetached) {
          setGhostPos({ x: e.clientX, y: e.clientY });
        }
      }
    };

    const handleMouseUp = (e: MouseEvent) => {
      if (dragRef.current.active) {
        if (dragRef.current.hasDetached) {
          openWindow(dragRef.current.item, e.clientX, e.clientY);
        } else {
          const dist = Math.hypot(e.clientX - dragRef.current.startX, e.clientY - dragRef.current.startY);
          if (dist < 10) {
            if (dragRef.current.item.path === '#') {
               setActiveMainView(dragRef.current.item.id);
            } else {
               navigate(dragRef.current.item.path);
            }
          }
        }
        
        dragRef.current = { active: false, startX: 0, startY: 0, item: null, hasDetached: false };
        setGhostPos(null);
      }
    };
    
    window.addEventListener('mousemove', handleMouseMove);
    window.addEventListener('mouseup', handleMouseUp);
    return () => {
      window.removeEventListener('mousemove', handleMouseMove);
      window.removeEventListener('mouseup', handleMouseUp);
    };
  }, [nextZ, navigate]);

  return (
    <div 
      ref={containerRef}
      onMouseEnter={(e) => {
        realMouse.current = { x: e.clientX, y: e.clientY };
      }}
      className="min-h-screen bg-slate-50 flex flex-col items-center justify-center p-8 overflow-hidden relative font-sans text-slate-800 select-none"
    >
      <div className="absolute inset-0 bg-[radial-gradient(circle_at_center,_var(--tw-gradient-stops))] from-white via-slate-100 to-slate-200 opacity-90 pointer-events-none" />
      
      <InteractiveGrid />

      <div className="absolute top-0 left-0 w-full h-1 bg-gradient-to-r from-transparent via-[#2e4de6]/30 to-transparent opacity-40" />
      <div className="absolute bottom-0 left-0 w-full h-1 bg-gradient-to-r from-transparent via-[#2e4de6]/30 to-transparent opacity-40" />

      {/* Main View Overlay */}
      {activeMainView && (
        <div className="fixed inset-0 z-40 bg-slate-50/80 backdrop-blur-xl animate-in fade-in duration-300">
          {/* Header - Floating */}
          <div className="absolute top-0 left-0 right-0 h-16 border-b border-slate-200 flex items-center justify-between px-8 bg-white/50 z-50">
            <div className="flex items-center gap-6">
               <button 
                 onClick={() => setActiveMainView(null)}
                 className="flex items-center gap-2 px-4 py-2 rounded-full bg-white border border-slate-200 shadow-sm text-[10px] font-bold text-slate-600 hover:text-[#2e4de6] transition-all hover:shadow-md active:scale-95"
               >
                 <svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" className="lucide lucide-arrow-left"><path d="m12 19-7-7 7-7"/><path d="M19 12H5"/></svg>
                 <span className="tracking-widest">BACK</span>
               </button>
               <div className="h-4 w-[1px] bg-slate-200 mx-2" />
               <div className="flex items-center gap-3">
                  <div className="w-2 h-2 rounded-full bg-[#2e4de6] animate-pulse" />
                  <h2 className="text-[11px] font-black tracking-[0.4em] text-slate-800 uppercase">{activeMainView}</h2>
               </div>
            </div>
            <div className="flex items-center gap-4">
               <div className="text-[9px] font-mono text-slate-400 tracking-widest uppercase">VERS-SYSTEM Interface / {activeMainView}</div>
            </div>
          </div>

          {/* Centered Content Area */}
          <div className="absolute inset-0 flex items-center justify-center p-6 md:p-12">
            <div className="w-full max-w-4xl h-full max-h-[700px] bg-white/40 backdrop-blur-md rounded-2xl border-2 border-[#2e4de6]/50 overflow-hidden flex flex-col scale-in-center animate-in fade-in zoom-in-95 duration-500">
              <Suspense fallback={<div className="flex items-center justify-center h-full text-xs font-mono text-slate-400">SYNCHRONIZING...</div>}>
                {activeMainView === 'sandbox' && <SandboxCore />}
                {activeMainView === 'plugin' && <VersPluginManager />}
                {activeMainView === 'system' && <div className="p-20 text-slate-400 font-mono text-xs flex-1 flex items-center justify-center">SYSTEM SETTINGS MODULE</div>}
              </Suspense>
            </div>
          </div>
        </div>
      )}

      {/* Windows Layer */}
      <div className="absolute inset-0 z-30 pointer-events-none">
        {windows.map(win => (
          <div key={win.id} className="pointer-events-auto">
            <GlassWindow
              id={win.id}
              title={win.title}
              initialPosition={{ x: win.x, y: win.y }}
              zIndex={win.zIndex}
              onClose={() => closeWindow(win.id)}
              onFocus={() => focusWindow(win.id)}
            >
              <Suspense fallback={<div className="flex items-center justify-center h-full text-xs font-mono text-slate-400">LOADING MODULE...</div>}>
                {win.type === 'status' && <StatusCore isWindowMode={true} />}
                {win.type === 'memory' && <MemoryCore isWindowMode={true} onClose={() => closeWindow(win.id)} />}
                {win.type === 'sandbox' && <SandboxCore />}
                {win.type === 'plugin' && <VersPluginManager />}
                {win.type === 'system' && <div className="p-8 text-slate-500 font-mono text-xs">SYSTEM / CONFIG MODULE INTEGRATED.</div>}
              </Suspense>
            </GlassWindow>
          </div>
        ))}
      </div>

      {/* Ghost Dragging Icon */}
      {ghostPos && dragRef.current.item && (
        <div 
          className="fixed z-50 pointer-events-none p-4 bg-white/50 backdrop-blur-md rounded-lg border border-[#2e4de6] text-[#2e4de6] shadow-xl animate-pulse"
          style={{ left: ghostPos.x, top: ghostPos.y, transform: 'translate(-50%, -50%)' }}
        >
          <dragRef.current.item.icon size={32} />
        </div>
      )}

      {/* Main Menu */}
      <div className="relative z-20 w-full max-w-5xl flex flex-col items-center">
        <div className="mb-16 text-center">
          <h1 className="text-4xl font-black tracking-[0.2em] text-slate-800">
            VERS SYSTEM <span className="text-xl font-black tracking-widest text-[#2e4de6] ml-1">v0.1.0</span>
          </h1>
          <p className="text-[10px] text-slate-400 mt-3 font-mono uppercase tracking-[0.4em]">
            Neural Interface / Central Archive
          </p>
        </div>

        <div className="flex justify-center items-center gap-6">
          {menuItems.map((item) => (
            <div
              key={item.id}
              onMouseDown={(e) => handleItemMouseDown(e, item)}
              className={`
                group relative w-[96px] h-[224px] border-2 bg-white/60 backdrop-blur-sm
                flex flex-col items-center py-6 shadow-sm rounded-md
                transition-all duration-300 ease-out
                ${item.disabled 
                  ? 'border-slate-300 opacity-40 cursor-not-allowed grayscale bg-slate-100' 
                  : 'border-[#2e4de6] hover:bg-white hover:shadow-[0_10px_30px_-10px_rgba(46,77,230,0.5)] cursor-pointer active:scale-95'
                }
              `}
            >
              <div className={`flex-1 flex items-center justify-center transition-all ${item.disabled ? 'text-slate-300' : 'text-[#2e4de6]'}`}>
                <item.icon size={32} strokeWidth={2} />
              </div>
              <div className={`text-[10px] font-bold tracking-[0.1em] uppercase mb-2 ${item.disabled ? 'text-slate-400' : 'text-[#2e4de6]'}`}>
                {item.label}
              </div>
            </div>
          ))}
        </div>
        
        <div className="mt-16 text-center">
           <div className="inline-flex items-center gap-2 px-4 py-1 rounded-full bg-white border border-slate-200 shadow-sm text-[10px] text-slate-400 font-mono">
              <span className="w-1.5 h-1.5 rounded-full bg-[#2e4de6] animate-pulse"></span>
              SYSTEM ONLINE
           </div>
                </div>
              </div>
        
              {isCursorActive && <CustomCursor />}
            </div>
          );
        }
        