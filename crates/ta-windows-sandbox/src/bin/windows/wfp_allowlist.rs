use std::{
    mem,
    net::{IpAddr, Ipv4Addr, Ipv6Addr},
    ptr::{null, null_mut},
};

use ta_sandbox::NetworkPolicy;
use windows_sys::{
    Win32::{
        Foundation::{HANDLE, PSID},
        NetworkManagement::WindowsFilteringPlatform::{
            FWP_ACTION_BLOCK, FWP_ACTION_PERMIT, FWP_BYTE_BLOB, FWP_CONDITION_VALUE0,
            FWP_CONDITION_VALUE0_0, FWP_EMPTY, FWP_MATCH_EQUAL, FWP_SID, FWP_UINT8, FWP_UINT16,
            FWP_V4_ADDR_AND_MASK, FWP_V4_ADDR_MASK, FWP_V6_ADDR_AND_MASK, FWP_V6_ADDR_MASK,
            FWP_VALUE0, FWP_VALUE0_0, FWPM_ACTION0, FWPM_ACTION0_0, FWPM_CONDITION_ALE_PACKAGE_ID,
            FWPM_CONDITION_IP_PROTOCOL, FWPM_CONDITION_IP_REMOTE_ADDRESS,
            FWPM_CONDITION_IP_REMOTE_PORT, FWPM_DISPLAY_DATA0, FWPM_FILTER_CONDITION0,
            FWPM_FILTER0, FWPM_FILTER0_0, FWPM_LAYER_ALE_AUTH_CONNECT_V4,
            FWPM_LAYER_ALE_AUTH_CONNECT_V6, FWPM_SESSION_FLAG_DYNAMIC, FWPM_SESSION0,
            FWPM_SUBLAYER_UNIVERSAL, FwpmEngineClose0, FwpmEngineOpen0, FwpmFilterAdd0,
            FwpmTransactionAbort0, FwpmTransactionBegin0, FwpmTransactionCommit0,
        },
        Networking::WinSock::IPPROTO_TCP,
        Security::SID,
        System::{Rpc::RPC_C_AUTHN_DEFAULT, Threading::INFINITE},
    },
    core::GUID,
};

use super::{
    HelperError,
    util::{to_wide, win32_error},
};

const SESSION_NAME: &str = "Taugentic Windows Sandbox WFP";
const PERMIT_WEIGHT: u8 = 0xf0;
const BLOCK_WEIGHT: u8 = 0x10;
const DOMAIN_ALLOWLIST_UNSUPPORTED: &str = "Windows WFP allowlist supports TCP ports and IP/CIDR entries; domain names require a managed resolver";

pub struct WfpAllowlist {
    _engine: WfpEngine,
}

impl WfpAllowlist {
    pub fn apply(policy: &NetworkPolicy, appcontainer_sid: PSID) -> Result<Self, HelperError> {
        let NetworkPolicy::Allowlist(entries) = policy else {
            return Err(unsupported(
                "Windows WFP allowlist requires NetworkPolicy::Allowlist",
            ));
        };
        let rules = parse_allowlist(entries)?;
        let engine = WfpEngine::open()?;
        {
            let mut transaction = engine.begin_transaction()?;
            for rule in &rules {
                engine.add_permit_filters(rule, appcontainer_sid)?;
            }
            engine.add_block_filter(AddressFamily::V4, appcontainer_sid)?;
            engine.add_block_filter(AddressFamily::V6, appcontainer_sid)?;
            transaction.commit()?;
        }
        Ok(Self { _engine: engine })
    }
}

struct WfpEngine {
    handle: HANDLE,
}

impl WfpEngine {
    fn open() -> Result<Self, HelperError> {
        let session_name = to_wide(SESSION_NAME);
        let mut session: FWPM_SESSION0 = unsafe { mem::zeroed() };
        session.displayData = FWPM_DISPLAY_DATA0 {
            name: session_name.as_ptr() as *mut _,
            description: null_mut(),
        };
        session.flags = FWPM_SESSION_FLAG_DYNAMIC;
        session.txnWaitTimeoutInMSec = INFINITE;

        let mut handle = 0;
        win32_error(
            unsafe {
                FwpmEngineOpen0(
                    null(),
                    RPC_C_AUTHN_DEFAULT as u32,
                    null(),
                    &session,
                    &mut handle,
                )
            },
            "FwpmEngineOpen0",
        )?;
        Ok(Self { handle })
    }

    fn begin_transaction(&self) -> Result<WfpTransaction<'_>, HelperError> {
        win32_error(
            unsafe { FwpmTransactionBegin0(self.handle, 0) },
            "FwpmTransactionBegin0",
        )?;
        Ok(WfpTransaction {
            engine: self,
            committed: false,
        })
    }

    fn add_permit_filters(&self, rule: &AllowRule, sid: PSID) -> Result<(), HelperError> {
        match rule.family() {
            RuleFamily::V4 => self.add_filter(
                AddressFamily::V4,
                FWP_ACTION_PERMIT,
                PERMIT_WEIGHT,
                Some(rule),
                sid,
            ),
            RuleFamily::V6 => self.add_filter(
                AddressFamily::V6,
                FWP_ACTION_PERMIT,
                PERMIT_WEIGHT,
                Some(rule),
                sid,
            ),
            RuleFamily::Any => {
                self.add_filter(
                    AddressFamily::V4,
                    FWP_ACTION_PERMIT,
                    PERMIT_WEIGHT,
                    Some(rule),
                    sid,
                )?;
                self.add_filter(
                    AddressFamily::V6,
                    FWP_ACTION_PERMIT,
                    PERMIT_WEIGHT,
                    Some(rule),
                    sid,
                )
            }
        }
    }

    fn add_block_filter(&self, family: AddressFamily, sid: PSID) -> Result<(), HelperError> {
        self.add_filter(family, FWP_ACTION_BLOCK, BLOCK_WEIGHT, None, sid)
    }

    fn add_filter(
        &self,
        family: AddressFamily,
        action: u32,
        weight: u8,
        rule: Option<&AllowRule>,
        sid: PSID,
    ) -> Result<(), HelperError> {
        let name = to_wide(match action {
            FWP_ACTION_PERMIT => "Taugentic sandbox allowlist permit",
            _ => "Taugentic sandbox allowlist default deny",
        });
        let mut conditions = vec![package_condition(sid)];
        let mut v4_masks = Vec::new();
        let mut v6_masks = Vec::new();

        if let Some(rule) = rule {
            conditions.push(protocol_tcp_condition());
            if let Some(port) = rule.port {
                conditions.push(port_condition(port));
            }
            match rule.cidr {
                Some(IpCidr::V4 { addr, prefix }) if family == AddressFamily::V4 => {
                    v4_masks.push(Box::new(FWP_V4_ADDR_AND_MASK {
                        addr: u32::from(addr),
                        mask: ipv4_prefix_mask(prefix),
                    }));
                    let mask = v4_masks.last_mut().expect("v4 mask");
                    conditions.push(ipv4_condition(mask));
                }
                Some(IpCidr::V6 { addr, prefix }) if family == AddressFamily::V6 => {
                    v6_masks.push(Box::new(FWP_V6_ADDR_AND_MASK {
                        addr: addr.octets(),
                        prefixLength: prefix,
                    }));
                    let mask = v6_masks.last_mut().expect("v6 mask");
                    conditions.push(ipv6_condition(mask));
                }
                Some(_) | None => {}
            }
        }

        let filter = FWPM_FILTER0 {
            filterKey: GUID::from_u128(0),
            displayData: FWPM_DISPLAY_DATA0 {
                name: name.as_ptr() as *mut _,
                description: null_mut(),
            },
            flags: 0,
            providerKey: null_mut(),
            providerData: empty_blob(),
            layerKey: family.layer(),
            subLayerKey: FWPM_SUBLAYER_UNIVERSAL,
            weight: weight_value(weight),
            numFilterConditions: conditions.len() as u32,
            filterCondition: conditions.as_mut_ptr(),
            action: FWPM_ACTION0 {
                r#type: action,
                Anonymous: FWPM_ACTION0_0 {
                    filterType: GUID::from_u128(0),
                },
            },
            Anonymous: FWPM_FILTER0_0 { rawContext: 0 },
            reserved: null_mut(),
            filterId: 0,
            effectiveWeight: empty_value(),
        };
        let mut filter_id = 0;
        win32_error(
            unsafe { FwpmFilterAdd0(self.handle, &filter, null_mut(), &mut filter_id) },
            "FwpmFilterAdd0",
        )
    }
}

impl Drop for WfpEngine {
    fn drop(&mut self) {
        if self.handle != 0 {
            unsafe {
                FwpmEngineClose0(self.handle);
            }
        }
    }
}

struct WfpTransaction<'engine> {
    engine: &'engine WfpEngine,
    committed: bool,
}

impl WfpTransaction<'_> {
    fn commit(&mut self) -> Result<(), HelperError> {
        win32_error(
            unsafe { FwpmTransactionCommit0(self.engine.handle) },
            "FwpmTransactionCommit0",
        )?;
        self.committed = true;
        Ok(())
    }
}

impl Drop for WfpTransaction<'_> {
    fn drop(&mut self) {
        if !self.committed {
            unsafe {
                FwpmTransactionAbort0(self.engine.handle);
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AddressFamily {
    V4,
    V6,
}

impl AddressFamily {
    fn layer(self) -> GUID {
        match self {
            Self::V4 => FWPM_LAYER_ALE_AUTH_CONNECT_V4,
            Self::V6 => FWPM_LAYER_ALE_AUTH_CONNECT_V6,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RuleFamily {
    V4,
    V6,
    Any,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct AllowRule {
    cidr: Option<IpCidr>,
    port: Option<u16>,
}

impl AllowRule {
    fn family(&self) -> RuleFamily {
        match self.cidr {
            Some(IpCidr::V4 { .. }) => RuleFamily::V4,
            Some(IpCidr::V6 { .. }) => RuleFamily::V6,
            None => RuleFamily::Any,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum IpCidr {
    V4 { addr: Ipv4Addr, prefix: u8 },
    V6 { addr: Ipv6Addr, prefix: u8 },
}

fn parse_allowlist(entries: &[String]) -> Result<Vec<AllowRule>, HelperError> {
    if entries.is_empty() {
        return Err(unsupported(
            "Windows WFP allowlist requires at least one destination",
        ));
    }
    entries
        .iter()
        .map(|entry| parse_allow_rule(entry))
        .collect::<Result<Vec<_>, _>>()
}

fn parse_allow_rule(entry: &str) -> Result<AllowRule, HelperError> {
    let entry = entry.trim();
    let entry = entry.strip_prefix("tcp:").unwrap_or(entry);
    if let Some(port) = parse_port(entry) {
        return Ok(AllowRule {
            cidr: None,
            port: Some(port),
        });
    }

    let (address, port) = split_address_port(entry)?;
    let cidr = parse_cidr(address)?;
    Ok(AllowRule {
        cidr: Some(cidr),
        port,
    })
}

fn split_address_port(entry: &str) -> Result<(&str, Option<u16>), HelperError> {
    if let Some(rest) = entry.strip_prefix('[') {
        let Some((address, port)) = rest.split_once("]:") else {
            return Ok((entry, None));
        };
        return parse_required_port(port).map(|port| (address, Some(port)));
    }

    if let Some((address, port)) = entry.rsplit_once(':')
        && !address.contains(':')
    {
        return parse_required_port(port).map(|port| (address, Some(port)));
    }
    Ok((entry, None))
}

fn parse_cidr(value: &str) -> Result<IpCidr, HelperError> {
    let (address, prefix) = match value.split_once('/') {
        Some((address, prefix)) => (address, Some(prefix)),
        None => (value, None),
    };
    let address = address
        .parse::<IpAddr>()
        .map_err(|_| unsupported(DOMAIN_ALLOWLIST_UNSUPPORTED))?;
    match address {
        IpAddr::V4(addr) => {
            let prefix = parse_prefix(prefix, 32)?;
            Ok(IpCidr::V4 { addr, prefix })
        }
        IpAddr::V6(addr) => {
            let prefix = parse_prefix(prefix, 128)?;
            Ok(IpCidr::V6 { addr, prefix })
        }
    }
}

fn parse_prefix(prefix: Option<&str>, max: u8) -> Result<u8, HelperError> {
    let Some(prefix) = prefix else {
        return Ok(max);
    };
    let prefix = prefix
        .parse::<u8>()
        .map_err(|_| unsupported(DOMAIN_ALLOWLIST_UNSUPPORTED))?;
    if prefix > max {
        return Err(unsupported(DOMAIN_ALLOWLIST_UNSUPPORTED));
    }
    Ok(prefix)
}

fn parse_required_port(port: &str) -> Result<u16, HelperError> {
    parse_port(port).ok_or_else(|| unsupported(DOMAIN_ALLOWLIST_UNSUPPORTED))
}

fn unsupported(message: &str) -> HelperError {
    HelperError::UnsupportedProfile(message.to_string())
}

fn parse_port(value: &str) -> Option<u16> {
    let port = value.parse::<u16>().ok()?;
    (port != 0).then_some(port)
}

fn package_condition(sid: PSID) -> FWPM_FILTER_CONDITION0 {
    FWPM_FILTER_CONDITION0 {
        fieldKey: FWPM_CONDITION_ALE_PACKAGE_ID,
        matchType: FWP_MATCH_EQUAL,
        conditionValue: FWP_CONDITION_VALUE0 {
            r#type: FWP_SID,
            Anonymous: FWP_CONDITION_VALUE0_0 {
                sid: sid.cast::<SID>(),
            },
        },
    }
}

fn protocol_tcp_condition() -> FWPM_FILTER_CONDITION0 {
    FWPM_FILTER_CONDITION0 {
        fieldKey: FWPM_CONDITION_IP_PROTOCOL,
        matchType: FWP_MATCH_EQUAL,
        conditionValue: FWP_CONDITION_VALUE0 {
            r#type: FWP_UINT8,
            Anonymous: FWP_CONDITION_VALUE0_0 {
                uint8: IPPROTO_TCP as u8,
            },
        },
    }
}

fn port_condition(port: u16) -> FWPM_FILTER_CONDITION0 {
    FWPM_FILTER_CONDITION0 {
        fieldKey: FWPM_CONDITION_IP_REMOTE_PORT,
        matchType: FWP_MATCH_EQUAL,
        conditionValue: FWP_CONDITION_VALUE0 {
            r#type: FWP_UINT16,
            Anonymous: FWP_CONDITION_VALUE0_0 { uint16: port },
        },
    }
}

fn ipv4_condition(mask: &mut FWP_V4_ADDR_AND_MASK) -> FWPM_FILTER_CONDITION0 {
    FWPM_FILTER_CONDITION0 {
        fieldKey: FWPM_CONDITION_IP_REMOTE_ADDRESS,
        matchType: FWP_MATCH_EQUAL,
        conditionValue: FWP_CONDITION_VALUE0 {
            r#type: FWP_V4_ADDR_MASK,
            Anonymous: FWP_CONDITION_VALUE0_0 { v4AddrMask: mask },
        },
    }
}

fn ipv6_condition(mask: &mut FWP_V6_ADDR_AND_MASK) -> FWPM_FILTER_CONDITION0 {
    FWPM_FILTER_CONDITION0 {
        fieldKey: FWPM_CONDITION_IP_REMOTE_ADDRESS,
        matchType: FWP_MATCH_EQUAL,
        conditionValue: FWP_CONDITION_VALUE0 {
            r#type: FWP_V6_ADDR_MASK,
            Anonymous: FWP_CONDITION_VALUE0_0 { v6AddrMask: mask },
        },
    }
}

fn ipv4_prefix_mask(prefix: u8) -> u32 {
    if prefix == 0 {
        0
    } else {
        u32::MAX << (32 - prefix)
    }
}

fn empty_blob() -> FWP_BYTE_BLOB {
    FWP_BYTE_BLOB {
        size: 0,
        data: null_mut(),
    }
}

fn empty_value() -> FWP_VALUE0 {
    FWP_VALUE0 {
        r#type: FWP_EMPTY,
        Anonymous: unsafe { mem::zeroed() },
    }
}

fn weight_value(weight: u8) -> FWP_VALUE0 {
    FWP_VALUE0 {
        r#type: FWP_UINT8,
        Anonymous: FWP_VALUE0_0 { uint8: weight },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_tcp_port_entries() {
        assert_eq!(
            parse_allow_rule("tcp:443").expect("rule"),
            AllowRule {
                cidr: None,
                port: Some(443),
            }
        );
        assert_eq!(
            parse_allow_rule("22").expect("rule"),
            AllowRule {
                cidr: None,
                port: Some(22),
            }
        );
    }

    #[test]
    fn parses_ip_cidr_and_port_entries() {
        assert_eq!(
            parse_allow_rule("203.0.113.0/24:443").expect("rule"),
            AllowRule {
                cidr: Some(IpCidr::V4 {
                    addr: Ipv4Addr::new(203, 0, 113, 0),
                    prefix: 24,
                }),
                port: Some(443),
            }
        );
        assert_eq!(
            parse_allow_rule("[2001:db8::]/32:443").expect("rule"),
            AllowRule {
                cidr: Some(IpCidr::V6 {
                    addr: "2001:db8::".parse().expect("ipv6"),
                    prefix: 32,
                }),
                port: Some(443),
            }
        );
    }

    #[test]
    fn rejects_domain_name_entries() {
        let error = parse_allow_rule("example.com").expect_err("domain rejected");

        assert!(error.to_string().contains("domain names"));
    }

    #[test]
    fn converts_ipv4_prefix_to_mask() {
        assert_eq!(ipv4_prefix_mask(0), 0);
        assert_eq!(ipv4_prefix_mask(24), 0xffffff00);
        assert_eq!(ipv4_prefix_mask(32), 0xffffffff);
    }
}
