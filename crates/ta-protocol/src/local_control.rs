#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeControlBootstrapCommand {
    Start,
    Snapshot,
    Reconcile,
    ResetLocal,
    EnableBackground,
    DisableBackground,
    Stop,
}

impl RuntimeControlBootstrapCommand {
    pub const SUBCOMMAND: &str = "__runtime-control-bootstrap";

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Start => "start",
            Self::Snapshot => "snapshot",
            Self::Reconcile => "reconcile",
            Self::ResetLocal => "reset-local",
            Self::EnableBackground => "enable-background",
            Self::DisableBackground => "disable-background",
            Self::Stop => "stop",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "start" => Some(Self::Start),
            "snapshot" => Some(Self::Snapshot),
            "reconcile" => Some(Self::Reconcile),
            "reset-local" => Some(Self::ResetLocal),
            "enable-background" => Some(Self::EnableBackground),
            "disable-background" => Some(Self::DisableBackground),
            "stop" => Some(Self::Stop),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeControlHandoffCommand {
    EnableBackground,
    DisableBackground,
    StopLocalRuntime,
    StopBackgroundRuntime,
}

impl RuntimeControlHandoffCommand {
    pub const SUBCOMMAND: &str = "__runtime-control-handoff";

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::EnableBackground => "enable-background",
            Self::DisableBackground => "disable-background",
            Self::StopLocalRuntime => "stop-local-runtime",
            Self::StopBackgroundRuntime => "stop-background-runtime",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "enable-background" => Some(Self::EnableBackground),
            "disable-background" => Some(Self::DisableBackground),
            "stop-local-runtime" => Some(Self::StopLocalRuntime),
            "stop-background-runtime" => Some(Self::StopBackgroundRuntime),
            _ => None,
        }
    }
}
