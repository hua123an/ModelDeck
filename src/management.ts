import { invoke } from "@tauri-apps/api/core";
import type { AccountSummary, ManagedGroup, ManagedKey, ManagedKeyInput, UsageSummary } from "./types";
const isTauri = () => "__TAURI_INTERNALS__" in window;
const wait = (ms = 350) => new Promise((resolve) => setTimeout(resolve, ms));
const accounts: Record<string, AccountSummary> = {
  "preview-2": { providerId: "preview-2", providerName: "Team New API", providerType: "new-api", supported: true, accountName: "demo-user", balance: 38.42, used: 61.58, requestCount: 1248, group: "default", multiplier: 1, checkedAt: new Date().toISOString() },
};
const groups: Record<string, ManagedGroup[]> = {
  "preview-2": [{ providerId: "preview-2", id: "default", name: "default", description: "默认分组", multiplier: 1, subscription: false, available: true }, { providerId: "preview-2", id: "vip", name: "vip", description: "VIP 高优先级", multiplier: 0.8, subscription: false, available: true }],
};
let keys: ManagedKey[] = [{ providerId: "preview-2", providerName: "Team New API", id: "1", name: "Coding", maskedKey: "sk-••••••••a81f", enabled: true, groupId: "default", groupName: "default", quota: 20, used: 5.74, unlimited: false, lastUsedAt: new Date().toISOString() }, { providerId: "preview-2", providerName: "Team New API", id: "2", name: "Playground", maskedKey: "sk-••••••••18c2", enabled: false, groupId: "vip", groupName: "vip", unlimited: true }];
export async function syncAccount(providerId: string) { if (isTauri()) return invoke<AccountSummary>("sync_account", { providerId }); await wait(); return accounts[providerId] ?? { providerId, providerName: "Compatible", providerType: "openai-compatible", supported: false, error: "兼容站没有统一账户管理协议", checkedAt: new Date().toISOString() }; }
export async function listGroups(providerId: string) { if (isTauri()) return invoke<ManagedGroup[]>("list_groups", { providerId }); await wait(); return groups[providerId] ?? []; }
export async function listManagedKeys(providerId: string) { if (isTauri()) return invoke<ManagedKey[]>("list_managed_keys", { providerId }); await wait(); return keys.filter((key) => key.providerId === providerId); }
export async function saveManagedKey(input: ManagedKeyInput) { if (isTauri()) return invoke<void>("save_managed_key", { input }); await wait(); if (input.id) keys = keys.map((key) => key.id === input.id && key.providerId === input.providerId ? { ...key, ...input, maskedKey: key.maskedKey, providerName: key.providerName, groupName: input.groupId } : key); else keys.push({ ...input, id: crypto.randomUUID(), providerName: "Team New API", maskedKey: "sk-••••••••new", groupName: input.groupId }); }
export async function toggleManagedKey(providerId: string, keyId: string, enabled: boolean) { if (isTauri()) return invoke<void>("toggle_managed_key", { providerId, keyId, enabled }); await wait(); keys = keys.map((key) => key.providerId === providerId && key.id === keyId ? { ...key, enabled } : key); }
export async function deleteManagedKey(providerId: string, keyId: string) { if (isTauri()) return invoke<void>("delete_managed_key", { providerId, keyId }); await wait(); keys = keys.filter((key) => !(key.providerId === providerId && key.id === keyId)); }
export async function loadUsage(providerId: string) { if (isTauri()) return invoke<UsageSummary>("load_usage", { providerId }); await wait(); return { providerId, providerName: "Team New API", totalRequests: 1248, totalTokens: 8_420_310, totalCost: 61.58, todayRequests: 84, todayTokens: 421_390, todayCost: 3.27, checkedAt: new Date().toISOString() }; }
