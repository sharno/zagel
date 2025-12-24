use std::collections::{BTreeMap, BTreeSet};

use crate::model::{Environment, RequestDraft};

pub(super) fn default_environment() -> Environment {
    Environment {
        name: "No environment".to_string(),
        vars: BTreeMap::new(),
    }
}

pub(super) fn with_default_environment(mut envs: Vec<Environment>) -> Vec<Environment> {
    let mut all = Vec::with_capacity(envs.len() + 1);
    all.push(default_environment());
    all.append(&mut envs);
    all
}

pub(super) fn status_with_missing(
    base: &str,
    draft: &RequestDraft,
    env: Option<&Environment>,
    extra_inputs: &[&str],
) -> String {
    let missing = missing_env_vars(draft, env, extra_inputs);
    if missing.is_empty() {
        base.to_string()
    } else {
        let env_name = env.map(|e| e.name.as_str()).unwrap_or("environment");
        format!(
            "{base} - Missing variables in {env_name}: {}",
            missing.join(", ")
        )
    }
}

fn missing_env_vars(
    draft: &RequestDraft,
    env: Option<&Environment>,
    extra_inputs: &[&str],
) -> Vec<String> {
    let mut placeholders = BTreeSet::new();
    for text in [&draft.url, &draft.headers, &draft.body] {
        for name in collect_placeholders(text) {
            placeholders.insert(name);
        }
    }
    for text in extra_inputs {
        for name in collect_placeholders(text) {
            placeholders.insert(name);
        }
    }

    let env_vars = env.map(|e| &e.vars);
    placeholders
        .into_iter()
        .filter(|name| env_vars.map_or(true, |vars| !vars.contains_key(name)))
        .collect()
}

fn collect_placeholders(input: &str) -> Vec<String> {
    let mut names = Vec::new();
    let mut search_start = 0;

    while let Some(open_rel) = input[search_start..].find("{{") {
        let open = search_start + open_rel;
        let after_open = open + 2;
        if let Some(close_rel) = input[after_open..].find("}}") {
            let close = after_open + close_rel;
            let candidate = input[after_open..close].trim();
            if !candidate.is_empty() {
                names.push(candidate.to_string());
            }
            search_start = close + 2;
        } else {
            break;
        }
    }

    names
}
