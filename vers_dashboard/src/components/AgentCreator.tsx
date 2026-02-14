import { useState, useEffect } from 'react';
import { User, Plus, Cpu, Activity } from 'lucide-react';
import { AgentMetadata, PluginManifest } from '../types';

export function AgentCreator({ onAgentCreated }: { onAgentCreated?: () => void }) {
  const [agents, setAgents] = useState<AgentMetadata[]>([]);
  const [plugins, setPlugins] = useState<PluginManifest[]>([]);
  const [name, setName] = useState('');
  const [description, setDescription] = useState('');
  const [engine, setEngine] = useState('');
  const [isCreating, setIsCreating] = useState(false);

  useEffect(() => {
    fetch('/api/agents').then(r => r.json()).then(setAgents).catch(console.error);
    fetch('/api/plugins').then(r => r.json()).then(setPlugins).catch(console.error);
  }, []);

  const handleCreate = async () => {
    setIsCreating(true);
    try {
      const res = await fetch('/api/agents', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ name, description, default_engine: engine })
      });
      if (res.ok) {
        // Refresh agents
        fetch('/api/agents').then(r => r.json()).then(setAgents);
        if (onAgentCreated) onAgentCreated();
        setName('');
        setDescription('');
      }
    } catch (e) {
      console.error(e);
    } finally {
      setIsCreating(false);
    }
  };

  const engines = plugins.filter(p => p.service_type === 'Reasoning');

  return (
    <div className="bg-white/80 backdrop-blur-sm p-6 rounded-lg border border-slate-200 shadow-sm h-full overflow-auto">
      <h2 className="text-lg font-bold text-slate-700 flex items-center gap-2 mb-6">
        <User size={20} className="text-[#2e4de6]" />
        Agent Management
      </h2>

      <div className="grid grid-cols-1 md:grid-cols-2 gap-8">
        {/* List */}
        <div className="space-y-4">
          <h3 className="text-xs font-bold text-slate-400 uppercase tracking-widest mb-2">Active Agents</h3>
          {agents.map(a => (
            <div key={a.id} className="p-4 bg-white border border-slate-100 rounded-lg shadow-sm flex items-center justify-between">
              <div>
                <div className="font-bold text-slate-700">{a.name}</div>
                <div className="text-xs text-slate-500">{a.description}</div>
              </div>
              <div className={`px-2 py-1 rounded text-[10px] font-bold ${a.status === 'online' ? 'bg-green-100 text-green-600' : 'bg-slate-100 text-slate-400'}`}>
                {a.status.toUpperCase()}
              </div>
            </div>
          ))}
        </div>

        {/* Create Form */}
        <div className="bg-slate-50 p-6 rounded-lg border border-slate-200">
          <h3 className="text-xs font-bold text-slate-400 uppercase tracking-widest mb-4">Initialize New Agent</h3>
          
          <div className="space-y-4">
            <div>
              <label className="block text-xs font-bold text-slate-500 mb-1">Agent Name</label>
              <input 
                type="text" 
                value={name}
                onChange={e => setName(e.target.value)}
                className="w-full px-3 py-2 rounded border border-slate-200 text-sm focus:outline-none focus:border-[#2e4de6]"
                placeholder="e.g. Mike"
              />
            </div>
            
            <div>
              <label className="block text-xs font-bold text-slate-500 mb-1">Description / System Prompt</label>
              <textarea 
                value={description}
                onChange={e => setDescription(e.target.value)}
                className="w-full px-3 py-2 rounded border border-slate-200 text-sm focus:outline-none focus:border-[#2e4de6] h-20 resize-none"
                placeholder="Briefly describe the agent's personality and role."
              />
            </div>

            <div>
              <label className="block text-xs font-bold text-slate-500 mb-1">Reasoning Engine (Brain)</label>
              <div className="grid grid-cols-1 gap-2">
                {engines.map(p => (
                  <button
                    key={p.id}
                    onClick={() => setEngine(p.id)}
                    className={`flex items-center gap-3 p-3 rounded border text-left transition-all ${
                      engine === p.id 
                        ? 'border-[#2e4de6] bg-blue-50/50' 
                        : 'border-slate-200 bg-white hover:border-slate-300'
                    }`}
                  >
                    <Cpu size={16} className={engine === p.id ? 'text-[#2e4de6]' : 'text-slate-400'} />
                    <div>
                      <div className="text-xs font-bold text-slate-700">{p.name}</div>
                      <div className="text-[10px] text-slate-400 line-clamp-1">{p.description}</div>
                    </div>
                  </button>
                ))}
              </div>
            </div>

            <button
              onClick={handleCreate}
              disabled={!name || !engine || isCreating}
              className="w-full mt-4 bg-[#2e4de6] text-white py-2 rounded text-sm font-bold shadow-sm hover:shadow-md transition-all disabled:opacity-50 disabled:cursor-not-allowed flex items-center justify-center gap-2"
            >
              {isCreating ? <Activity size={16} className="animate-spin" /> : <Plus size={16} />}
              Create Agent
            </button>
          </div>
        </div>
      </div>
    </div>
  );
}