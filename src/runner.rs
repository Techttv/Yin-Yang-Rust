use std::any::Any;
use std::thread;

use anyhow::anyhow;

use crate::config::{Config, PluginConfig};
use crate::plugin::{Env, Plugin, Variant};

pub enum Outcome {
    Applied,
    /// Enabled but cannot run now (program or service missing).
    Skipped(String),
    Failed(anyhow::Error),
}

/// Enabled plugins, in registry order, with their config. Notification plugins drop out when `quiet`.
fn selected<'a>(
    cfg: &'a Config,
    plugins: &'a [Box<dyn Plugin>],
    quiet: bool,
) -> Vec<(&'a dyn Plugin, &'a PluginConfig)> {
    plugins
        .iter()
        .filter_map(|plugin| {
            let plugin_cfg = cfg.plugins.get(plugin.key())?;
            let wanted = plugin_cfg.enabled && !(quiet && plugin.is_notification());
            wanted.then_some((plugin.as_ref(), plugin_cfg))
        })
        .collect()
}

/// Applies `variant` with every enabled plugin. `system` goes first and alone, because the Plasma
/// look-and-feel also sets colours and icons that other plugins then override; the rest run in
/// parallel. One plugin failing, even by panicking, never affects the others.
pub fn apply_all(
    env: &Env,
    cfg: &Config,
    plugins: &[Box<dyn Plugin>],
    variant: Variant,
    quiet: bool,
) -> Vec<(String, Outcome)> {
    let selected = selected(cfg, plugins, quiet);
    let mut outcomes: Vec<Option<Outcome>> = selected.iter().map(|_| None).collect();
    let (mut first, mut rest) = (Vec::new(), Vec::new());

    for (i, (plugin, _)) in selected.iter().enumerate() {
        if !plugin.available(env) {
            outcomes[i] = Some(Outcome::Skipped("not available".into()));
        } else if plugin.key() == "system" {
            first.push(i);
        } else {
            rest.push(i);
        }
    }

    for batch in [first, rest] {
        for (i, outcome) in run_batch(env, variant, &selected, &batch) {
            outcomes[i] = Some(outcome);
        }
    }

    selected
        .iter()
        .zip(outcomes)
        .map(|((plugin, _), outcome)| (plugin.key().to_string(), outcome.expect("every plugin has an outcome")))
        .collect()
}

fn run_batch(
    env: &Env,
    variant: Variant,
    selected: &[(&dyn Plugin, &PluginConfig)],
    batch: &[usize],
) -> Vec<(usize, Outcome)> {
    thread::scope(|scope| {
        let handles: Vec<_> = batch
            .iter()
            .map(|&i| {
                let (plugin, plugin_cfg) = selected[i];
                (i, scope.spawn(move || plugin.apply(env, variant, plugin_cfg)))
            })
            .collect();

        handles
            .into_iter()
            .map(|(i, handle)| {
                let outcome = match handle.join() {
                    Ok(Ok(())) => Outcome::Applied,
                    Ok(Err(e)) => Outcome::Failed(e),
                    Err(payload) => Outcome::Failed(anyhow!("plugin panicked: {}", panic_message(&payload))),
                };
                (i, outcome)
            })
            .collect()
    })
}

fn panic_message(payload: &Box<dyn Any + Send>) -> String {
    payload
        .downcast_ref::<&str>()
        .map(|s| s.to_string())
        .or_else(|| payload.downcast_ref::<String>().cloned())
        .unwrap_or_else(|| "unknown panic".into())
}

/// What `apply_all` would do, one line per plugin, without doing anything.
pub fn plan(cfg: &Config, plugins: &[Box<dyn Plugin>], variant: Variant, quiet: bool) -> Vec<String> {
    selected(cfg, plugins, quiet)
        .into_iter()
        .map(|(plugin, plugin_cfg)| plugin.describe(variant, plugin_cfg))
        .collect()
}
