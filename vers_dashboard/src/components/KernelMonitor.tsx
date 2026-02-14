import React from 'react';
import { Activity, Shield, Terminal } from 'lucide-react';

interface KernelMonitorProps {
  onClose: () => void;
}

export const KernelMonitor: React.FC<KernelMonitorProps> = ({ onClose }) => {
  return (
    <div className="flex flex-col h-full bg-white/20 backdrop-blur-3xl p-6 overflow-hidden animate-in fade-in duration-300">
      <div className="mb-8 px-4">
        <h2 className="text-2xl font-black tracking-tighter text-slate-800 uppercase leading-none">Kernel Monitor</h2>
        <p className="text-[10px] text-slate-400 font-mono tracking-[0.3em] uppercase mt-2">VERS System Core Status</p>
      </div>

      <div className="grid grid-cols-2 gap-4 px-4 mb-6">
        <div className="bg-white/60 p-4 rounded-2xl border border-slate-100 shadow-sm">
          <div className="flex items-center gap-2 mb-2">
            <Activity size={14} className="text-[#2e4de6]" />
            <span className="text-[9px] font-black text-slate-400 uppercase tracking-widest">Efficiency</span>
          </div>
          <div className="text-xl font-mono font-bold text-slate-800">98.2%</div>
        </div>
        <div className="bg-white/60 p-4 rounded-2xl border border-slate-100 shadow-sm">
          <div className="flex items-center gap-2 mb-2">
            <Shield size={14} className="text-emerald-500" />
            <span className="text-[9px] font-black text-slate-400 uppercase tracking-widest">Security</span>
          </div>
          <div className="text-xl font-mono font-bold text-slate-800">LOCKED</div>
        </div>
      </div>

      <div className="flex-1 bg-slate-900/5 rounded-2xl mx-4 mb-4 p-4 font-mono text-[10px] text-slate-500 overflow-y-auto no-scrollbar border border-slate-200/50">
        <div className="flex items-center gap-2 mb-1">
          <Terminal size={10} />
          <span>[SYSTEM] Kernel initialization successful.</span>
        </div>
        <div>[SYSTEM] Memory adapter bound to sqlite:./vers_memories.db</div>
        <div>[SYSTEM] DeepSeek Reasoning engine connected.</div>
        <div>[SYSTEM] All plugins synchronized and ready.</div>
        <div className="animate-pulse">_</div>
      </div>

      <div className="px-4 pb-2">
        <button 
          onClick={onClose}
          className="w-full py-3 bg-white border border-slate-200 rounded-xl text-[10px] font-bold text-slate-400 hover:text-[#2e4de6] transition-all hover:shadow-md"
        >
          CLOSE MONITOR / BACK TO HOME
        </button>
      </div>
    </div>
  );
};
