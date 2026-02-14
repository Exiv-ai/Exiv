import React, { useState, useEffect } from 'react';
import { Cpu, ChevronRight, Puzzle, Activity, MessageSquare } from 'lucide-react';
import { AgentMetadata, PluginManifest } from '../types';
import { AgentPluginWorkspace } from './AgentPluginWorkspace';

import { api } from '../services/api';

export interface AgentTerminalProps {
  agents?: AgentMetadata[];
  plugins?: PluginManifest[];
  selectedAgent?: AgentMetadata | null;
  onSelectAgent?: (agent: AgentMetadata | null) => void;
}

export function AgentTerminal({ 
  agents: propAgents, 
  plugins: propPlugins,
  selectedAgent: propSelectedAgent, 
  onSelectAgent: propOnSelectAgent 
}: AgentTerminalProps = {}) {
  const [internalAgents, setInternalAgents] = useState<AgentMetadata[]>([]);
  const [internalPlugins, setInternalPlugins] = useState<PluginManifest[]>([]);
  const [isLoading, setIsLoading] = useState(true);
  const [internalSelectedAgent, setInternalSelectedAgent] = useState<AgentMetadata | null>(null);
  const [configuringAgent, setConfiguringAgent] = useState<AgentMetadata | null>(null);

  const agents = propAgents || internalAgents;
  const plugins = propPlugins || internalPlugins;
  const selectedAgent = propSelectedAgent !== undefined ? propSelectedAgent : internalSelectedAgent;

  useEffect(() => {
    if (!propAgents) {
      fetchInitialData();
    } else {
      setIsLoading(false);
    }
  }, [propAgents]);

  const fetchInitialData = async () => {
    try {
      const [agentsData, pluginsData] = await Promise.all([
        api.getAgents(),
        api.getPlugins()
      ]);
      setInternalAgents(agentsData);
      setInternalPlugins(pluginsData);
    } catch (err) {
      console.error('Failed to fetch data:', err);
    } finally {
      setIsLoading(false);
    }
  };

  const handleSelectAgent = (agent: AgentMetadata | null) => {
    if (propOnSelectAgent) {
      propOnSelectAgent(agent);
    } else {
      setInternalSelectedAgent(agent);
    }
  };

  if (configuringAgent) {
    return (
      <AgentPluginWorkspace 
        agent={configuringAgent}
        availablePlugins={plugins.filter(p => p.is_active)}
        onBack={() => setConfiguringAgent(null)}
      />
    );
  }

  if (selectedAgent) {
    // チャット画面（コンソール）へ遷移した状態（簡易実装）
    return (
      <div className="flex flex-col h-full bg-white/40 backdrop-blur-3xl p-8 items-center justify-center space-y-4">
        <Cpu size={48} className="text-[#2e4de6] animate-pulse" />
        <h2 className="text-xl font-black text-slate-800 tracking-widest uppercase">{selectedAgent.name} CONSOLE</h2>
        <p className="text-xs text-slate-400 font-mono tracking-widest">ESTABLISHING NEURAL LINK...</p>
        <button 
          onClick={() => handleSelectAgent(null)}
          className="mt-8 px-6 py-2 rounded-full border border-slate-200 text-[10px] font-bold text-slate-400 hover:text-[#2e4de6] transition-all"
        >
          DISCONNECT / BACK TO LIST
        </button>
      </div>
    );
  }

  return (
    <div className="flex flex-col h-full bg-white/20 backdrop-blur-3xl p-6 overflow-hidden">
      {/* Header */}
      <div className="mb-8 px-4 flex justify-between items-end">
        <div>
          <h2 className="text-2xl font-black tracking-tighter text-slate-800 uppercase leading-none">AI Containers</h2>
          <p className="text-[10px] text-slate-400 font-mono tracking-[0.3em] uppercase mt-2">VERS Registered Instances</p>
        </div>
        <div className="text-[9px] font-mono text-slate-300">SYSTEM_UPTIME: 100%</div>
      </div>

      {/* Container List */}
      <div className="flex-1 overflow-y-auto space-y-2 no-scrollbar px-2">
        {isLoading ? (
          <div className="h-full flex items-center justify-center text-slate-300 font-mono text-[10px] tracking-widest uppercase animate-pulse">
            Scanning for containers...
          </div>
        ) : (
          agents.map((agent) => (
            <div 
              key={agent.id}
              className="group flex h-24 bg-white/60 hover:bg-white border border-slate-100 rounded-2xl overflow-hidden transition-all duration-500 shadow-sm"
            >
              {/* Left / Main Info */}
              <div 
                onClick={() => handleSelectAgent(agent)}
                className="flex-1 flex items-center px-8 cursor-pointer relative overflow-hidden"
              >
                <div className="absolute left-0 top-0 w-1 h-full bg-[#2e4de6] opacity-0 group-hover:opacity-100 transition-opacity" />
                <div className="w-12 h-12 rounded-xl bg-slate-50 flex items-center justify-center text-[#2e4de6] mr-6 border border-slate-100 group-hover:bg-[#2e4de6] group-hover:text-white transition-colors duration-500">
                  <Cpu size={24} />
                </div>
                <div>
                  <div className="flex items-center gap-3">
                    <h3 className="text-lg font-black text-slate-800 tracking-tight">{agent.name}</h3>
                    <span className="flex items-center gap-1 px-2 py-0.5 rounded bg-emerald-50 text-emerald-500 text-[8px] font-bold tracking-widest uppercase">
                       <Activity size={8} /> Online
                    </span>
                  </div>
                  <p className="text-[10px] text-slate-400 mt-1 font-mono">{agent.description}</p>
                </div>
                <ChevronRight size={20} className="ml-auto text-slate-200 group-hover:text-[#2e4de6] transition-colors" />
              </div>

              {/* Right Small Square (Plugin Connection Port) */}
              <button 
                title="Manage Container Plugins"
                className="w-24 h-full border-l border-slate-100 bg-slate-50/50 hover:bg-[#2e4de6]/10 flex flex-col items-center justify-center gap-2 transition-all hover:text-[#2e4de6]"
                onClick={(e) => {
                   e.stopPropagation();
                   setConfiguringAgent(agent);
                }}
              >
                <Puzzle size={20} className="text-slate-400 group-hover:text-[#2e4de6]/80 transition-colors" />
                <span className="text-[8px] font-black tracking-tighter uppercase text-slate-400 group-hover:text-[#2e4de6]">Plugins</span>
              </button>
            </div>
          ))
        )}
      </div>

      {/* Footer / Stats */}
      <div className="mt-6 pt-6 border-t border-slate-100/50 flex justify-between items-center px-4">
        <div className="flex gap-6">
           <div className="flex flex-col">
             <span className="text-[8px] font-mono text-slate-300 uppercase">Total Memory</span>
             <span className="text-[10px] font-bold text-slate-500">2.4 GB</span>
           </div>
           <div className="flex flex-col">
             <span className="text-[8px] font-mono text-slate-300 uppercase">Active Units</span>
             <span className="text-[10px] font-bold text-slate-500">{agents.length} Agent(s)</span>
           </div>
        </div>
        <div className="p-2 rounded-lg bg-slate-50 border border-slate-100">
           <MessageSquare size={16} className="text-slate-300" />
        </div>
      </div>
    </div>
  );
}