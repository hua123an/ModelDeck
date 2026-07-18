import { invoke } from "@tauri-apps/api/core";
import type { LaunchPreview, ToolProfile, ToolProfileInput } from "./types";
const isTauri = () => "__TAURI_INTERNALS__" in window;
export async function saveProfile(input: ToolProfileInput) { if (isTauri()) return invoke<ToolProfile>("save_tool_profile", { input }); throw new Error("浏览器预览不能保存本地档案"); }
export async function deleteProfile(profileId: string) { if (isTauri()) return invoke<void>("delete_tool_profile", { profileId }); throw new Error("浏览器预览不能删除本地档案"); }
export async function previewLaunch(profileId: string) { if (isTauri()) return invoke<LaunchPreview>("preview_tool_launch", { profileId }); return { profileId, tool: "codex", targetFile: "~/.codex/config.toml", backupDirectory: "~/Library/Application Support/ModelDeck/config-backups/codex", changes: ["model = gpt-5", "model_provider = modeldeck", "保留 auth.json 和未知字段"], untouchedPaths: ["~/.codex/auth.json", "~/.codex/history.jsonl", "项目目录与登录态"] }; }
export async function applyProfile(profileId: string) { if (isTauri()) return invoke<void>("launch_tool_profile", { profileId }); throw new Error("浏览器预览不能修改系统配置"); }
export async function restoreConfig(tool: "codex" | "claude") { if (isTauri()) return invoke<string>("restore_tool_config", { tool }); throw new Error("浏览器预览不能恢复系统配置"); }
