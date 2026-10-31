//! Validated, session-addressed messages between Rust, PTYs and the local webview.
use crate::tabs::TabId;
use base64::Engine;
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum WorkspaceRequest {
    Ready {},
    NewTab {},
    SelectTab { id: TabId },
    SelectNumber { number: u8 },
    CycleTab { backwards: bool },
    CloseTab { id: TabId },
    RenameTab { id: TabId, text: String },
    Input { id: TabId, data: String },
    Resize { id: TabId, cols: u16, rows: u16 },
    Copy { id: TabId, data: String },
    Paste { id: TabId },
    Title { id: TabId, text: String },
    OutputAck { id: TabId, sequence: u32 },
}
pub fn parse_workspace_request(body: &str) -> Result<WorkspaceRequest, String> {
    let request: WorkspaceRequest =
        serde_json::from_str(body).map_err(|e| format!("invalid workspace request: {e}"))?;
    if matches!(
        request,
        WorkspaceRequest::Resize { cols: 0, .. } | WorkspaceRequest::Resize { rows: 0, .. }
    ) {
        return Err("terminal dimensions must be positive".into());
    }
    Ok(request)
}
#[derive(Debug, Clone, Serialize)]
pub struct TabSummary {
    pub id: TabId,
    pub title: String,
    pub manual: Option<String>,
    pub busy: bool,
}
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum UiDelivery {
    CreateTab {
        id: TabId,
    },
    RemoveTab {
        id: TabId,
    },
    SelectTab {
        id: TabId,
    },
    TabMetadata {
        tabs: Vec<TabSummary>,
        active: Option<TabId>,
    },
    Notice {
        text: String,
    },
    Output {
        id: TabId,
        sequence: u32,
        data: String,
    },
    End {
        id: TabId,
        sequence: u32,
        message: String,
    },
    Paste {
        id: TabId,
        text: String,
    },
    Appearance {
        appearance: crate::config::Appearance,
    },
    TabPosition {
        position: crate::config::TabPosition,
    },
    TerminalTheme {
        theme: String,
    },
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TerminalEvent {
    Output(Vec<u8>),
    Exited { code: Option<u32> },
    Failed(String),
}
pub type TerminalDelivery = TerminalEvent;

impl From<&crate::session_output::SessionDelivery> for UiDelivery {
    fn from(delivery: &crate::session_output::SessionDelivery) -> Self {
        let id = delivery.id;
        let sequence = delivery.sequence;
        match &delivery.event {
            TerminalDelivery::Output(bytes) => Self::Output {
                id,
                sequence,
                data: base64::engine::general_purpose::STANDARD.encode(bytes),
            },
            TerminalDelivery::Exited { code } => Self::End {
                id,
                sequence,
                message: code.map_or_else(
                    || "Process ended.".into(),
                    |code| format!("Process ended with status {code}."),
                ),
            },
            TerminalDelivery::Failed(text) => Self::End {
                id,
                sequence,
                message: format!("Terminal error: {text}"),
            },
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn wire_rejects_missing_or_invalid_id_and_extra_fields() {
        for body in [
            r#"{"type":"input","data":"x"}"#,
            r#"{"type":"input","id":0,"data":"x"}"#,
            r#"{"type":"paste","id":1,"extra":0}"#,
            r#"{"type":"resize","id":1,"cols":0,"rows":24}"#,
            r#"{"type":"resize","id":1,"cols":80,"rows":65536}"#,
            r#"{"type":"ready","id":1}"#,
            r#"{"type":"bogus"}"#,
        ] {
            assert!(parse_workspace_request(body).is_err(), "{body}");
        }
        assert!(
            matches!(parse_workspace_request(r#"{"type":"input","id":1,"data":"é"}"#).unwrap(),WorkspaceRequest::Input{id,data} if id.get()==1&&data=="é")
        );
        let encoded = serde_json::to_string(&UiDelivery::Notice {
            text: "</script>\"".into(),
        })
        .unwrap();
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&encoded).unwrap()["text"],
            "</script>\""
        );
    }
    #[test]
    fn binary_output_and_terminal_end_keep_id_and_sequence() {
        let id = TabId::new(2).unwrap();
        let delivery = crate::session_output::SessionDelivery {
            id,
            sequence: 7,
            event: TerminalDelivery::Output(vec![0, 255]),
        };
        let wire = serde_json::to_value(UiDelivery::from(&delivery)).unwrap();
        assert_eq!(wire["id"], 2);
        assert_eq!(wire["sequence"], 7);
        assert_eq!(wire["data"], "AP8=");
        let end = UiDelivery::from(&crate::session_output::SessionDelivery {
            id,
            sequence: 8,
            event: TerminalDelivery::Exited { code: Some(3) },
        });
        assert_eq!(
            serde_json::to_value(end).unwrap()["message"],
            "Process ended with status 3."
        );
    }
}
