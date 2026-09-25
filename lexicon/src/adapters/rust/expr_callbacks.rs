use crate::adapters::rust::call_resolution;
use crate::adapters::rust::flow::Analyzer;
use crate::adapters::rust::model::ValueSet;

pub(crate) fn propagate_known_callback(
    analyzer: &mut Analyzer<'_>,
    method: &str,
    receiver: &ValueSet,
    arguments: &[ValueSet],
) {
    let item = receiver
        .contained_values
        .first()
        .cloned()
        .unwrap_or_else(|| ValueSet {
            types: receiver.contained_types.clone(),
            builtin: receiver.builtin,
            external: receiver.external,
            unknown: receiver.contained_types.is_empty() && !receiver.builtin && !receiver.external,
            ..ValueSet::default()
        });
    let error = receiver
        .contained_values
        .get(1)
        .cloned()
        .unwrap_or_else(|| item.clone());
    if method == "map_or_else" {
        propagate_callback(analyzer, arguments.first(), &[]);
        propagate_callback(analyzer, arguments.get(1), std::slice::from_ref(&item));
        return;
    }
    let (callback_index, inputs): (usize, Vec<ValueSet>) = match method {
        "sort_by" => (0, vec![item.clone(), item]),
        "map" | "filter_map" | "and_then" | "filter" | "inspect" | "find" | "any" | "all"
        | "position" | "take_while" | "skip_while" | "sort_by_key" | "for_each" | "with"
        | "is_some_and" | "is_ok_and" | "is_err_and" => (0, vec![item]),
        "then" | "then_with" | "unwrap_or_else" => (0, Vec::new()),
        "map_err" | "or_else" => (0, vec![error]),
        "fold" | "try_fold" => {
            let Some(initial) = arguments.first() else {
                return;
            };
            (1, vec![initial.clone(), item])
        }
        _ => return,
    };
    propagate_callback(analyzer, arguments.get(callback_index), &inputs);
}

fn propagate_callback(
    analyzer: &mut Analyzer<'_>,
    callback: Option<&ValueSet>,
    inputs: &[ValueSet],
) {
    let Some(callback) = callback else {
        return;
    };
    for target in &callback.callables {
        for (index, input) in inputs.iter().enumerate() {
            analyzer
                .result
                .parameter_updates
                .entry((target.clone(), index))
                .or_default()
                .merge(input);
        }
    }
}

pub(crate) fn merge_callback_return(
    analyzer: &Analyzer<'_>,
    method: &str,
    arguments: &[ValueSet],
    output: &mut ValueSet,
) {
    let callback_index = match method {
        "map" | "filter_map" | "map_err" | "and_then" | "or_else" | "then" | "then_with"
        | "unwrap_or_else" => 0,
        "map_or_else" => 1,
        _ => return,
    };
    let Some(callback) = arguments.get(callback_index) else {
        return;
    };
    if callback.callables.is_empty() {
        return;
    }
    let returned = call_resolution::returns_for_targets(analyzer.context, &callback.callables);
    if returned == ValueSet::default() {
        return;
    }
    if matches!(method, "unwrap_or_else" | "map_or_else") {
        output.merge(&returned);
        return;
    }
    output.builtin = true;
    output
        .contained_types
        .extend(returned.types.iter().cloned());
    output
        .contained_types
        .extend(returned.contained_types.iter().cloned());
    output.contained_values.push(returned);
}
