//! Standalone Artisan command entry, independent of HTTP serving.
use std::collections::BTreeMap;

use tokio_util::sync::CancellationToken;

/// Execute the current project's Artisan command with literal process arguments.
/// Ctrl-C requests cooperative cancellation and joins the owned PHP execution.
pub async fn run() -> Result<i32, String> {
    let args = std::env::args_os()
        .skip(1)
        .map(|arg| {
            arg.into_string()
                .map_err(|_| "Artisan arguments must be UTF-8".to_owned())
        })
        .collect::<Result<Vec<_>, _>>()?;
    let environment = std::env::vars_os()
        .map(|(key, value)| {
            Ok((
                key.into_string()
                    .map_err(|_| "Environment keys must be UTF-8".to_owned())?,
                value.into_string().map_err(|_| {
                    "Environment values must be UTF-8".to_owned()
                })?,
            ))
        })
        .collect::<Result<BTreeMap<_, _>, String>>()?;
    let cwd = std::env::current_dir().map_err(|error| error.to_string())?;
    let cancellation = CancellationToken::new();
    let command_cancellation = cancellation.clone();
    let mut command = tokio::task::spawn_blocking(move || {
        crate::artisan::run(cwd, args, environment, command_cancellation)
    });
    tokio::select! {
        result = &mut command => result.map_err(|error| error.to_string())?,
        signal = tokio::signal::ctrl_c() => {
            cancellation.cancel();
            // Cancellation is cooperative. Never detach the owned PHP thread.
            let result = command.await.map_err(|error| error.to_string())?;
            signal.map_err(|error| error.to_string())?;
            result
        }
    }
}
