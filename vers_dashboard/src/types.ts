export type VersId = string;

export interface VersMessage {
  id: VersId;
  source: {
    type: 'User' | 'Agent' | 'System';
    id?: string;
    name?: string;
  };
  target_agent?: VersId;
  content: string;
  timestamp: string;
  metadata: Record<string, string>;
}

export interface AgentMetadata {
  id: VersId;
  name: string;
  description: string;
  capabilities: Capability[];
  status: 'online' | 'offline' | 'busy';
}

export type Capability = 
  | 'VisionRead' 
  | 'InputControl' 
  | 'FileRead' 
  | 'FileWrite' 
  | 'NetworkAccess' 
  | 'ProcessExecution' 
  | 'MemoryRead' 
  | 'MemoryWrite';

export interface PluginManifest {
  id: VersId;
  name: string;
  description: string;
  version: string;
  service_type: ServiceType;
  tags: string[];
  is_active: boolean;
  is_configured: boolean;
  required_config_keys: string[];
  action_icon?: string;
  action_target?: string;
  required_capabilities: Capability[];
  provided_tools: string[];
}

export type ServiceType = 
  | 'Communication'
  | 'Reasoning'
  | 'Skill'
  | 'Vision'
  | 'Action'
  | 'Memory'
  | 'HAL';

export interface ColorVisionData {
  captured_at: string;
  detected_elements: DetectedElement[];
}

export interface DetectedElement {
  label: string;
  bounds: [number, number, number, number];
  confidence: number;
}