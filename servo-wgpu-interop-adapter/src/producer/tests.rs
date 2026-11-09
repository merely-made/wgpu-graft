// Copyright 2026 Mark Alan Boykin
// SPDX-License-Identifier: MPL-2.0

use std::{collections::HashMap, sync::Arc};

use grafting::DiagnosticGpuSync;
use servo::{
    Key, NamedKey,
    accesskit::{Action, Node, NodeId, Rect, Role, Tree, TreeId, TreeUpdate, Uuid},
};

use super::{
    delegate::Delegate,
    keys::logical_key,
    types::{A11yFocusPolicy, ServoKeyEvent, ServoModifiers, parse_gpu_sync},
};

fn accessibility_delegate() -> Delegate {
    Delegate::new(String::new(), Arc::new(|| {}))
}

fn accessibility_update(tree: TreeId, grafted: Option<TreeId>) -> TreeUpdate {
    let mut node = Node::new(Role::GenericContainer);
    if let Some(grafted) = grafted {
        node.set_tree_id(grafted);
    }
    TreeUpdate {
        nodes: vec![(NodeId(0), node)],
        tree: Some(Tree::new(NodeId(0))),
        tree_id: tree,
        focus: NodeId(0),
    }
}

#[test]
fn servo_updates_survive_the_serde_boundary_unchanged() {
    let mut root = Node::new(Role::RootWebArea);
    root.set_children(vec![NodeId(2)]);
    root.set_tree_id(TreeId(Uuid::from_u64_pair(0, 110)));
    let mut child = Node::new(Role::Button);
    child.set_label("supplier node");
    child.add_action(Action::Click);
    child.set_bounds(Rect::new(1.0, 2.0, 30.0, 40.0));
    let update = TreeUpdate {
        nodes: vec![(NodeId(1), root), (NodeId(2), child)],
        tree: Some(Tree::new(NodeId(1))),
        tree_id: TreeId(Uuid::from_u64_pair(0, 109)),
        focus: NodeId(2),
    };
    let value = serde_json::to_value(&update).unwrap();
    assert_eq!(serde_json::from_value::<TreeUpdate>(value).unwrap(), update);
}

#[test]
fn accessibility_callbacks_preserve_order_and_nested_supplier_ids() {
    let delegate = accessibility_delegate();
    let wrapper = TreeId(Uuid::from_u64_pair(0, 101));
    let document = TreeId(Uuid::from_u64_pair(0, 102));
    let iframe = TreeId(Uuid::from_u64_pair(0, 103));
    delegate.accessibility_active.set(true);
    delegate.push_accessibility_update(accessibility_update(wrapper, Some(document)));
    delegate.push_accessibility_update(accessibility_update(document, Some(iframe)));
    delegate.push_accessibility_update(accessibility_update(iframe, None));
    let mut updates = delegate.accessibility_updates.borrow_mut();
    for (tree, grafted) in [
        (wrapper, Some(document)),
        (document, Some(iframe)),
        (iframe, None),
    ] {
        let update = updates.pop_front().unwrap();
        assert_eq!(update.tree_id, tree);
        assert_eq!(update.nodes[0].0, NodeId(0));
        assert_eq!(update.nodes[0].1.tree_id(), grafted);
    }
    assert!(updates.is_empty());
}

#[test]
fn accessibility_deactivation_retires_queued_and_late_callbacks() {
    let delegate = accessibility_delegate();
    let old = TreeId(Uuid::from_u64_pair(0, 104));
    let replacement = TreeId(Uuid::from_u64_pair(0, 105));
    delegate.accessibility_active.set(true);
    delegate.push_accessibility_update(accessibility_update(old, None));
    delegate.deactivate_accessibility();
    delegate.push_accessibility_update(accessibility_update(old, None));
    assert!(delegate.accessibility_updates.borrow().is_empty());
    delegate.accessibility_active.set(true);
    delegate.push_accessibility_update(accessibility_update(replacement, None));
    let mut updates = delegate.accessibility_updates.borrow_mut();
    assert_eq!(updates.pop_front().unwrap().tree_id, replacement);
    assert!(updates.is_empty());
}

#[test]
fn accessibility_focus_policy_projects_only_focus_after_raw_fifo_admission() {
    for policy in [A11yFocusPolicy::Upstream, A11yFocusPolicy::DocumentRoot] {
        let delegate = accessibility_delegate();
        delegate.accessibility_active.set(true);
        let raw = [(106, 76), (107, 92)].map(|(id, root_id)| {
            let mut root = Node::new(Role::RootWebArea);
            root.set_children(vec![NodeId(root_id + 1)]);
            let mut child = Node::new(Role::Button);
            child.set_label("supplier node");
            child.add_action(Action::Click);
            TreeUpdate {
                nodes: vec![(NodeId(root_id), root), (NodeId(root_id + 1), child)],
                tree: Some(Tree::new(NodeId(root_id))),
                tree_id: TreeId(Uuid::from_u64_pair(0, id)),
                focus: NodeId(1),
            }
        });
        for update in &raw {
            delegate.push_accessibility_update(update.clone());
        }
        assert_eq!(delegate.accessibility_updates.borrow()[0], raw[0]);
        assert_eq!(delegate.accessibility_updates.borrow()[1], raw[1]);
        for original in raw {
            let mut projected = delegate
                .poll_projected_accessibility_update(policy)
                .unwrap();
            assert_eq!(
                projected.focus,
                match policy {
                    A11yFocusPolicy::Upstream => original.focus,
                    A11yFocusPolicy::DocumentRoot => original.tree.as_ref().unwrap().root,
                }
            );
            // Full equality after restoring focus pins IDs, tree metadata,
            // node ordering, children, labels and action masks unchanged.
            projected.focus = original.focus;
            assert_eq!(projected, original);
        }
        assert!(
            delegate
                .poll_projected_accessibility_update(policy)
                .is_none()
        );
    }
}

#[test]
fn accessibility_focus_projection_does_not_invent_missing_metadata() {
    let delegate = accessibility_delegate();
    delegate.accessibility_active.set(true);
    let mut raw = accessibility_update(TreeId(Uuid::from_u64_pair(0, 108)), None);
    raw.tree = None;
    raw.focus = NodeId(999);
    delegate.push_accessibility_update(raw.clone());
    assert_eq!(
        delegate
            .poll_projected_accessibility_update(A11yFocusPolicy::DocumentRoot)
            .unwrap(),
        raw
    );
}

#[test]
fn accessibility_focus_setting_is_explicit_and_rejects_unknown_values() {
    assert_eq!(
        A11yFocusPolicy::parse(None).unwrap(),
        A11yFocusPolicy::DocumentRoot
    );
    assert_eq!(
        A11yFocusPolicy::parse(Some("document-root")).unwrap(),
        A11yFocusPolicy::DocumentRoot
    );
    assert_eq!(
        A11yFocusPolicy::parse(Some("upstream")).unwrap(),
        A11yFocusPolicy::Upstream
    );
    for invalid in ["", "root", "Document-Root", "document-root ", "1"] {
        assert!(A11yFocusPolicy::parse(Some(invalid)).is_err());
    }
}

#[test]
fn gpu_diagnostics_select_the_requested_wait_and_refuse_unknown_modes() {
    let timeout = std::time::Duration::from_millis(137);
    assert_eq!(
        parse_gpu_sync("existing", timeout).unwrap(),
        DiagnosticGpuSync::Existing
    );
    assert_eq!(
        parse_gpu_sync("producer", timeout).unwrap(),
        DiagnosticGpuSync::ProducerCompletion
    );
    assert_eq!(
        parse_gpu_sync("normalization", timeout).unwrap(),
        DiagnosticGpuSync::NormalizationCompletion { timeout }
    );
    assert_eq!(
        parse_gpu_sync("both", timeout).unwrap(),
        DiagnosticGpuSync::Both { timeout }
    );
    assert!(parse_gpu_sync("unknown", timeout).is_err());
}

fn event(key_code: u32, pressed: bool, text: Option<&str>) -> ServoKeyEvent {
    ServoKeyEvent {
        key_code,
        modifiers: ServoModifiers::default(),
        pressed,
        text: text.map(str::to_owned),
    }
}

#[test]
fn empty_and_control_text_preserve_named_keys_and_matching_release() {
    for (code, named) in [
        (8, NamedKey::Backspace),
        (9, NamedKey::Tab),
        (13, NamedKey::Enter),
        (37, NamedKey::ArrowLeft),
        (39, NamedKey::ArrowRight),
    ] {
        let mut pressed = HashMap::new();
        assert_eq!(
            logical_key(&event(code, true, Some("")), &mut pressed),
            Key::Named(named)
        );
        assert_eq!(
            logical_key(&event(code, false, None), &mut pressed),
            Key::Named(named)
        );
        assert!(pressed.is_empty());
    }
    let mut pressed = HashMap::new();
    assert_eq!(
        logical_key(&event(13, true, Some("\r")), &mut pressed),
        Key::Named(NamedKey::Enter)
    );
}

#[test]
fn unicode_text_is_preserved_on_release_without_text() {
    let mut pressed = HashMap::new();
    let expected = Key::Character("é中".into());
    assert_eq!(
        logical_key(&event(0, true, Some("é中")), &mut pressed),
        expected
    );
    assert_eq!(logical_key(&event(0, false, None), &mut pressed), expected);
    assert!(pressed.is_empty());
}
