// Act 3, the vendored agent (slide "VENDORED DETAILED"): fetch the pinned
// .mcpb into the shared cache at build time and hand its path to
// include_bundle!(). Override with STACKQL_MCP_BUNDLE_FILE to embed a bundle
// you already have. Quiet on purpose: a build script can only talk to the
// terminal through cargo:warning=, and the embedded bundle's path is printed
// by the binary at run time anyway.
fn main() {
    println!("cargo:rerun-if-env-changed=STACKQL_MCP_BUNDLE_FILE");
    if std::env::var_os("STACKQL_MCP_BUNDLE_FILE").is_some() {
        return;
    }
    let path = stackql_mcp::fetch_bundle().expect("fetch the platform stackql-mcp bundle");
    println!("cargo:rustc-env=STACKQL_MCP_BUNDLE_FILE={}", path.display());
}
