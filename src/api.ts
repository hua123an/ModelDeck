import { invoke } from "@tauri-apps/api/core";
import type { BalanceInfo, ModelStatus, Provider, ProviderInput, Snapshot, StorageInfo } from "./types";

const isTauri = () => "__TAURI_INTERNALS__" in window;
const STORE = "modeldeck-preview";
const now = () => new Date().toISOString();

const initial: Snapshot = {
  providers: [
    { id: "preview-1", name: "Northstar AI", type: "openai-compatible", baseUrl: "https://api.example.com", enabled: true, createdAt: now(), updatedAt: now(), hasApiKey: true, hasAccountToken: false },
    { id: "preview-2", name: "Team New API", type: "new-api", baseUrl: "https://newapi.example.com", enabled: true, createdAt: now(), updatedAt: now(), hasApiKey: true, hasAccountToken: true, accountUserId: "42" },
  ],
  models: [
    { providerId: "preview-1", providerName: "Northstar AI", modelId: "gpt-4.1-mini", name: "GPT 4.1 Mini", available: true, api: "responses", latencyMs: 842, lastCheckedAt: now(), statusCode: 200 },
    { providerId: "preview-1", providerName: "Northstar AI", modelId: "gpt-4o-mini", name: "GPT 4o Mini", available: true, api: "chat-completions", latencyMs: 629, lastCheckedAt: now(), statusCode: 200 },
    { providerId: "preview-2", providerName: "Team New API", modelId: "claude-3-5-haiku", name: "Claude 3.5 Haiku", available: false, api: "unknown", latencyMs: 1134, lastCheckedAt: now(), statusCode: 404, error: "模型端点未启用" },
  ],
  balances: [{ providerId: "preview-2", supported: true, quota: 38.42, used: 61.58, multiplier: 1, accountName: "demo-user", group: "default", checkedAt: now() }],
  profiles: [],
};

function read(): Snapshot {
  const value = localStorage.getItem(STORE);
  if (!value) { localStorage.setItem(STORE, JSON.stringify(initial)); return initial; }
  return JSON.parse(value) as Snapshot;
}
function write(snapshot: Snapshot) { localStorage.setItem(STORE, JSON.stringify(snapshot)); }
function delay(ms = 450) { return new Promise((resolve) => setTimeout(resolve, ms)); }

export async function getSnapshot(): Promise<Snapshot> {
  return isTauri() ? invoke("get_snapshot") : read();
}

export async function saveProvider(input: ProviderInput): Promise<Provider> {
  if (isTauri()) return invoke("save_provider", { input });
  await delay(); const snapshot = read(); const existing = snapshot.providers.find((item) => item.id === input.id);
  const provider: Provider = { id: input.id ?? crypto.randomUUID(), name: input.name, type: input.type, baseUrl: input.baseUrl.replace(/\/$/, ""), enabled: input.enabled, createdAt: existing?.createdAt ?? now(), updatedAt: now(), hasApiKey: Boolean(input.apiKey || existing?.hasApiKey), hasAccountToken: Boolean(input.accountToken || existing?.hasAccountToken), accountUserId: input.accountUserId || existing?.accountUserId, hasRefreshToken: Boolean(input.refreshToken || existing?.hasRefreshToken), tokenExpiresAt: input.tokenExpiresAt || existing?.tokenExpiresAt };
  snapshot.providers = existing ? snapshot.providers.map((item) => item.id === provider.id ? provider : item) : [...snapshot.providers, provider]; write(snapshot); return provider;
}

export async function deleteProvider(providerId: string): Promise<void> {
  if (isTauri()) return invoke("delete_provider", { providerId });
  const snapshot = read(); snapshot.providers = snapshot.providers.filter((item) => item.id !== providerId); snapshot.models = snapshot.models.filter((item) => item.providerId !== providerId); snapshot.balances = snapshot.balances.filter((item) => item.providerId !== providerId); write(snapshot);
}

export async function toggleProvider(providerId: string, enabled: boolean): Promise<void> {
  if (isTauri()) return invoke("toggle_provider", { providerId, enabled });
  const snapshot = read(); snapshot.providers = snapshot.providers.map((item) => item.id === providerId ? { ...item, enabled, updatedAt: now() } : item); write(snapshot);
}

export async function fetchModels(providerId: string): Promise<ModelStatus[]> {
  if (isTauri()) return invoke("fetch_models", { providerId });
  await delay(700); return read().models.filter((item) => item.providerId === providerId);
}

export async function testModel(providerId: string, modelId: string): Promise<ModelStatus> {
  if (isTauri()) return invoke("test_model", { providerId, modelId });
  await delay(500); const snapshot = read(); const current = snapshot.models.find((item) => item.providerId === providerId && item.modelId === modelId); if (!current) throw new Error("模型不存在");
  const result = { ...current, available: true, api: current.api === "unknown" ? "chat-completions" as const : current.api, latencyMs: Math.round(480 + Math.random() * 620), lastCheckedAt: now(), statusCode: 200, error: undefined };
  snapshot.models = snapshot.models.map((item) => item.providerId === providerId && item.modelId === modelId ? result : item); write(snapshot); return result;
}

export async function queryBalance(providerId: string): Promise<BalanceInfo> {
  if (isTauri()) return invoke("query_balance", { providerId });
  await delay(650); const snapshot = read(); const existing = snapshot.balances.find((item) => item.providerId === providerId); const result = existing ?? { providerId, supported: false, error: "该服务商暂不支持余额查询", checkedAt: now() };
  snapshot.balances = [...snapshot.balances.filter((item) => item.providerId !== providerId), result]; write(snapshot); return result;
}

export async function storageInfo(): Promise<StorageInfo> {
  return isTauri() ? invoke("storage_info") : { dataFile: "~/Library/Application Support/com.huaan.modeldeck/modeldeck.json", keyStorage: "操作系统钥匙串（桌面应用中启用）" };
}

export async function exportPiModels(): Promise<string> {
  if (isTauri()) return invoke("export_pi_models");
  const snapshot = read(); const output = { providers: snapshot.providers.filter((p) => p.enabled).map((p) => ({ id: p.id, name: p.name, baseUrl: `${p.baseUrl}/v1`, apiKey: `MODELDECK_KEY_${p.id.replace(/-/g, "_").toUpperCase()}`, models: snapshot.models.filter((m) => m.providerId === p.id).map((m) => ({ id: m.modelId, name: m.name, api: m.api })) })) };
  const anchor = document.createElement("a"); anchor.href = URL.createObjectURL(new Blob([JSON.stringify(output, null, 2)], { type: "application/json" })); anchor.download = "models.json"; anchor.click(); URL.revokeObjectURL(anchor.href); return "浏览器下载目录/models.json";
}
