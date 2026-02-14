import { AgentMetadata, PluginManifest } from '../types';

const API_BASE = '/api';

export const api = {
  async getAgents(): Promise<AgentMetadata[]> {
    const res = await fetch(`${API_BASE}/agents`);
    if (!res.ok) throw new Error(`Failed to fetch agents: ${res.statusText}`);
    return res.json();
  },

  async getPlugins(): Promise<PluginManifest[]> {
    const res = await fetch(`${API_BASE}/plugins`);
    if (!res.ok) throw new Error(`Failed to fetch plugins: ${res.statusText}`);
    return res.json();
  },

  async togglePlugin(id: string, isActive: boolean): Promise<void> {
    const res = await fetch(`${API_BASE}/plugins/apply`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ id, is_active: isActive })
    });
    if (!res.ok) throw new Error(`Failed to toggle plugin: ${res.statusText}`);
  },

  async getPluginConfig(id: string): Promise<Record<string, string>> {
    const res = await fetch(`${API_BASE}/plugins/${id}/config`);
    if (!res.ok) throw new Error(`Failed to get plugin config: ${res.statusText}`);
    return res.json();
  },

  async updatePluginConfig(id: string, config: Record<string, string>): Promise<void> {
    const res = await fetch(`${API_BASE}/plugins/${id}/config`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(config)
    });
    if (!res.ok) throw new Error(`Failed to update plugin config: ${res.statusText}`);
  }
};
