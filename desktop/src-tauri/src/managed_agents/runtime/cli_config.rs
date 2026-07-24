use crate::managed_agents::{resolve_command, KnownAcpRuntime};

pub(crate) fn configure_runtime_cli(
    command: &mut std::process::Command,
    runtime: Option<&KnownAcpRuntime>,
) {
    let Some(runtime) = runtime else {
        return;
    };
    if runtime.id != "claude" {
        return;
    }
    if let Some(cli_path) = runtime.underlying_cli.and_then(resolve_command) {
        command.env("CLAUDE_CODE_EXECUTABLE", cli_path);
    }
}
