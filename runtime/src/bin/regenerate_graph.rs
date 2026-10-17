use std::collections::BTreeMap;
use std::error::Error;
use std::path::PathBuf;

use truenorth_mcp::engine::graph::{build_graph_report_with_contracts, to_jsonl};
use truenorth_mcp::engine::skill::{discover_skills, read_skill_raw};
use truenorth_mcp::engine::skill_contract::read_skill_contract;
use truenorth_mcp::engine::skill_parser::parse_skill;

fn main() -> Result<(), Box<dyn Error>> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .ok_or("runtime manifest has no repository parent")?
        .to_path_buf();
    let skills = discover_skills(&root)
        .into_iter()
        .map(|entry| read_skill_raw(&root, &entry.name).map(|raw| parse_skill(&raw)))
        .collect::<Result<Vec<_>, _>>()?;
    let contracts = skills
        .iter()
        .map(|skill| {
            read_skill_contract(&root, &skill.name).map(|contract| (skill.name.clone(), contract))
        })
        .collect::<Result<BTreeMap<_, _>, _>>()?;
    let result = build_graph_report_with_contracts(&skills, &contracts);
    if !result.diagnostics.unresolved_mentions.is_empty()
        || !result.diagnostics.unclassified_mentions.is_empty()
    {
        return Err(format!("graph diagnostics: {:?}", result.diagnostics).into());
    }
    std::fs::write(
        root.join(".agent/tasks/skill-graph.jsonl"),
        to_jsonl(&result.graph),
    )?;
    Ok(())
}
