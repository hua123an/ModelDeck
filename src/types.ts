export type ProviderType = "new-api" | "sub2api" | "openai-compatible";
export type ModelApi = "responses" | "chat-completions" | "unknown";

export interface Provider {
  id: string;
  name: string;
  type: ProviderType;
  baseUrl: string;
  enabled: boolean;
  createdAt: string;
  updatedAt: string;
  hasApiKey: boolean;
  hasAccountToken: boolean;
  accountUserId?: string;
  hasRefreshToken?: boolean;
  tokenExpiresAt?: string;
}

export interface ProviderInput {
  id?: string;
  name: string;
  type: ProviderType;
  baseUrl: string;
  apiKey?: string;
  accountToken?: string;
  refreshToken?: string;
  tokenExpiresAt?: string;
  accountUserId?: string;
  enabled: boolean;
}

export interface ModelStatus {
  providerId: string;
  providerName: string;
  modelId: string;
  name: string;
  available: boolean;
  api: ModelApi;
  latencyMs?: number;
  lastCheckedAt?: string;
  error?: string;
  statusCode?: number;
}

export interface BalanceInfo {
  providerId: string;
  supported: boolean;
  balance?: number;
  quota?: number;
  used?: number;
  resetAt?: string;
  multiplier?: number;
  accountName?: string;
  frozenBalance?: number;
  group?: string;
  subscriptionName?: string;
  subscriptionStatus?: string;
  error?: string;
  checkedAt: string;
}

export interface ToolProfile {
  id: string;
  name: string;
  tool: "codex" | "claude";
  providerId: string;
  model?: string;
  executable?: string;
  active: boolean;
  createdAt: string;
  updatedAt: string;
}

export interface ToolProfileInput {
  id?: string;
  name: string;
  tool: "codex" | "claude";
  providerId: string;
  model?: string;
  executable?: string;
}

export interface LaunchPreview {
  profileId: string;
  tool: string;
  executable: string;
  terminal: string;
  isolatedHome?: string;
  environment: string[];
  untouchedPaths: string[];
  commandPreview: string;
}

export interface Snapshot {
  providers: Provider[];
  models: ModelStatus[];
  balances: BalanceInfo[];
  profiles: ToolProfile[];
}

export interface AccountSummary {
  providerId: string;
  providerName: string;
  providerType: ProviderType;
  supported: boolean;
  accountName?: string;
  balance?: number;
  used?: number;
  frozenBalance?: number;
  requestCount?: number;
  group?: string;
  multiplier?: number;
  subscriptionName?: string;
  subscriptionStatus?: string;
  expiresAt?: string;
  tokenExpiresAt?: string;
  error?: string;
  checkedAt: string;
}

export interface ManagedGroup {
  providerId: string;
  id: string;
  name: string;
  description?: string;
  multiplier?: number;
  subscription: boolean;
  available: boolean;
}

export interface ManagedKey {
  providerId: string;
  providerName: string;
  id: string;
  name: string;
  maskedKey: string;
  enabled: boolean;
  groupId?: string;
  groupName?: string;
  quota?: number;
  used?: number;
  unlimited: boolean;
  expiresAt?: string;
  lastUsedAt?: string;
}

export interface ManagedKeyInput {
  providerId: string;
  id?: string;
  name: string;
  enabled: boolean;
  groupId?: string;
  quota?: number;
  unlimited: boolean;
  expiresAt?: string;
}

export interface UsageSummary {
  providerId: string;
  providerName: string;
  totalRequests: number;
  totalTokens?: number;
  totalCost?: number;
  todayRequests?: number;
  todayTokens?: number;
  todayCost?: number;
  checkedAt: string;
}

export interface StorageInfo {
  dataFile: string;
  keyStorage: string;
}
