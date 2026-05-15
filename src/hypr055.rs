//! Hyprland 0.55+ with Lua config rejects legacy `dispatch exec foo` IPC. Route those calls
//! through `hyprctl dispatch 'hl.dsp....'` when `hyprctl status` reports `configProvider: lua`.

use hyprland::dispatch::{
    Dispatch, DispatchType, WindowIdentifier, WorkspaceIdentifierWithSpecial,
};
use hyprland::shared::HyprError;
use std::sync::OnceLock;

fn uses_lua_ipc() -> bool {
    static CACHE: OnceLock<bool> = OnceLock::new();
    *CACHE.get_or_init(|| {
        let Ok(out) = std::process::Command::new("hyprctl")
            .args(["status", "-j"])
            .output()
        else {
            return false;
        };
        let Ok(text) = String::from_utf8(out.stdout) else {
            return false;
        };
        text.contains("\"configProvider\": \"lua\"")
    })
}

fn window_selector_for_lua(w: &WindowIdentifier<'_>) -> String {
    match w {
        WindowIdentifier::ClassRegularExpression(s) => format!("class:{s}"),
        _ => format!("{w}"),
    }
}

fn escape_lua_double_quoted(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 8);
    for ch in s.chars() {
        match ch {
            '\\' => out.push_str(r"\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            _ => out.push(ch),
        }
    }
    out
}

fn workspace_lua_rhs(ws: &WorkspaceIdentifierWithSpecial<'_>) -> String {
    match ws {
        WorkspaceIdentifierWithSpecial::Id(id) => id.to_string(),
        _ => format!("\"{}\"", escape_lua_double_quoted(&format!("{ws}"))),
    }
}

fn hyprctl_dispatch_lua(expr: &str) -> Result<(), HyprError> {
    let output = std::process::Command::new("hyprctl")
        .args(["dispatch", expr])
        .output()
        .map_err(HyprError::IoError)?;
    let stderr = String::from_utf8_lossy(&output.stderr);
    if stderr.contains("error:") {
        return Err(HyprError::NotOkDispatch(stderr.trim().to_string()));
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    if stdout.trim() == "ok" {
        return Ok(());
    }
    Err(HyprError::NotOkDispatch(if stdout.trim().is_empty() {
        stderr.trim().to_string()
    } else {
        stdout.trim().to_string()
    }))
}

fn try_lua_expr(cmd: &DispatchType<'_>) -> Option<String> {
    use DispatchType::*;
    Some(match cmd {
        Exec(cmdline) => format!(
            r#"hl.dsp.exec_cmd("{}")"#,
            escape_lua_double_quoted(cmdline)
        ),
        MoveToWorkspaceSilent(ws, win) => {
            let rhs = workspace_lua_rhs(ws);
            match win {
                Some(w) => {
                    let wstr = window_selector_for_lua(w);
                    format!(
                        r#"hl.dsp.window.move({{ workspace = {}, window = "{}", follow = false }})"#,
                        rhs,
                        escape_lua_double_quoted(&wstr)
                    )
                }
                None => format!(
                    r#"hl.dsp.window.move({{ workspace = {}, follow = false }})"#,
                    rhs
                ),
            }
        }
        MoveToWorkspace(ws, win) => {
            let rhs = workspace_lua_rhs(ws);
            match win {
                Some(w) => {
                    let wstr = window_selector_for_lua(w);
                    format!(
                        r#"hl.dsp.window.move({{ workspace = {}, window = "{}", follow = true }})"#,
                        rhs,
                        escape_lua_double_quoted(&wstr)
                    )
                }
                None => format!(
                    r#"hl.dsp.window.move({{ workspace = {}, follow = true }})"#,
                    rhs
                ),
            }
        }
        BringActiveToTop => r#"hl.dsp.window.alter_zorder({ mode = "top" })"#.to_string(),
        _ => return None,
    })
}

pub fn dispatch_compat(cmd: DispatchType<'_>) -> Result<(), HyprError> {
    if uses_lua_ipc() {
        if let Some(expr) = try_lua_expr(&cmd) {
            return hyprctl_dispatch_lua(&expr);
        }
    }
    Dispatch::call(cmd)
}
