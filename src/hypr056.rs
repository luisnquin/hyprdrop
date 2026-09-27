//! Hyprland 0.56 reports special workspaces with a null or absent `id`, which hyprland-rs
//! refuses; restore the negative id older releases sent.

use hyprland::data::{Client, Workspace};
use hyprland::shared::HyprError;
use serde::de::DeserializeOwned;
use serde_json::Value;

const SPECIAL_WORKSPACE_ID: i64 = -99;

fn hyprctl_json(command: &str) -> Result<Value, HyprError> {
    let output = std::process::Command::new("hyprctl")
        .args(["-j", command])
        .output()
        .map_err(HyprError::IoError)?;
    Ok(serde_json::from_slice(&output.stdout)?)
}

fn restore_workspace_id(workspace: &mut Value) {
    if let Some(workspace) = workspace.as_object_mut() {
        let id = workspace.entry("id").or_insert(Value::Null);
        if id.is_null() {
            *id = SPECIAL_WORKSPACE_ID.into();
        }
    }
}

fn parse<T: DeserializeOwned>(value: Value) -> Result<T, HyprError> {
    Ok(serde_json::from_value(value)?)
}

pub fn clients() -> Result<Vec<Client>, HyprError> {
    let mut clients = hyprctl_json("clients")?;
    if let Some(clients) = clients.as_array_mut() {
        for client in clients {
            if let Some(workspace) = client.get_mut("workspace") {
                restore_workspace_id(workspace);
            }
        }
    }
    parse(clients)
}

pub fn active_workspace() -> Result<Workspace, HyprError> {
    let mut workspace = hyprctl_json("activeworkspace")?;
    restore_workspace_id(&mut workspace);
    parse(workspace)
}
