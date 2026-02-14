export type VersId = string;

export interface VersMessage {
  id: string;
  source: 
    | { type: 'User'; id: string; name: string }
    | { type: 'Agent'; id: string }
    | { type: 'System' };
  target_agent?: string;
  content: string;
  timestamp: string;
  metadata: Record<string, string>;
}

export interface AgentMetadata {
  id: VersId;
  name: string;
  description: string;
  required_capabilities: CapabilityType[];
  status: 'online' | 'offline' | 'busy';
  metadata: Record<string, string>;
}

export type Permission = 
  | 'VisionRead' 
  | 'InputControl' 
  | 'FileRead' 
  | 'FileWrite' 
  | 'NetworkAccess' 
  | 'ProcessExecution' 
  | 'MemoryRead' 
  | 'MemoryWrite';

export type CapabilityType =
  | 'Reasoning'
  | 'Memory'
  | 'Communication'
  | 'Tool'
  | 'Vision'
  | 'HAL';

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
  magic_seal: number;
  sdk_version: string;
  required_permissions: Permission[];
  provided_capabilities: CapabilityType[];
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