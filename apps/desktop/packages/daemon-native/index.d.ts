/**
 * Rust-owned daemon boundary. Credentials, authorities, socket paths, and
 * cursors never cross this API; method results are public protocol JSON.
 */
export declare class NativeDaemonBridge {
  constructor();
  start(): Promise<string>;
  listSessions(): Promise<string>;
  openSession(paramsJson: string): Promise<string>;
  attachSession(sessionId: string): Promise<string>;
  navigationSnapshot(search?: string | null): Promise<string>;
  navigationIntent(intentJson: string): Promise<string>;
  openProject(path: string, trustAcknowledged: boolean): Promise<string>;
  getAgentRuntime(): Promise<string>;
  loginAuthProfile(paramsJson: string): Promise<string>;
  completeAuthProfileLogin(paramsJson: string): Promise<string>;
  logoutAuthProfile(paramsJson: string): Promise<string>;
  listApprovals(queryJson: string): Promise<string>;
  decideApproval(paramsJson: string): Promise<string>;
  startRun(commandJson: string): Promise<string>;
  cancelRun(runId: string): Promise<string>;
  releaseRunEventSubscription(): string;
  subscribeRunEvents(sessionId: string, runId: string, callback: (eventJson: string) => void): Promise<string>;
  subscribeLifecycle(callback: (projectionJson: string) => void): Promise<string>;
  close(): Promise<string>;
}
