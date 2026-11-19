/// The reference the agent reads with `docs()`.
pub fn docs() -> String {
    format!(
        "{}\n\n{}",
        include_str!("../../../docs/agent.md").trim_end(),
        include_str!("../../../docs/config.md").trim_end()
    )
}
