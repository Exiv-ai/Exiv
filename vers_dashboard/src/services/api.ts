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

  async applyPluginSettings(settings: { id: string, is_active: boolean }[]): Promise<void> {
    const res = await fetch(`${API_BASE}/plugins/apply`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(settings)
    });
    if (!res.ok) throw new Error(`Failed to apply plugin settings: ${res.statusText}`);
  },

  async getPluginConfig(id: string): Promise<Record<string, string>> {
    const res = await fetch(`${API_BASE}/plugins/${id}/config`);
    if (!res.ok) throw new Error(`Failed to get plugin config: ${res.statusText}`);
    return res.json();
  },

  async updatePluginConfig(id: string, payload: { key: string, value: string }): Promise<void> {
    const res = await fetch(`${API_BASE}/plugins/${id}/config`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(payload)
    });
    if (!res.ok) throw new Error(`Failed to update plugin config: ${res.statusText}`);
  },

  async updateAgent(id: string, payload: { metadata: Record<string, string> }): Promise<void> {
    const res = await fetch(`${API_BASE}/agents/${id}`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(payload)
    });
    if (!res.ok) throw new Error(`Failed to update agent: ${res.statusText}`);
  },

  async grantPermission(pluginId: string, permission: string): Promise<void> {
    const res = await fetch(`${API_BASE}/plugins/${pluginId}/permissions/grant`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ permission })
    });
    if (!res.ok) throw new Error(`Failed to grant permission: ${res.statusText}`);
  }
};
