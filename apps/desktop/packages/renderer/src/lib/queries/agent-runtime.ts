import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";

import type {
  AgentRuntimeSnapshot,
  AuthProfileLoginResult,
  AuthProfileLogoutResult,
  DaemonAgentRuntimeAuthLoginParams,
  DaemonAgentRuntimeAuthLogoutParams,
  DaemonAgentRuntimePatchProfileParams,
  DaemonAgentRuntimeSelectProfileParams,
  DaemonAgentRuntimeSetExtensionEnabledParams,
  DaemonAgentRuntimeTestLocalEndpointParams,
  LocalModelEndpointTestResult,
} from "@taugentic/desktop-shared";

import {
  getAgentRuntime,
  loginAgentRuntimeAuthProfile,
  logoutAgentRuntimeAuthProfile,
  patchAgentRuntimeProfile,
  selectAgentRuntimeProfile,
  setAgentRuntimeExtensionEnabled,
  testLocalModelEndpoint,
} from "@/lib/ipc/api";

import { agentRuntimeRootKey, queryKeys } from "./keys";
import { SESSION_QUERY_POLL_INTERVAL_MS, type SessionQueryView } from "./session-queries";

export function useAgentRuntimeQuery(): SessionQueryView<AgentRuntimeSnapshot> {
  const query = useQuery({
    queryKey: queryKeys.agentRuntime.snapshot,
    queryFn: () => getAgentRuntime(),
    refetchInterval: SESSION_QUERY_POLL_INTERVAL_MS,
  });
  return {
    data: query.data,
    error: query.error,
    isLoading: query.isLoading,
    isFetching: query.isFetching,
    refetch: query.refetch,
  };
}

export function useSelectAgentRuntimeProfileMutation() {
  const qc = useQueryClient();
  return useMutation<AgentRuntimeSnapshot, Error, DaemonAgentRuntimeSelectProfileParams>({
    mutationFn: (params) => selectAgentRuntimeProfile(params),
    onSuccess: (snapshot) => {
      qc.setQueryData(queryKeys.agentRuntime.snapshot, snapshot);
      void qc.invalidateQueries({ queryKey: agentRuntimeRootKey });
    },
  });
}

export function usePatchAgentRuntimeProfileMutation() {
  const qc = useQueryClient();
  return useMutation<AgentRuntimeSnapshot, Error, DaemonAgentRuntimePatchProfileParams>({
    mutationFn: (params) => patchAgentRuntimeProfile(params),
    onSuccess: (snapshot) => {
      qc.setQueryData(queryKeys.agentRuntime.snapshot, snapshot);
      void qc.invalidateQueries({ queryKey: agentRuntimeRootKey });
    },
  });
}

export function useLoginAgentRuntimeAuthProfileMutation() {
  const qc = useQueryClient();
  return useMutation<AuthProfileLoginResult, Error, DaemonAgentRuntimeAuthLoginParams>({
    mutationFn: (params) => loginAgentRuntimeAuthProfile(params),
    onSuccess: () => {
      void qc.invalidateQueries({ queryKey: agentRuntimeRootKey });
    },
  });
}

export function useLogoutAgentRuntimeAuthProfileMutation() {
  const qc = useQueryClient();
  return useMutation<AuthProfileLogoutResult, Error, DaemonAgentRuntimeAuthLogoutParams>({
    mutationFn: (params) => logoutAgentRuntimeAuthProfile(params),
    onSuccess: () => {
      void qc.invalidateQueries({ queryKey: agentRuntimeRootKey });
    },
  });
}

export function useSetAgentRuntimeExtensionEnabledMutation() {
  const qc = useQueryClient();
  return useMutation<AgentRuntimeSnapshot, Error, DaemonAgentRuntimeSetExtensionEnabledParams>({
    mutationFn: (params) => setAgentRuntimeExtensionEnabled(params),
    onSuccess: (snapshot) => {
      qc.setQueryData(queryKeys.agentRuntime.snapshot, snapshot);
      void qc.invalidateQueries({ queryKey: agentRuntimeRootKey });
    },
  });
}

export function useTestLocalModelEndpointMutation() {
  return useMutation<
    LocalModelEndpointTestResult,
    Error,
    DaemonAgentRuntimeTestLocalEndpointParams
  >({
    mutationFn: (params) => testLocalModelEndpoint(params),
  });
}
