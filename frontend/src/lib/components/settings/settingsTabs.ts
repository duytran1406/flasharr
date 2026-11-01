export type SettingsTabId =
  | "general"
  | "accounts"
  | "download"
  | "integrations"
  | "sources"
  | "logs";

export interface SettingsTab {
  id: SettingsTabId;
  label: string;
  icon: string;
}

export const SETTINGS_TABS: readonly SettingsTab[] = [
  { id: "general", label: "General", icon: "tune" },
  { id: "accounts", label: "Accounts", icon: "key" },
  { id: "download", label: "Download Config", icon: "download" },
  { id: "integrations", label: "Integrations", icon: "hub" },
  { id: "sources", label: "Sources", icon: "travel_explore" },
  { id: "logs", label: "Logs", icon: "terminal" },
];

/** Roving-tabindex keyboard navigation: ArrowLeft/ArrowRight wrap around, Home/End jump to the ends. */
export function nextSettingsTab(current: SettingsTabId, key: string): SettingsTabId {
  if (key === "Home") return SETTINGS_TABS[0].id;
  if (key === "End") return SETTINGS_TABS.at(-1)!.id;
  const index = SETTINGS_TABS.findIndex((tab) => tab.id === current);
  const step = key === "ArrowLeft" ? -1 : key === "ArrowRight" ? 1 : 0;
  return SETTINGS_TABS[(index + step + SETTINGS_TABS.length) % SETTINGS_TABS.length].id;
}
