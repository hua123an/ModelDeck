import type { SVGProps } from "react";

function Icon({ children, ...props }: SVGProps<SVGSVGElement>) {
  return <svg viewBox="0 0 24 24" width="18" height="18" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true" {...props}>{children}</svg>;
}
export const ServerIcon = () => <Icon><rect x="3" y="4" width="18" height="6" rx="2"/><rect x="3" y="14" width="18" height="6" rx="2"/><path d="M7 7h.01M7 17h.01"/></Icon>;
export const BoxesIcon = () => <Icon><path d="m12 2 4 2.2v4.6L12 11 8 8.8V4.2L12 2Z"/><path d="m6 12 4 2.2v4.6L6 21l-4-2.2v-4.6L6 12Zm12 0 4 2.2v4.6L18 21l-4-2.2v-4.6L18 12Z"/></Icon>;
export const SettingsIcon = () => <Icon><circle cx="12" cy="12" r="3"/><path d="M19.4 15a1.7 1.7 0 0 0 .3 1.9l.1.1-2.8 2.8-.1-.1a1.7 1.7 0 0 0-1.9-.3 1.7 1.7 0 0 0-1 1.6v.2h-4V21a1.7 1.7 0 0 0-1-1.6 1.7 1.7 0 0 0-1.9.3l-.1.1L4.2 17l.1-.1a1.7 1.7 0 0 0 .3-1.9A1.7 1.7 0 0 0 3 14H2.8v-4H3a1.7 1.7 0 0 0 1.6-1 1.7 1.7 0 0 0-.3-1.9L4.2 7 7 4.2l.1.1A1.7 1.7 0 0 0 9 4.6a1.7 1.7 0 0 0 1-1.6v-.2h4V3a1.7 1.7 0 0 0 1 1.6 1.7 1.7 0 0 0 1.9-.3l.1-.1L19.8 7l-.1.1a1.7 1.7 0 0 0-.3 1.9 1.7 1.7 0 0 0 1.6 1h.2v4H21a1.7 1.7 0 0 0-1.6 1Z"/></Icon>;
export const PlusIcon = () => <Icon><path d="M12 5v14M5 12h14"/></Icon>;
export const RefreshIcon = () => <Icon><path d="M20 11a8 8 0 1 0-2.3 5.7L20 14"/><path d="M20 5v6h-6"/></Icon>;
export const MoreIcon = () => <Icon><circle cx="5" cy="12" r="1" fill="currentColor" stroke="none"/><circle cx="12" cy="12" r="1" fill="currentColor" stroke="none"/><circle cx="19" cy="12" r="1" fill="currentColor" stroke="none"/></Icon>;
export const PencilIcon = () => <Icon><path d="m4 20 4.5-1 10-10a2.1 2.1 0 0 0-3-3l-10 10L4 20Z"/><path d="m14 7 3 3"/></Icon>;
export const TrashIcon = () => <Icon><path d="M4 7h16M9 7V4h6v3M18 7l-1 13H7L6 7M10 11v5M14 11v5"/></Icon>;
export const PlayIcon = () => <Icon><path d="m8 5 11 7-11 7V5Z"/></Icon>;
export const WalletIcon = () => <Icon><path d="M4 6h14a2 2 0 0 1 2 2v10H5a2 2 0 0 1-2-2V6.5A2.5 2.5 0 0 1 5.5 4H17"/><path d="M16 11h4v4h-4a2 2 0 0 1 0-4Z"/></Icon>;
export const SearchIcon = () => <Icon><circle cx="11" cy="11" r="7"/><path d="m20 20-4-4"/></Icon>;
export const DownloadIcon = () => <Icon><path d="M12 3v12M7 10l5 5 5-5M5 21h14"/></Icon>;
export const LockIcon = () => <Icon><rect x="4" y="10" width="16" height="11" rx="2"/><path d="M8 10V7a4 4 0 0 1 8 0v3"/></Icon>;
export const XIcon = () => <Icon><path d="m6 6 12 12M18 6 6 18"/></Icon>;
export const EyeIcon = () => <Icon><path d="M2 12s3.5-6 10-6 10 6 10 6-3.5 6-10 6S2 12 2 12Z"/><circle cx="12" cy="12" r="2.5"/></Icon>;
export const CheckIcon = () => <Icon><path d="m5 12 4 4L19 6"/></Icon>;
export const AlertIcon = () => <Icon><path d="M12 3 2.5 20h19L12 3Z"/><path d="M12 9v4M12 17h.01"/></Icon>;
