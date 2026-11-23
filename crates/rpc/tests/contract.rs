//! `docs("json")` documents every JSON-RPC method.
#[test]
fn docs_cover_every_method() {
    let docs = tuse_core::docs(Some("json"));
    for m in tuse_rpc::METHODS {
        let short = m.rsplit('.').next().unwrap();
        let documented = docs.contains(&format!("`{m}`")) || docs.contains(&format!("`.{short}`"));
        assert!(documented, "docs(\"json\") misses `{m}`");
    }
}
