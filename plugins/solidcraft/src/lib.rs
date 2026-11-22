// SPDX-License-Identifier: MPL-2.0
//! The TeamCraft plugin for SolidCraft: a WebAssembly component for
//! SolidCraft's plugin runtime (`craft:plugin@0.2.0`, world
//! `command-plugin`, capability `commands@0.2`). See `README.md`.
//!
//! [`logic`] holds what the entries do; this file only connects it to the
//! host: `commands.run` for SolidCraft's commands and the exported
//! `entry.execute`. The bindings exist only in the WebAssembly build, so the
//! logic's tests run on the build machine.

pub mod bridge;
pub mod logic;

#[cfg(target_arch = "wasm32")]
mod component {
    wit_bindgen::generate!({
        path: "wit",
        world: "command-plugin",
    });

    use craft::plugin::types::{CommandFailure, FieldFailure, PluginError};
    use exports::craft::plugin::entry::Guest;

    use super::logic::{self, EntryError, Host};

    /// SolidCraft's command surface through the host's `commands.run`.
    struct Commands;

    impl Host for Commands {
        fn run(
            &mut self,
            command: &str,
            args: serde_json::Value,
        ) -> Result<serde_json::Value, String> {
            let reply = craft::plugin::commands::run(command, &args.to_string())
                .map_err(|e| describe(&e))?;
            serde_json::from_str(&reply)
                .map_err(|e| format!("{command} answered with no JSON: {e}"))
        }

        fn read_file(&mut self, path: &str) -> Result<Option<Vec<u8>>, String> {
            match craft::plugin::files::read_file(super::bridge::GRANT, path) {
                Ok(bytes) => Ok(Some(bytes)),
                // a file that is not there is an answer, not a failure (SolidCraft #17899)
                Err(PluginError::InvalidInput(f)) if f.reason.contains("no such file") => Ok(None),
                Err(e) => Err(describe(&e)),
            }
        }

        fn write_file(&mut self, path: &str, data: &[u8]) -> Result<(), String> {
            craft::plugin::files::write_file(super::bridge::GRANT, path, data)
                .map_err(|e| describe(&e))
        }

        fn now(&mut self) -> u64 {
            craft::plugin::clock::now()
        }
    }

    fn describe(e: &PluginError) -> String {
        match e {
            PluginError::PermissionDenied(s)
            | PluginError::LimitExceeded(s)
            | PluginError::Trap(s)
            | PluginError::Unsupported(s) => s.clone(),
            PluginError::CommandFailed(f) => format!("{}: {}", f.command, f.reason),
            PluginError::InvalidManifest(f) | PluginError::InvalidInput(f) => {
                format!("{}: {}", f.field, f.reason)
            }
            PluginError::Cancelled => "cancelled".into(),
        }
    }

    struct Plugin;

    impl Guest for Plugin {
        fn execute(entry: String, params: String) -> Result<String, PluginError> {
            let params: serde_json::Value = serde_json::from_str(&params).map_err(|e| {
                PluginError::InvalidInput(FieldFailure {
                    field: "params".into(),
                    reason: format!("the entry's input is not JSON ({e}); nothing was changed"),
                })
            })?;
            match logic::execute(&mut Commands, &entry, &params) {
                Ok(result) => Ok(result.to_string()),
                Err(EntryError::Input { field, reason }) => {
                    Err(PluginError::InvalidInput(FieldFailure { field, reason }))
                }
                Err(EntryError::Command { command, reason }) => {
                    Err(PluginError::CommandFailed(CommandFailure {
                        command,
                        reason,
                    }))
                }
                Err(EntryError::Unsupported(text)) => Err(PluginError::Unsupported(text)),
            }
        }
    }

    export!(Plugin);
}
