//! Binary acquisition: resolve a runnable stackql binary from (in order)
//! env overrides, builder overrides, vendored bytes, or sidecar download.

use std::fs;
use std::path::{Path, PathBuf};

use crate::bundle;
use crate::cache;
use crate::error::{Error, Result};
#[cfg(feature = "sidecar")]
use crate::platform::Platform;
use crate::release::BundleVersion;

/// Acquisition inputs collected from the builder. Env overrides are read at
/// resolve time and take precedence over everything here.
#[derive(Default)]
pub struct Acquisition {
    /// Run this binary directly, skipping bundles entirely.
    pub binary: Option<PathBuf>,
    /// Extract this local .mcpb instead of downloading.
    pub bundle_path: Option<PathBuf>,
    /// Embedded .mcpb bytes (vendored feature).
    #[cfg(feature = "vendored")]
    pub bundle_bytes: Option<&'static [u8]>,
    /// Which release the sidecar runs (`STACKQL_MCP_VERSION` overrides).
    pub version: BundleVersion,
}

/// Resolve the server binary, acquiring it if needed. Blocking; call from a
/// blocking context (`start()` wraps this in `spawn_blocking`).
pub fn resolve_binary(acq: &Acquisition) -> Result<PathBuf> {
    // 1. Env binary override: run it as-is.
    if let Some(bin) = std::env::var_os(cache::ENV_BIN).filter(|v| !v.is_empty()) {
        return existing(PathBuf::from(bin), cache::ENV_BIN);
    }
    // 2. Builder binary override.
    if let Some(bin) = &acq.binary {
        return existing(bin.clone(), "Builder::binary");
    }
    // 3. Env bundle override: extract a local .mcpb. No pin check - the
    //    override is explicit operator intent and may be a custom build.
    if let Some(bundle_path) = std::env::var_os(cache::ENV_BUNDLE).filter(|v| !v.is_empty()) {
        return extract_local_bundle(&PathBuf::from(bundle_path), cache::ENV_BUNDLE);
    }
    // 4. Builder bundle override.
    if let Some(bundle_path) = &acq.bundle_path {
        return extract_local_bundle(bundle_path, "Builder::bundle_path");
    }
    // 5. Vendored bytes embedded by the caller.
    #[cfg(feature = "vendored")]
    if let Some(bytes) = acq.bundle_bytes {
        return extract_vendored(bytes);
    }
    // 6. Sidecar: shared cache, then verified download.
    #[cfg(feature = "sidecar")]
    {
        return sidecar(&acq.version.resolved_from_env());
    }
    #[cfg(not(feature = "sidecar"))]
    let _ = &acq.version;
    #[allow(unreachable_code)]
    Err(Error::Bundle(
        "no binary source: enable the sidecar feature, embed a bundle with the vendored \
         feature, or set STACKQL_MCP_BIN / STACKQL_MCP_BUNDLE"
            .into(),
    ))
}

fn existing(path: PathBuf, what: &'static str) -> Result<PathBuf> {
    if path.is_file() {
        Ok(path)
    } else {
        Err(Error::OverrideNotFound { what, path })
    }
}

/// Extract a caller-supplied .mcpb into a cache slot keyed by its content
/// hash, so different bundles never collide.
fn extract_local_bundle(bundle_path: &Path, what: &'static str) -> Result<PathBuf> {
    if !bundle_path.is_file() {
        return Err(Error::OverrideNotFound {
            what,
            path: bundle_path.to_path_buf(),
        });
    }
    let digest = crate::download::sha256_file(bundle_path)?;
    let dest = cache::bin_cache_root()?.join("custom").join(&digest[..16]);
    if let Some(binary) = bundle::cached_binary(&dest) {
        return Ok(binary);
    }
    let file = fs::File::open(bundle_path)?;
    bundle::extract_bundle(file, &dest)
}

#[cfg(feature = "vendored")]
fn extract_vendored(bytes: &'static [u8]) -> Result<PathBuf> {
    use sha2::{Digest, Sha256};
    let digest = Sha256::digest(bytes);
    let mut key = String::with_capacity(16);
    for b in &digest[..8] {
        key.push_str(&format!("{b:02x}"));
    }
    let dest = cache::bin_cache_root()?.join("vendored").join(key);
    if let Some(binary) = bundle::cached_binary(&dest) {
        return Ok(binary);
    }
    bundle::extract_bundle(std::io::Cursor::new(bytes), &dest)
}

#[cfg(feature = "sidecar")]
fn sidecar(version: &BundleVersion) -> Result<PathBuf> {
    use crate::release::{self, ResolvedBundle};

    let platform = Platform::detect()?;
    let bundle: ResolvedBundle = match release::resolve(version, platform) {
        Ok(b) => b,
        // Latest could not be resolved (offline, GitHub unreachable): run
        // what is already on disk rather than failing the start.
        Err(e) if *version == BundleVersion::Latest => {
            if let Some((cached_version, binary)) = newest_cached(platform)? {
                eprintln!(
                    "stackql-mcp: could not resolve the latest release ({e}); using cached {cached_version}"
                );
                return Ok(binary);
            }
            eprintln!(
                "stackql-mcp: could not resolve the latest release ({e}); using pinned {}",
                crate::pins::STACKQL_VERSION
            );
            release::pinned(platform)?
        }
        Err(e) => return Err(e),
    };

    let dest = cache::bundle_cache_dir(&bundle.version, platform.key())?;
    if let Some(binary) = bundle::cached_binary(&dest) {
        return Ok(binary);
    }

    let url = bundle.url();
    let mcpb = cache::bin_cache_root()?
        .join(&bundle.version)
        .join(bundle.bundle_name);
    eprintln!(
        "stackql-mcp: downloading {} (first run, cached at {})",
        url,
        dest.display()
    );
    crate::download::download_verified(&url, &bundle.sha256, &mcpb)?;
    let file = fs::File::open(&mcpb)?;
    let binary = bundle::extract_bundle(file, &dest)?;
    // The extracted dir is the cache; drop the archive to halve disk use.
    let _ = fs::remove_file(&mcpb);
    Ok(binary)
}

/// The highest-versioned release already extracted for `platform` in the
/// shared cache, if any: `(version, binary path)`.
#[cfg(feature = "sidecar")]
fn newest_cached(platform: Platform) -> Result<Option<(String, PathBuf)>> {
    let root = cache::bin_cache_root()?;
    let Ok(entries) = fs::read_dir(&root) else {
        return Ok(None);
    };
    let mut found: Vec<(Vec<u64>, String, PathBuf)> = entries
        .filter_map(|e| e.ok())
        .filter_map(|e| {
            let name = e.file_name().into_string().ok()?;
            let key = version_key(&name)?;
            let binary = bundle::cached_binary(&e.path().join(platform.key()))?;
            Some((key, name, binary))
        })
        .collect();
    found.sort();
    Ok(found.pop().map(|(_, v, b)| (v, b)))
}

/// `"0.10.601"` -> `[0, 10, 601]`; `None` for non-version dir names such as
/// `custom` or `vendored`.
#[cfg(feature = "sidecar")]
fn version_key(name: &str) -> Option<Vec<u64>> {
    let parts: Vec<u64> = name
        .split('.')
        .map(|p| p.parse().ok())
        .collect::<Option<_>>()?;
    (!parts.is_empty()).then_some(parts)
}

#[cfg(all(test, feature = "sidecar"))]
mod tests {
    use super::*;

    #[test]
    fn version_key_orders_numerically_and_skips_non_versions() {
        assert!(version_key("0.10.601") > version_key("0.10.99"));
        assert_eq!(version_key("custom"), None);
        assert_eq!(version_key("vendored"), None);
        assert_eq!(version_key(""), None);
    }
}
