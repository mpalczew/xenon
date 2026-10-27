use super::*;

#[test]
fn client_msgs_parse_from_page_shapes() {
    let hello: ClientMsg = serde_json::from_str(r#"{"t":"hello","token":"abc"}"#).unwrap();
    assert_eq!(
        hello,
        ClientMsg::Hello {
            token: "abc".into()
        }
    );
    let attach: ClientMsg =
        serde_json::from_str(r#"{"t":"attach","workspaceId":"w","tabId":7}"#).unwrap();
    assert_eq!(
        attach,
        ClientMsg::Attach {
            workspace_id: "w".into(),
            tab_id: 7
        }
    );
    for low_bandwidth in [false, true] {
        let msg = ClientMsg::Bandwidth { low_bandwidth };
        let json = format!(r#"{{"t":"bandwidth","lowBandwidth":{low_bandwidth}}}"#);
        assert_eq!(serde_json::from_str::<ClientMsg>(&json).unwrap(), msg);
        assert_eq!(serde_json::to_string(&msg).unwrap(), json);
    }
    for (json, appearance) in [
        (
            r#"{"t":"appearance","appearance":"dark"}"#,
            Some(AppearanceWire::Dark),
        ),
        (
            r#"{"t":"appearance","appearance":"light"}"#,
            Some(AppearanceWire::Light),
        ),
        (r#"{"t":"appearance","appearance":null}"#, None),
    ] {
        let msg = ClientMsg::Appearance { appearance };
        assert_eq!(serde_json::from_str::<ClientMsg>(json).unwrap(), msg);
        assert_eq!(serde_json::to_string(&msg).unwrap(), json);
    }
    let theme = ServerMsg::Theme {
        theme: ThemeWire {
            bg: "#000000".into(),
            fg: "#ffffff".into(),
            cursor: "#ff0000".into(),
        },
    };
    assert_eq!(
        serde_json::to_string(&theme).unwrap(),
        r##"{"t":"theme","theme":{"bg":"#000000","fg":"#ffffff","cursor":"#ff0000"}}"##
    );
    let key: ClientMsg = serde_json::from_str(r#"{"t":"key","key":"shift-tab"}"#).unwrap();
    assert_eq!(
        key,
        ClientMsg::Key {
            key: NamedKey::ShiftTab
        }
    );
    let fit: ClientMsg = serde_json::from_str(r#"{"t":"fit","cols":45,"rows":30}"#).unwrap();
    assert_eq!(fit, ClientMsg::Fit { cols: 45, rows: 30 });
    let wheel: ClientMsg = serde_json::from_str(r#"{"t":"wheel","rows":-2}"#).unwrap();
    assert_eq!(wheel, ClientMsg::Wheel { rows: -2 });
    let bottom: ClientMsg = serde_json::from_str(r#"{"t":"bottom"}"#).unwrap();
    assert_eq!(bottom, ClientMsg::Bottom);
    let click: ClientMsg = serde_json::from_str(r#"{"t":"click","col":3,"row":4}"#).unwrap();
    assert_eq!(click, ClientMsg::Click { col: 3, row: 4 });
    let page: ClientMsg = serde_json::from_str(r#"{"t":"key","key":"page-up"}"#).unwrap();
    assert_eq!(
        page,
        ClientMsg::Key {
            key: NamedKey::PageUp
        }
    );
}

#[test]
fn task_messages_roundtrip_and_hide_secrets() {
    let run: ClientMsg =
        serde_json::from_str(r#"{"t":"runTask","workspaceId":"w","label":"build"}"#).unwrap();
    assert_eq!(
        run,
        ClientMsg::RunTask {
            workspace_id: "w".into(),
            label: "build".into()
        }
    );
    let answer: ClientMsg =
        serde_json::from_str(r#"{"t":"taskInput","token":4,"value":"hunter2"}"#).unwrap();
    assert_eq!(
        answer,
        ClientMsg::TaskInput {
            token: 4,
            value: Some(Secret("hunter2".into()))
        }
    );
    assert!(!format!("{answer:?}").contains("hunter2"));
    let cancel: ClientMsg = serde_json::from_str(r#"{"t":"taskInput","token":4}"#).unwrap();
    assert_eq!(
        cancel,
        ClientMsg::TaskInput {
            token: 4,
            value: None
        }
    );
    let ask = ServerMsg::TaskInput {
        token: 4,
        id: "pw".into(),
        description: "Password".into(),
        default: None,
        kind: TaskInputKind::Password,
        options: vec![],
    };
    assert_eq!(
        serde_json::to_string(&ask).unwrap(),
        r#"{"t":"taskInput","token":4,"id":"pw","description":"Password","kind":"password"}"#
    );
    let info = TaskInfo {
        label: "b".into(),
        detail: None,
        command: "make".into(),
        group: Some("build".into()),
        default: true,
        unsupported: None,
    };
    assert_eq!(
        serde_json::to_string(&info).unwrap(),
        r#"{"label":"b","command":"make","group":"build","default":true}"#
    );
}

#[test]
fn frame_serializes_compactly() {
    let msg = ServerMsg::Frame {
        seq: 3,
        cols: 4,
        rows: 1,
        full: true,
        cursor: Some(CursorWire {
            r: 0,
            c: 2,
            s: CursorShapeWire::Block,
        }),
        lines: vec![(0, vec![("ab".into(), 0)])],
        scrolled: false,
    };
    let json = serde_json::to_string(&msg).unwrap();
    assert_eq!(
        json,
        r#"{"t":"frame","seq":3,"cols":4,"rows":1,"full":true,"cursor":{"r":0,"c":2,"s":"block"},"lines":[[0,[["ab",0]]]]}"#
    );
}
