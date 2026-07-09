//! FitLLM CLI. STUB — replaced by the CLI/recommend agent.

fn main() -> anyhow::Result<()> {
    let profile = fitllm_core::hardware::profile()?;
    println!("{}", serde_json::to_string_pretty(&profile)?);
    Ok(())
}
