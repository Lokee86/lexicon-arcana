mod call_model;
mod call_resolution;
mod call_support;
mod cli;
mod contract;
mod dataflow;
mod declarations;
mod dependencies;
mod discovery;
mod emit;
mod expr_eval;
mod expr_values;
mod extractor;
mod flow;
mod function_index;
mod implementations;
mod imports;
mod items;
mod model;
mod orchestrator;
mod parser;
mod paths;
mod relationships;
mod resolve;
mod semantic;
mod semantic_actions;
mod semantic_facts;
#[cfg(test)]
mod semantic_facts_tests;
mod syntax;
mod type_resolution;

fn main() -> anyhow::Result<()> {
    cli::run()
}

#[cfg(test)]
mod tests;
