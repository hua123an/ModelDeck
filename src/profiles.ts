import { invoke } from "@tauri-apps/api/core";
import type { LaunchPreview, ToolProfile, ToolProfileInput } from "./types";
const isTauri = () => "__TAURI_INTERNALS__" in window;
export async function saveProfile(input: ToolProfileInput) { if (isTauri()) return invoke<ToolProfile>("save_tool_profile", { input }); throw new Error("浏览器预览不能保存或启动本地工具"); }
export async function deleteProfile(profileId: string) { if (isTauri()) return invoke<void>("delete_tool_profile", { profileId }); throw new Error("浏览器预览不能删除本地档案"); }
export async function previewLaunch(profileId: string) { if (isTauri()) return invoke<LaunchPreview>("preview_tool_launch", { profileId }); return { profileId, tool: "codex", executable: "/usr/local/bin/codex", isolatedHome: "~/Library/Application Support/ModelDeck/profiles/codex/demo", environment: ["CODEX_HOME=<ModelDeck 独立目录>", "OPENAI_API_KEY=<系统钥匙串>", "OPENAI_BASE_URL=https://example.com/v1"], untouchedPaths: ["~/.codex", "~/.claude", "~/.zshrc / ~/.bashrc"], commandPreview: "CODEX_HOME=… OPENAI_API_KEY=•••• codex" }; }
export async function launchProfile(profileId: string) { if (isTauri()) return invoke<void>("launch_tool_profile", { profileId }); throw new Error("浏览器预览不能启动本地工具"); }
