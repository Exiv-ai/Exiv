import { AgentMetadata, PluginManifest } from '../types';

const API_URL = import.meta.env.VITE_API_URL || `${window.location.origin}/api`;
export const API_BASE = API_URL.endsWith('/api') ? API_URL : `${API_URL}/api`;

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

  async updateAgent(id: string, payload: { default_engine_id?: string, metadata: Record<string, string> }): Promise<void> {
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
  },

  async postEvent(eventData: any): Promise<void> {
    const res = await fetch(`${API_BASE}/events/publish`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(eventData)
    });
    if (!res.ok) throw new Error(`Failed to post event: ${res.statusText}`);
  },

  async post(path: string, payload: any): Promise<void> {
    const res = await fetch(`${API_BASE}${path}`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(payload)
    });
    if (!res.ok) throw new Error(`Failed to post to ${path}: ${res.statusText}`);
  },

  async getPendingPermissions(): Promise<any[]> {
    const res = await fetch(`${API_BASE}/permissions/pending`);
    if (!res.ok) throw new Error(`Failed to fetch pending permissions: ${res.statusText}`);
    return res.json();
  },

  async approvePermission(requestId: string, approvedBy: string): Promise<void> {
    const res = await fetch(`${API_BASE}/permissions/${requestId}/approve`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ approved_by: approvedBy })
    });
    if (!res.ok) throw new Error(`Failed to approve permission: ${res.statusText}`);
  },

  async denyPermission(requestId: string, approvedBy: string): Promise<void> {
    const res = await fetch(`${API_BASE}/permissions/${requestId}/deny`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ approved_by: approvedBy })
    });
    if (!res.ok) throw new Error(`Failed to deny permission: ${res.statusText}`);
  }
};
