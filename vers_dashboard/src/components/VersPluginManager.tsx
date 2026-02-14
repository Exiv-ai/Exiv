import React, { useState, useEffect } from 'react';
import { Puzzle, Shield, CheckCircle2, AlertTriangle, Save, Filter, Brain, Database, Zap, Globe, Settings, MousePointer2, ExternalLink, Terminal } from 'lucide-react';
import { PluginManifest } from '../types';

import { api } from '../services/api';

const MANDATORY_TAGS = ['#CORE', '#MIND', '#MEMORY', '#LLM', '#TOOL', '#ADAPTER', '#HAL'];

function ConfigModal({ plugin, onClose }: { plugin: PluginManifest, onClose: () => void }) {
  const [config, setConfig] = useState<Record<string, string>>({});
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    api.getPluginConfig(plugin.id)
      .then(setConfig)
      .catch(console.error)
      .finally(() => setLoading(false));
  }, [plugin.id]);

  const save = async (key: string, value: string) => {
    const newConfig = { ...config, [key]: value };
    await api.updatePluginConfig(plugin.id, { key, value });
    setConfig(newConfig);
  };

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/50 backdrop-blur-sm p-4" onClick={onClose}>
      <div className="bg-white rounded-2xl w-full max-w-lg p-6 shadow-2xl animate-in zoom-in-95 duration-200" onClick={e => e.stopPropagation()}>
        <h3 className="text-lg font-bold mb-4 flex items-center gap-2 text-slate-800">
          <Settings size={18} className="text-[#2e4de6]" />
          Configure {plugin.name}
        </h3>
        
        {loading ? (
          <div className="p-8 text-center text-slate-400 font-mono text-xs">Loading config...</div>
        ) : (
          <div className="space-y-4">
            {plugin.required_config_keys.length > 0 ? plugin.required_config_keys.map(key => (
              <div key={key}>
                <label className="block text-xs font-bold text-slate-500 mb-1 uppercase tracking-wider">{key}</label>
                <input 
                  type={key.includes('key') || key.includes('password') ? 'password' : 'text'}
                  value={config[key] || ''}
                  onChange={e => setConfig(prev => ({ ...prev, [key]: e.target.value }))}
                  onBlur={e => save(key, e.target.value)}
                  className="w-full px-3 py-2 rounded-lg border border-slate-200 text-sm font-mono focus:outline-none focus:border-[#2e4de6] focus:ring-1 focus:ring-[#2e4de6]"
                  placeholder={`Enter ${key}...`}
                />
              </div>
            )) : (
              <div className="p-4 bg-slate-50 text-slate-400 text-xs rounded-lg text-center font-mono border border-slate-100 border-dashed">
                No configuration required for this plugin.
              </div>
            )}
          </div>
        )}

        <div className="mt-6 flex justify-end gap-3">
          <button onClick={onClose} className="px-6 py-2 bg-slate-100 text-slate-600 rounded-lg text-xs font-bold hover:bg-slate-200 transition-colors tracking-wide">
            CANCEL
          </button>
          <button 
            onClick={async (e) => {
              e.preventDefault();
              const btn = e.currentTarget;
              btn.disabled = true;
              btn.innerText = "SAVING...";
              
              try {
                // Save all keys in parallel with correct format { key, value }
                await Promise.all(
                  Object.entries(config).map(([key, value]) => 
                    api.updatePluginConfig(plugin.id, { key, value })
                  )
                );
                onClose();
              } catch (err) {
                console.error("Failed to save config:", err);
                btn.disabled = false;
                btn.innerText = "SAVE ERROR (TRY AGAIN)";
              }
            }} 
            className="px-6 py-2 bg-[#2e4de6] text-white rounded-lg text-xs font-bold hover:bg-[#1e3bb3] transition-all shadow-md shadow-[#2e4de6]/20 tracking-wide disabled:opacity-50"
          >
            SAVE & CLOSE
          </button>
        </div>
      </div>
    </div>
  );
}

export function VersPluginManager() {
  const [plugins, setPlugins] = useState<PluginManifest[]>([]);
  const [editingPlugins, setEditingPlugins] = useState<PluginManifest[]>([]);
  const [selectedTags, setSelectedTags] = useState<string[]>([]);
  const [isLoading, setIsLoading] = useState(true);
  const [isSaving, setIsSaving] = useState(false);
  const [configTarget, setConfigTarget] = useState<PluginManifest | null>(null);

  useEffect(() => {
    fetchPlugins();
  }, []);

  const fetchPlugins = async () => {
    setIsLoading(true);
    try {
      const data = await api.getPlugins();
      setPlugins(data);
      setEditingPlugins(JSON.parse(JSON.stringify(data)));
    } catch (err) {
      console.error('Failed to fetch plugins:', err);
    } finally {
      setIsLoading(false);
    }
  };

  const togglePlugin = (id: string) => {
    setEditingPlugins(prev => prev.map(p => 
      p.id === id ? { ...p, is_active: !p.is_active } : p
    ));
  };

  const hasChanges = JSON.stringify(plugins) !== JSON.stringify(editingPlugins);

  const isPluginPending = (id: string) => {
    const original = plugins.find(p => p.id === id);
    const editing = editingPlugins.find(p => p.id === id);
    return JSON.stringify(original) !== JSON.stringify(editing);
  };

  const applyChanges = async () => {
    setIsSaving(true);
    try {
      const changes = editingPlugins
        .filter(p => {
          const original = plugins.find(orig => orig.id === p.id);
          return original && original.is_active !== p.is_active;
        })
        .map(p => ({ id: p.id, is_active: p.is_active }));

      if (changes.length > 0) {
        await api.applyPluginSettings(changes);
      }
      setPlugins(JSON.parse(JSON.stringify(editingPlugins)));
    } catch (err) {
      console.error('Failed to apply plugin changes:', err);
    } finally {
      setIsSaving(false);
    }
  };

  const allTags = Array.from(new Set(plugins.flatMap(p => p.tags)));
  
  const filteredPlugins = editingPlugins.filter(p => 
    selectedTags.length === 0 || selectedTags.some(tag => p.tags.includes(tag))
  );

  const getActionIcon = (iconName?: string) => {
    switch(iconName) {
      case 'Settings': return <Settings size={14} />;
      case 'Database': return <Database size={14} />;
      case 'MousePointer2': return <MousePointer2 size={14} />;
      case 'ExternalLink': return <ExternalLink size={14} />;
      case 'Terminal': return <Terminal size={14} />;
      default: return <Settings size={14} />;
    }
  };

  const getIcon = (type: string) => {
    switch(type) {
      case 'Reasoning': return <Brain size={20} />;
      case 'Memory': return <Database size={20} />;
      case 'Skill': return <Zap size={20} />;
      case 'Communication': return <Globe size={20} />;
      default: return <Puzzle size={20} />;
    }
  };

  return (
    <div className="flex flex-col h-full bg-white/40 backdrop-blur-3xl overflow-hidden">
      {/* Header */}
      <div className="p-6 border-b border-slate-100 flex items-center justify-between bg-white/40">
        <div>
          <h2 className="text-xl font-black tracking-tight text-slate-800 uppercase">System Plugins</h2>
          <p className="text-[10px] text-slate-400 font-mono tracking-widest uppercase mt-1">
            VERS-SYSTEM Kernel v0.3.3 / Configuration Panel
          </p>
        </div>
        <div className="flex items-center gap-3">
           <div className="px-3 py-1 rounded-full bg-slate-100 text-[10px] font-bold text-slate-500">
             {plugins.filter(p => p.is_active).length} / {plugins.length} ACTIVE
           </div>
        </div>
      </div>

      <div className="flex-1 flex overflow-hidden">
        {/* Sidebar - Tags */}
        <div className="w-64 border-r border-slate-100 bg-slate-50/30 p-6 flex flex-col gap-6">
          <div>
            <div className="flex items-center gap-2 text-slate-400 mb-4">
              <Filter size={14} />
              <span className="text-[10px] font-black uppercase tracking-widest">Tag Filters</span>
            </div>
            <div className="flex flex-wrap gap-2">
              {allTags.map(tag => (
                <button
                  key={tag}
                  onClick={() => setSelectedTags(prev => 
                    prev.includes(tag) ? prev.filter(t => t !== tag) : [...prev, tag]
                  )}
                  className={`px-3 py-1.5 rounded-lg text-[10px] font-bold transition-all ${
                    selectedTags.includes(tag)
                      ? 'bg-[#2e4de6] text-white shadow-md'
                      : 'bg-white text-slate-500 border border-slate-200 hover:border-[#2e4de6]/50'
                  }`}
                >
                  {tag}
                </button>
              ))}
            </div>
          </div>

          <div className="mt-auto">
             <div className="p-4 rounded-2xl bg-[#2e4de6]/5 border border-[#2e4de6]/10 text-[#2e4de6]">
               <div className="flex items-center gap-2 mb-2">
                 <Shield size={16} />
                 <span className="text-[10px] font-black uppercase tracking-widest">Security</span>
               </div>
               <p className="text-[9px] leading-relaxed opacity-80">
                 Only plugins signed with the official VERS SDK (Magic Seal) are marked as VERIFIED.
               </p>
             </div>
          </div>
        </div>

        {/* Main Area - Plugin Cards */}
        <div className="flex-1 overflow-y-auto p-6 no-scrollbar">
          <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
            {filteredPlugins.map(plugin => {
              const isVerified = plugin.magic_seal === 0x56455253;
              return (
                <div 
                  key={plugin.id}
                  className={`group relative p-5 rounded-2xl border transition-all duration-300 ${
                    plugin.is_active 
                      ? 'bg-white border-slate-200 shadow-sm' 
                      : 'bg-slate-50/50 border-slate-100 opacity-60'
                  }`}
                >
                  <div className="flex items-start justify-between mb-4">
                    <div className={`p-2.5 rounded-xl ${plugin.is_active ? 'bg-[#2e4de6]/10 text-[#2e4de6]' : 'bg-slate-200 text-slate-400'}`}>
                      {getIcon(plugin.service_type)}
                    </div>
                    <button
                      onClick={() => togglePlugin(plugin.id)}
                      className={`w-12 h-6 rounded-full relative transition-colors duration-300 ${
                        plugin.is_active ? 'bg-[#2e4de6]' : 'bg-slate-300'
                      }`}
                    >
                      <div className={`absolute top-1 w-4 h-4 bg-white rounded-full transition-all duration-300 ${
                        plugin.is_active ? 'left-7' : 'left-1'
                      }`} />
                    </button>
                  </div>

                  <div className="mb-4">
                    <div className="flex items-center gap-2">
                      <h3 className="font-bold text-slate-800 text-sm">{plugin.name}</h3>
                      {isVerified ? (
                        <CheckCircle2 size={14} className="text-emerald-500" title={`Verified (SDK v${plugin.sdk_version})`} />
                      ) : (
                        <AlertTriangle size={14} className="text-amber-500" title="Unverified Plugin" />
                      )}
                    </div>
                    <p className="text-[11px] text-slate-500 mt-1 line-clamp-2 leading-relaxed">
                      {plugin.description}
                    </p>
                  </div>

                  <div className="flex flex-wrap gap-1.5 mt-auto">
                    {plugin.tags.map(tag => (
                      <span key={tag} className="px-2 py-0.5 bg-slate-100 rounded text-[9px] font-mono text-slate-400">
                        {tag}
                      </span>
                    ))}
                    {!isVerified && (
                      <span className="px-2 py-0.5 bg-amber-100 text-amber-600 rounded text-[9px] font-black uppercase tracking-tighter">
                        UNVERIFIED
                      </span>
                    )}
                    {isVerified && (
                      <span className="px-2 py-0.5 bg-emerald-50 text-emerald-600 rounded text-[9px] font-mono">
                        v{plugin.sdk_version}
                      </span>
                    )}
                  </div>

                  {/* Action Icon (Bottom Right) */}
                  {plugin.action_icon && (
                    <button
                      disabled={!plugin.is_active || isPluginPending(plugin.id)}
                      onClick={() => setConfigTarget(plugin)}
                      className={`absolute bottom-4 right-4 p-2 rounded-lg transition-all ${
                        plugin.is_active && !isPluginPending(plugin.id)
                          ? 'bg-[#2e4de6]/10 text-[#2e4de6] hover:bg-[#2e4de6] hover:text-white shadow-sm'
                          : 'bg-slate-100 text-slate-300 cursor-not-allowed opacity-50'
                      }`}
                      title={!plugin.is_active ? "Activate plugin to configure" : isPluginPending(plugin.id) ? "Apply changes to configure" : "Plugin Settings"}
                    >
                      {getActionIcon(plugin.action_icon)}
                    </button>
                  )}
                </div>
              );
            })}
          </div>
        </div>
      </div>

      {/* Footer - Apply Bar */}
      {hasChanges && (
        <div className="p-4 bg-white border-t border-slate-100 flex items-center justify-between animate-in slide-in-from-bottom-full duration-500">
          <div className="flex items-center gap-2 text-amber-600 px-4">
            <AlertTriangle size={16} />
            <span className="text-[10px] font-bold uppercase tracking-widest">Pending changes exist</span>
          </div>
          <button
            onClick={applyChanges}
            disabled={isSaving}
            className="flex items-center gap-2 px-6 py-2.5 bg-[#2e4de6] text-white rounded-xl text-xs font-bold shadow-lg shadow-[#2e4de6]/20 hover:scale-105 active:scale-95 transition-all disabled:opacity-50"
          >
            {isSaving ? (
              <div className="w-4 h-4 border-2 border-white/20 border-t-white rounded-full animate-spin" />
            ) : <Save size={16} />}
            APPLY CONFIGURATION
          </button>
        </div>
      )}

      {/* Config Modal */}
      {configTarget && (
        <ConfigModal plugin={configTarget} onClose={() => setConfigTarget(null)} />
      )}
    </div>
  );
}
