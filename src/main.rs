#![forbid(unsafe_code)]
#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(clippy::panic)]

mod app;
mod browser;
mod chromium;
mod diff;
mod editor;
mod history;
#[cfg(target_os = "macos")]
mod macos;
mod manifest;
mod opener;
mod policy_tree;
mod tui;
#[cfg(target_os = "macos")]
mod watcher;
#[cfg(target_os = "windows")]
mod windows;

use anyhow::Result;
use app::App;
use browser::{ApplyResult, BrowserState};
use chromium::{Browser, detection, policy};
use manifest::Manifest;

const APPLY_BALANCED_ARG: &str = "--apply-balanced";

enum RunMode {
    Tui,
    ApplyBalanced,
}

impl RunMode {
    fn parse() -> Self {
        if std::env::args_os().any(|arg| arg == APPLY_BALANCED_ARG) {
            Self::ApplyBalanced
        } else {
            Self::Tui
        }
    }
}

fn main() -> Result<()> {
    let mode = RunMode::parse();

    #[cfg(target_os = "windows")]
    if windows::relaunch_elevated_if_needed() {
        return Ok(());
    }

    match mode {
        RunMode::Tui => {
            tui::install_panic_hook();

            let mut app = App::new()?;
            let terminal = tui::init()?;

            tui::run(terminal, &mut app)
        }
        RunMode::ApplyBalanced => apply_balanced_presets(),
    }
}

fn apply_balanced_presets() -> Result<()> {
    let manifest = Manifest::load()?;
    let mut applied = Vec::new();
    let mut unchanged = Vec::new();
    let mut skipped = Vec::new();

    for browser in Browser::all() {
        let preset = manifest.balanced_preset(browser);
        let mut state = BrowserState::new(
            browser,
            detection::detect(browser),
            policy::read(browser),
            preset.clone(),
        );

        if !state.detected() {
            skipped.push(format!("{}: not installed", browser.name()));
            continue;
        }

        state.stage_preset(preset);

        match state.apply_policy_changes() {
            Ok(ApplyResult::Applied) => applied.push(browser.name()),
            Ok(ApplyResult::AwaitingInstall) => applied.push(browser.name()),
            Ok(ApplyResult::NoChanges) => unchanged.push(browser.name()),
            Err(error) => skipped.push(format!("{}: {error}", browser.name())),
        }
    }

    if applied.is_empty() && unchanged.is_empty() {
        anyhow::bail!(
            "No supported browsers were updated.\n{}",
            skipped.join("\n")
        );
    }

    if !applied.is_empty() {
        println!("Applied balanced preset to: {}", applied.join(", "));
    }
    if !unchanged.is_empty() {
        println!("Already up to date: {}", unchanged.join(", "));
    }
    if !skipped.is_empty() {
        eprintln!("Skipped:");
        for item in skipped {
            eprintln!("- {item}");
        }
    }

    Ok(())
}
