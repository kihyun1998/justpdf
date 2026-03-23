use crate::object::IndirectRef;

/// An Optional Content Group.
#[derive(Debug, Clone)]
pub struct OCGroup {
    /// The indirect reference to this OCG object.
    pub obj_ref: IndirectRef,
    /// Display name of the group.
    pub name: String,
    /// Intent (typically "View" or "Design").
    pub intent: Vec<String>,
    /// Usage information.
    pub usage: OCGUsage,
}

/// OCG usage categories.
#[derive(Debug, Clone, Default)]
pub struct OCGUsage {
    /// Print usage: whether this group is intended for printing.
    pub print: Option<OCGState>,
    /// View usage: whether this group is intended for viewing.
    pub view: Option<OCGState>,
    /// Export usage.
    pub export: Option<OCGState>,
}

/// OCG state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OCGState {
    On,
    Off,
}

impl OCGState {
    pub fn from_name(name: &[u8]) -> Self {
        match name {
            b"OFF" => Self::Off,
            _ => Self::On, // default per PDF spec
        }
    }

    pub fn to_name(&self) -> &'static [u8] {
        match self {
            Self::On => b"ON",
            Self::Off => b"OFF",
        }
    }
}

/// Visibility policy for OCMD.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VisibilityPolicy {
    /// All OCGs must be ON.
    AllOn,
    /// Any OCG must be ON.
    AnyOn,
    /// All OCGs must be OFF.
    AllOff,
    /// Any OCG must be OFF.
    AnyOff,
}

impl VisibilityPolicy {
    pub fn from_name(name: &[u8]) -> Self {
        match name {
            b"AllOn" => Self::AllOn,
            b"AnyOn" => Self::AnyOn,
            b"AllOff" => Self::AllOff,
            b"AnyOff" => Self::AnyOff,
            _ => Self::AnyOn, // default per PDF spec
        }
    }

    pub fn to_name(&self) -> &[u8] {
        match self {
            Self::AllOn => b"AllOn",
            Self::AnyOn => b"AnyOn",
            Self::AllOff => b"AllOff",
            Self::AnyOff => b"AnyOff",
        }
    }
}

/// Optional Content Membership Dictionary.
#[derive(Debug, Clone)]
pub struct OCMembership {
    /// References to OCGs.
    pub groups: Vec<IndirectRef>,
    /// Visibility policy.
    pub policy: VisibilityPolicy,
}

/// Configuration for optional content.
#[derive(Debug, Clone)]
pub struct OCConfig {
    /// Name of this configuration.
    pub name: Option<String>,
    /// Creator application.
    pub creator: Option<String>,
    /// Default state for new OCGs.
    pub base_state: OCGState,
    /// OCGs that are ON (overrides base_state).
    pub on_groups: Vec<IndirectRef>,
    /// OCGs that are OFF (overrides base_state).
    pub off_groups: Vec<IndirectRef>,
    /// Display order.
    pub order: Vec<OCOrderItem>,
}

/// An item in the OCG display order tree.
#[derive(Debug, Clone)]
pub enum OCOrderItem {
    /// A single OCG reference.
    Group(IndirectRef),
    /// A labeled sub-group with a name and children.
    SubGroup {
        name: Option<String>,
        children: Vec<OCOrderItem>,
    },
}

/// Full optional content properties from the catalog.
#[derive(Debug, Clone)]
pub struct OCProperties {
    /// All OCGs in the document.
    pub groups: Vec<OCGroup>,
    /// Default configuration.
    pub default_config: Option<OCConfig>,
    /// Additional configurations.
    pub configs: Vec<OCConfig>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_visibility_policy_from_name() {
        assert_eq!(VisibilityPolicy::from_name(b"AllOn"), VisibilityPolicy::AllOn);
        assert_eq!(VisibilityPolicy::from_name(b"AnyOn"), VisibilityPolicy::AnyOn);
        assert_eq!(VisibilityPolicy::from_name(b"AllOff"), VisibilityPolicy::AllOff);
        assert_eq!(VisibilityPolicy::from_name(b"AnyOff"), VisibilityPolicy::AnyOff);
        assert_eq!(VisibilityPolicy::from_name(b"Unknown"), VisibilityPolicy::AnyOn);
    }

    #[test]
    fn test_visibility_policy_to_name() {
        assert_eq!(VisibilityPolicy::AllOn.to_name(), b"AllOn");
        assert_eq!(VisibilityPolicy::AnyOn.to_name(), b"AnyOn");
        assert_eq!(VisibilityPolicy::AllOff.to_name(), b"AllOff");
        assert_eq!(VisibilityPolicy::AnyOff.to_name(), b"AnyOff");
    }

    #[test]
    fn test_visibility_policy_roundtrip() {
        for policy in [
            VisibilityPolicy::AllOn,
            VisibilityPolicy::AnyOn,
            VisibilityPolicy::AllOff,
            VisibilityPolicy::AnyOff,
        ] {
            assert_eq!(VisibilityPolicy::from_name(policy.to_name()), policy);
        }
    }

    #[test]
    fn test_ocg_usage_default() {
        let usage = OCGUsage::default();
        assert!(usage.print.is_none());
        assert!(usage.view.is_none());
        assert!(usage.export.is_none());
    }

    #[test]
    fn test_ocg_state_equality() {
        assert_eq!(OCGState::On, OCGState::On);
        assert_eq!(OCGState::Off, OCGState::Off);
        assert_ne!(OCGState::On, OCGState::Off);
    }
}
