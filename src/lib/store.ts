import { create } from "zustand";
import * as api from "./tauri";
import type { ClaudeSettings, McpServerConfig, SessionInfo, Run, Profile, Schedule, DiscoveredPlist } from "./types";

// --- Settings Store ---
interface SettingsState {
  settings: ClaudeSettings | null;
  scope: "global" | "project";
  projectDir: string | null;
  loading: boolean;
  error: string | null;
}

interface SettingsActions {
  fetchSettings: () => Promise<void>;
  setScope: (scope: "global" | "project", projectDir?: string) => void;
  addPermission: (list: "allow" | "deny", pattern: string) => Promise<void>;
  removePermission: (list: "allow" | "deny", pattern: string) => Promise<void>;
  setBypassPermissions: (bypass: boolean) => Promise<void>;
  addMcpServer: (name: string, config: McpServerConfig) => Promise<void>;
  removeMcpServer: (name: string) => Promise<void>;
  setEnvVar: (key: string, value: string) => Promise<void>;
  removeEnvVar: (key: string) => Promise<void>;
}

export const useSettingsStore = create<SettingsState & SettingsActions>()((set, get) => ({
  settings: null,
  scope: "global",
  projectDir: null,
  loading: false,
  error: null,

  fetchSettings: async () => {
    const { scope, projectDir } = get();
    set({ loading: true, error: null });
    try {
      const settings = await api.getClaudeSettings(scope, projectDir ?? undefined);
      set({ settings, loading: false });
    } catch (err) {
      set({ error: String(err), loading: false });
    }
  },

  setScope: (scope, projectDir) => {
    set({ scope, projectDir: projectDir ?? null });
  },

  addPermission: async (list, pattern) => {
    const { scope, projectDir } = get();
    await api.addPermission(scope, list, pattern, projectDir ?? undefined);
    await get().fetchSettings();
  },

  removePermission: async (list, pattern) => {
    const { scope, projectDir } = get();
    await api.removePermission(scope, list, pattern, projectDir ?? undefined);
    await get().fetchSettings();
  },

  setBypassPermissions: async (bypass) => {
    const { scope, projectDir } = get();
    await api.setBypassPermissions(scope, bypass, projectDir ?? undefined);
    await get().fetchSettings();
  },

  addMcpServer: async (name, config) => {
    const { scope, projectDir } = get();
    await api.addMcpServer(scope, name, config, projectDir ?? undefined);
    await get().fetchSettings();
  },

  removeMcpServer: async (name) => {
    const { scope, projectDir } = get();
    await api.removeMcpServer(scope, name, projectDir ?? undefined);
    await get().fetchSettings();
  },

  setEnvVar: async (key, value) => {
    const { scope, projectDir } = get();
    await api.setEnvVar(scope, key, value, projectDir ?? undefined);
    await get().fetchSettings();
  },

  removeEnvVar: async (key) => {
    const { scope, projectDir } = get();
    await api.removeEnvVar(scope, key, projectDir ?? undefined);
    await get().fetchSettings();
  },
}));

// --- Sessions Store ---
interface SessionsState {
  liveSessions: SessionInfo[];
  runs: Run[];
  runsLoading: boolean;
}

interface SessionsActions {
  fetchLiveSessions: () => Promise<void>;
  fetchRuns: (limit?: number, offset?: number) => Promise<void>;
  spawnSession: (prompt: string, directory: string, profile?: string, agent?: string) => Promise<SessionInfo>;
  killSession: (sessionId: string) => Promise<void>;
}

export const useSessionsStore = create<SessionsState & SessionsActions>()((set) => ({
  liveSessions: [],
  runs: [],
  runsLoading: false,

  fetchLiveSessions: async () => {
    const liveSessions = await api.listSessions();
    set({ liveSessions });
  },

  fetchRuns: async (limit, offset) => {
    set({ runsLoading: true });
    try {
      const runs = await api.listRuns(limit, offset);
      set({ runs, runsLoading: false });
    } catch {
      set({ runsLoading: false });
    }
  },

  spawnSession: async (prompt, directory, profile, agent) => {
    const session = await api.spawnSession(prompt, directory, profile, agent);
    set((state) => ({ liveSessions: [...state.liveSessions, session] }));
    return session;
  },

  killSession: async (sessionId) => {
    await api.killSession(sessionId);
    set((state) => ({
      liveSessions: state.liveSessions.filter((s) => s.id !== sessionId),
    }));
  },
}));

// --- Profiles Store ---
interface ProfilesState {
  profiles: Profile[];
  loading: boolean;
}

interface ProfilesActions {
  fetchProfiles: () => Promise<void>;
  switchProfile: (name: string) => Promise<void>;
  importProfile: (name: string) => Promise<void>;
}

export const useProfilesStore = create<ProfilesState & ProfilesActions>()((set, get) => ({
  profiles: [],
  loading: false,

  fetchProfiles: async () => {
    set({ loading: true });
    try {
      const profiles = await api.listProfiles();
      set({ profiles, loading: false });
    } catch {
      set({ loading: false });
    }
  },

  switchProfile: async (name) => {
    await api.switchProfile(name);
    await get().fetchProfiles();
  },

  importProfile: async (name) => {
    await api.importProfile(name);
    await get().fetchProfiles();
  },
}));

// --- Scheduler Store ---
interface SchedulerState {
  schedules: Schedule[];
  discoveredJobs: DiscoveredPlist[];
  loading: boolean;
  selectedScheduleId: string | null;
  selectedScheduleRuns: Run[];
}

interface SchedulerActions {
  fetchSchedules: () => Promise<void>;
  createSchedule: (schedule: Schedule) => Promise<void>;
  updateSchedule: (schedule: Schedule) => Promise<void>;
  deleteSchedule: (id: string) => Promise<void>;
  toggleSchedule: (id: string, enabled: boolean) => Promise<void>;
  discoverJobs: () => Promise<void>;
  importJob: (label: string) => Promise<Schedule>;
  selectSchedule: (id: string | null) => void;
  fetchScheduleRuns: (scheduleId: string, limit?: number, offset?: number) => Promise<void>;
}

export const useSchedulerStore = create<SchedulerState & SchedulerActions>()((set, get) => ({
  schedules: [],
  discoveredJobs: [],
  loading: false,
  selectedScheduleId: null,
  selectedScheduleRuns: [],

  fetchSchedules: async () => {
    set({ loading: true });
    try {
      const schedules = await api.listSchedules();
      set({ schedules, loading: false });
    } catch {
      set({ loading: false });
    }
  },

  createSchedule: async (schedule) => {
    await api.createSchedule(schedule);
    await get().fetchSchedules();
  },

  updateSchedule: async (schedule) => {
    await api.updateSchedule(schedule);
    await get().fetchSchedules();
  },

  deleteSchedule: async (id) => {
    await api.deleteSchedule(id);
    set((state) => ({
      schedules: state.schedules.filter((s) => s.id !== id),
      selectedScheduleId: state.selectedScheduleId === id ? null : state.selectedScheduleId,
    }));
  },

  toggleSchedule: async (id, enabled) => {
    if (enabled) {
      await api.resumeSchedule(id);
    } else {
      await api.pauseSchedule(id);
    }
    set((state) => ({
      schedules: state.schedules.map((s) =>
        s.id === id ? { ...s, enabled } : s
      ),
    }));
  },

  discoverJobs: async () => {
    const discoveredJobs = await api.discoverLaunchdJobs();
    set({ discoveredJobs });
  },

  importJob: async (label) => {
    const schedule = await api.importLaunchdJob(label);
    set((state) => ({ schedules: [...state.schedules, schedule] }));
    return schedule;
  },

  selectSchedule: (id) => {
    set({ selectedScheduleId: id, selectedScheduleRuns: [] });
  },

  fetchScheduleRuns: async (scheduleId, limit, offset) => {
    const runs = await api.listScheduleRuns(scheduleId, limit, offset);
    set({ selectedScheduleRuns: runs });
  },
}));
