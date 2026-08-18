//! Which stackql release to run in sidecar mode, and how its bundle is
//! resolved and verified.
//!
//! - [`BundleVersion::Latest`] (the default): resolve the newest
//!   stackql/stackql release at start-up and verify the download against the
//!   `.mcpb.sha256` asset published alongside it
//! - [`BundleVersion::Pinned`]: the release whose sha256 pins are baked into
//!   this crate ([`crate::STACKQL_VERSION`]) - fully offline verification,
//!   and the automatic fallback when the latest release cannot be resolved
//! - [`BundleVersion::Exact`]: a specific release, verified like `Latest`

#[cfg(feature = "sidecar")]
use crate::error::Result;
#[cfg(feature = "sidecar")]
use crate::platform::Platform;

/// Env var selecting the release: `latest`, `pinned`, or a version such as
/// `0.10.601` (leading `v` accepted). Overrides [`crate::Builder::version`].
pub const ENV_VERSION: &str = "STACKQL_MCP_VERSION";

#[cfg(feature = "sidecar")]
const RELEASES: &str = "https://github.com/stackql/stackql/releases";

/// Which stackql release the sidecar should run.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
#[non_exhaustive]
pub enum BundleVersion {
    /// The newest published release, resolved at start-up (default). Falls
    /// back to a cached release, then to [`BundleVersion::Pinned`], when the
    /// release cannot be resolved (offline).
    #[default]
    Latest,
    /// The release pinned in this crate ([`crate::STACKQL_VERSION`]),
    /// verified against the baked sha256 pins - no resolution step.
    Pinned,
    /// A specific release, e.g. `0.10.601`.
    Exact(String),
}

impl BundleVersion {
    /// Parse the env-var / string form: `latest`, `pinned`, or a version.
    pub fn parse(s: &str) -> Self {
        let s = s.trim();
        match s.to_ascii_lowercase().as_str() {
            "" | "latest" => BundleVersion::Latest,
            "pinned" => BundleVersion::Pinned,
            _ => BundleVersion::Exact(s.trim_start_matches('v').to_string()),
        }
    }

    /// The effective selection: `STACKQL_MCP_VERSION` if set, else `self`.
    pub fn resolved_from_env(&self) -> Self {
        match std::env::var(ENV_VERSION) {
            Ok(v) if !v.trim().is_empty() => BundleVersion::parse(&v),
            _ => self.clone(),
        }
    }
}

#[cfg(feature = "sidecar")]
/// A concrete bundle to download: release version, asset name, expected
/// sha256 and URL.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResolvedBundle {
    pub version: String,
    pub bundle_name: &'static str,
    pub sha256: String,
}

#[cfg(feature = "sidecar")]
impl ResolvedBundle {
    pub fn url(&self) -> String {
        format!("{RELEASES}/download/v{}/{}", self.version, self.bundle_name)
    }
}

#[cfg(feature = "sidecar")]
/// The pinned bundle for `platform`, without any network access.
pub fn pinned(platform: Platform) -> Result<ResolvedBundle> {
    let pin = crate::pins::pin_for(platform)?;
    Ok(ResolvedBundle {
        version: crate::pins::STACKQL_VERSION.to_string(),
        bundle_name: pin.bundle_name,
        sha256: pin.sha256.to_string(),
    })
}

/// Resolve `version` for `platform`. `Latest` and `Exact` need the network
/// (two small requests: the release tag, then the `.sha256` asset).
#[cfg(feature = "sidecar")]
pub fn resolve(version: &BundleVersion, platform: Platform) -> Result<ResolvedBundle> {
    let bundle_name = crate::pins::pin_for(platform)?.bundle_name;
    let version = match version {
        BundleVersion::Pinned => return pinned(platform),
        BundleVersion::Latest => latest_version()?,
        BundleVersion::Exact(v) => v.clone(),
    };
    let sha256 = published_sha256(&version, bundle_name)?;
    Ok(ResolvedBundle {
        version,
        bundle_name,
        sha256,
    })
}

/// The newest release version: `releases/latest` redirects to
/// `releases/tag/v<version>`; read the redirect instead of following it.
#[cfg(feature = "sidecar")]
fn latest_version() -> Result<String> {
    use crate::error::Error;
    let url = format!("{RELEASES}/latest");
    let http = |message: String| Error::Http {
        url: url.clone(),
        message,
    };
    let response = ureq::get(&url)
        .config()
        .max_redirects(0)
        .max_redirects_will_error(false)
        .build()
        .call()
        .map_err(|e| http(e.to_string()))?;
    let location = response
        .headers()
        .get("location")
        .and_then(|v| v.to_str().ok())
        .ok_or_else(|| {
            http(format!(
                "expected a redirect, got HTTP {}",
                response.status()
            ))
        })?;
    parse_tag_location(location)
        .map(str::to_string)
        .ok_or_else(|| http(format!("unexpected redirect target {location}")))
}

/// Fetch and parse the `.mcpb.sha256` asset for one release/bundle.
#[cfg(feature = "sidecar")]
fn published_sha256(version: &str, bundle_name: &str) -> Result<String> {
    use crate::error::Error;
    let url = format!("{RELEASES}/download/v{version}/{bundle_name}.sha256");
    let http = |message: String| Error::Http {
        url: url.clone(),
        message,
    };
    let body = ureq::get(&url)
        .call()
        .map_err(|e| http(e.to_string()))?
        .body_mut()
        .read_to_string()
        .map_err(|e| http(e.to_string()))?;
    parse_sha256_asset(&body)
        .map(str::to_string)
        .ok_or_else(|| http(format!("malformed sha256 asset: {body:?}")))
}

#[cfg(feature = "sidecar")]
/// `.../releases/tag/v0.10.601` -> `0.10.601`.
fn parse_tag_location(location: &str) -> Option<&str> {
    let (_, tag) = location.trim_end_matches('/').rsplit_once("/tag/")?;
    let version = tag.trim_start_matches('v');
    (!version.is_empty()
        && version
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-'))
    .then_some(version)
}

#[cfg(feature = "sidecar")]
/// First token of a `sha256sum`-style line, validated as 64 hex chars.
fn parse_sha256_asset(body: &str) -> Option<&str> {
    let token = body.split_whitespace().next()?;
    (token.len() == 64 && token.chars().all(|c| c.is_ascii_hexdigit())).then_some(token)
}

#[cfg(test)]
mod tests {
    #[allow(unused_imports)]
    use super::*;

    #[test]
    fn parses_version_selection() {
        assert_eq!(BundleVersion::parse("latest"), BundleVersion::Latest);
        assert_eq!(BundleVersion::parse(""), BundleVersion::Latest);
        assert_eq!(BundleVersion::parse("Pinned"), BundleVersion::Pinned);
        assert_eq!(
            BundleVersion::parse("v0.10.601"),
            BundleVersion::Exact("0.10.601".into())
        );
    }

    #[cfg(feature = "sidecar")]
    #[test]
    fn parses_release_tag_redirect() {
        assert_eq!(
            parse_tag_location("https://github.com/stackql/stackql/releases/tag/v0.10.601"),
            Some("0.10.601")
        );
        assert_eq!(
            parse_tag_location("https://github.com/stackql/stackql/releases"),
            None
        );
        assert_eq!(
            parse_tag_location("https://github.com/x/releases/tag/"),
            None
        );
    }

    #[cfg(feature = "sidecar")]
    #[test]
    fn parses_sha256_asset() {
        let hex = "4a1cad1345fba1aae1f31269fd96aebed7a7825b38f6509466c1c995ce114e52";
        assert_eq!(
            parse_sha256_asset(&format!("{hex}  stackql-mcp-linux-x64.mcpb\n")),
            Some(hex)
        );
        assert_eq!(parse_sha256_asset("not a hash"), None);
        assert_eq!(parse_sha256_asset(""), None);
    }

    #[cfg(feature = "sidecar")]
    #[test]
    fn pinned_bundle_url_points_at_the_pinned_release() {
        let b = pinned(Platform::LinuxX64).unwrap();
        assert_eq!(
            b.url(),
            format!(
                "https://github.com/stackql/stackql/releases/download/v{}/stackql-mcp-linux-x64.mcpb",
                crate::pins::STACKQL_VERSION
            )
        );
    }
}
