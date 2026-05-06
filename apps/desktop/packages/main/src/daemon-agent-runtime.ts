import type {
  AgentRuntimeSnapshot,
  AuthProfileLoginResult,
  AuthProfileLogoutResult,
  DaemonAgentRuntimeAuthLoginParams,
  DaemonAgentRuntimeAuthLogoutParams,
  DaemonAgentRuntimePatchProfileParams,
  DaemonAgentRuntimeSelectProfileParams,
  DaemonAgentRuntimeSetExtensionEnabledParams,
  DaemonDiagnostics,
  DesktopInvokeHandlers,
} from "@taugentic/desktop-shared";
import {
  parseDaemonAgentRuntimeAuthLoginParams,
  parseDaemonAgentRuntimeAuthLogoutParams,
  parseDaemonAgentRuntimePatchProfileParams,
  parseDaemonAgentRuntimeSelectProfileParams,
  parseDaemonAgentRuntimeSetExtensionEnabledParams,
} from "@taugentic/desktop-shared/validation";

import { DaemonSessionRequestClient } from "./daemon-session-request-client.js";
import {
  DAEMON_REQUEST_TIMEOUT_AGENT_RUNTIME,
  DAEMON_REQUEST_TIMEOUT_DISABLED,
  DAEMON_REQUEST_TIMEOUT_INTERACTIVE_AUTH,
} from "./daemon-rpc-connection.js";

const snapshotDaemonRuntime = new DaemonSessionRequestClient(null, {
  requestTimeout: DAEMON_REQUEST_TIMEOUT_AGENT_RUNTIME,
});
const interactiveAuthLoginDaemonRuntime = new DaemonSessionRequestClient(null, {
  // Browser OAuth is human-paced. A fixed deadline turns valid logins into false failures.
  requestTimeout: DAEMON_REQUEST_TIMEOUT_DISABLED,
});
const interactiveAuthLogoutDaemonRuntime = new DaemonSessionRequestClient(null, {
  requestTimeout: DAEMON_REQUEST_TIMEOUT_INTERACTIVE_AUTH,
});

async function getAgentRuntime(): Promise<AgentRuntimeSnapshot> {
  return snapshotDaemonRuntime.getAgentRuntime();
}

async function getDaemonDiagnostics(): Promise<DaemonDiagnostics> {
  return snapshotDaemonRuntime.getDaemonDiagnostics();
}

async function selectAgentRuntimeProfile(
  params: DaemonAgentRuntimeSelectProfileParams,
): Promise<AgentRuntimeSnapshot> {
  return snapshotDaemonRuntime.selectAgentRuntimeProfile(
    parseDaemonAgentRuntimeSelectProfileParams(params),
  );
}

async function patchAgentRuntimeProfile(
  params: DaemonAgentRuntimePatchProfileParams,
): Promise<AgentRuntimeSnapshot> {
  return snapshotDaemonRuntime.patchAgentRuntimeProfile(
    parseDaemonAgentRuntimePatchProfileParams(params),
  );
}

async function loginAgentRuntimeAuthProfile(
  params: DaemonAgentRuntimeAuthLoginParams,
): Promise<AuthProfileLoginResult> {
  return interactiveAuthLoginDaemonRuntime.loginAgentRuntimeAuthProfile(
    parseDaemonAgentRuntimeAuthLoginParams(params),
  );
}

async function logoutAgentRuntimeAuthProfile(
  params: DaemonAgentRuntimeAuthLogoutParams,
): Promise<AuthProfileLogoutResult> {
  return interactiveAuthLogoutDaemonRuntime.logoutAgentRuntimeAuthProfile(
    parseDaemonAgentRuntimeAuthLogoutParams(params),
  );
}

async function setAgentRuntimeExtensionEnabled(
  params: DaemonAgentRuntimeSetExtensionEnabledParams,
): Promise<AgentRuntimeSnapshot> {
  return snapshotDaemonRuntime.setAgentRuntimeExtensionEnabled(
    parseDaemonAgentRuntimeSetExtensionEnabledParams(params),
  );
}

export const desktopAgentRuntimeInvokeHandlers: Pick<
  DesktopInvokeHandlers,
  | "getDaemonDiagnostics"
  | "getAgentRuntime"
  | "selectAgentRuntimeProfile"
  | "patchAgentRuntimeProfile"
  | "loginAgentRuntimeAuthProfile"
  | "logoutAgentRuntimeAuthProfile"
  | "setAgentRuntimeExtensionEnabled"
> = {
  getDaemonDiagnostics,
  getAgentRuntime,
  selectAgentRuntimeProfile,
  patchAgentRuntimeProfile,
  loginAgentRuntimeAuthProfile,
  logoutAgentRuntimeAuthProfile,
  setAgentRuntimeExtensionEnabled,
};
