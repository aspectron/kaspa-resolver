use crate::imports::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Service {
    Kaspa,
    Sparkle,
}

impl Display for Service {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Service::Kaspa => "kaspa",
            Service::Sparkle => "sparkle",
        };
        f.write_str(s)
    }
}
