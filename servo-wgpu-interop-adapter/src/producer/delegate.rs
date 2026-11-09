// Copyright 2026 Mark Alan Boykin
// SPDX-License-Identifier: MPL-2.0

use std::{
    cell::{Cell, RefCell},
    collections::VecDeque,
    sync::Arc,
};

use servo::{LoadStatus, PermissionRequest, WebView, WebViewDelegate};
use url::Url;

use super::types::{A11yFocusPolicy, ServoCursor, ServoEvent};

pub(super) struct Delegate {
    pub events: RefCell<VecDeque<ServoEvent>>,
    pub cursors: RefCell<VecDeque<ServoCursor>>,
    pub accessibility_updates: RefCell<VecDeque<servo::accesskit::TreeUpdate>>,
    pub accessibility_active: Cell<bool>,
    pub accessibility_focus_provenance: Cell<u32>,
    pub ready: Cell<bool>,
    pub animating: Cell<bool>,
    pub requested_url: RefCell<String>,
    pub wake: Arc<dyn Fn() + Send + Sync>,
}

impl Delegate {
    pub(super) fn new(url: String, wake: Arc<dyn Fn() + Send + Sync>) -> Self {
        Self {
            events: RefCell::new(VecDeque::new()),
            cursors: RefCell::new(VecDeque::new()),
            accessibility_updates: RefCell::new(VecDeque::new()),
            accessibility_active: Cell::new(false),
            accessibility_focus_provenance: Cell::new(0),
            ready: Cell::new(false),
            animating: Cell::new(false),
            requested_url: RefCell::new(url),
            wake,
        }
    }

    pub(super) fn push(&self, event: ServoEvent) {
        self.events.borrow_mut().push_back(event);
        (self.wake)();
    }

    pub(super) fn push_accessibility_update(&self, update: servo::accesskit::TreeUpdate) {
        if !self.accessibility_active.get() {
            return;
        }
        // Servo sends its wrapper graft before the document's update. Keep
        // every nested TreeId and the callback order intact for host admission.
        self.accessibility_updates.borrow_mut().push_back(update);
        (self.wake)();
    }

    pub(super) fn deactivate_accessibility(&self) {
        self.accessibility_active.set(false);
        self.accessibility_updates.borrow_mut().clear();
        self.accessibility_focus_provenance.set(0);
    }

    /// Pop the next update with the focus policy applied. The FIFO always
    /// holds the exact raw supplier callback; only focus changes at this
    /// boundary, preserving supplier identities, graph, bounds and actions.
    pub(super) fn poll_projected_accessibility_update(
        &self,
        policy: A11yFocusPolicy,
    ) -> Option<servo::accesskit::TreeUpdate> {
        let mut update = self.accessibility_updates.borrow_mut().pop_front()?;
        let supplier_focus = update.focus;
        policy.project(&mut update);
        // Bound provenance to the first 32 callbacks per activation. Metadata
        // alone is logged; labels, values and page text are never included.
        let count = self.accessibility_focus_provenance.get();
        if count < 32 {
            tracing::debug!(target: "grafting_servo::a11y",
                tree_id = %update.tree_id.0,
                supplier_focus = supplier_focus.0,
                projected_focus = update.focus.0,
                policy = policy.name(),
                "Servo accessibility focus projection");
            self.accessibility_focus_provenance.set(count + 1);
        }
        Some(update)
    }
}

impl WebViewDelegate for Delegate {
    fn notify_cursor_changed(&self, _: WebView, cursor: servo::Cursor) {
        use servo::Cursor as C;
        let cursor = match cursor {
            C::Pointer => ServoCursor::Pointer,
            C::Text | C::VerticalText => ServoCursor::Text,
            C::Grab => ServoCursor::Grab,
            C::Grabbing => ServoCursor::Grabbing,
            C::Crosshair => ServoCursor::Crosshair,
            C::Move | C::AllScroll => ServoCursor::Move,
            C::NResize | C::SResize | C::NsResize => ServoCursor::ResizeNs,
            C::EResize | C::WResize | C::EwResize => ServoCursor::ResizeEw,
            C::NeResize | C::SwResize | C::NeswResize => ServoCursor::ResizeNesw,
            C::NwResize | C::SeResize | C::NwseResize => ServoCursor::ResizeNwse,
            C::NotAllowed | C::NoDrop => ServoCursor::NotAllowed,
            C::None => ServoCursor::Hidden,
            _ => ServoCursor::Default,
        };
        self.cursors.borrow_mut().push_back(cursor);
        (self.wake)();
    }
    fn notify_accessibility_tree_update(&self, _: WebView, update: servo::accesskit::TreeUpdate) {
        self.push_accessibility_update(update);
    }
    fn request_permission(&self, _: WebView, request: PermissionRequest) {
        // The producer exposes no permission UI or decision persistence.
        // Resolve explicitly rather than inherit a caller-selected fallback.
        request.deny();
    }
    fn notify_new_frame_ready(&self, _: WebView) {
        self.ready.set(true);
        (self.wake)();
    }
    fn notify_animating_changed(&self, _: WebView, animating: bool) {
        self.animating.set(animating);
        (self.wake)();
    }
    fn notify_url_changed(&self, _: WebView, url: Url) {
        *self.requested_url.borrow_mut() = url.to_string();
        self.push(ServoEvent::AddressChanged {
            url: url.to_string(),
        });
    }
    fn notify_page_title_changed(&self, _: WebView, title: Option<String>) {
        self.push(ServoEvent::TitleChanged {
            title: title.unwrap_or_default(),
        });
    }
    fn notify_load_status_changed(&self, view: WebView, status: LoadStatus) {
        let url = self.requested_url.borrow().clone();
        self.push(match status {
            LoadStatus::Started => ServoEvent::NavigationStarted { url },
            LoadStatus::HeadParsed => ServoEvent::NavigationCommitted { url },
            LoadStatus::Complete => ServoEvent::NavigationFinished {
                url,
                title: view.page_title(),
            },
            LoadStatus::Stopped => ServoEvent::NavigationStopped { url },
        });
    }
    fn notify_crashed(&self, _: WebView, reason: String, _: Option<String>) {
        self.push(ServoEvent::NavigationFailed {
            url: self.requested_url.borrow().clone(),
            reason,
        });
    }
}
