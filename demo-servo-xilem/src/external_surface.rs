// Copyright 2026 Mark Alan Boykin
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
// SPDX-License-Identifier: MPL-2.0

//! A widget that takes the space it is given and paints as a Masonry external
//! layer. Masonry records only its bounds; the vendored `masonry_winit` copies
//! the registered texture (Servo's frame) into that rectangle each redraw.
//! It repaints every frame, which also drives the window's redraws.

use masonry::accesskit::{Node, Role};
use masonry::core::{
    AccessCtx, ChildrenIds, LayoutCtx, MeasureCtx, NoAction, PaintCtx, PaintLayerMode,
    PropertiesMut, PropertiesRef, RegisterCtx, Update, UpdateCtx, Widget, WidgetId,
};
use masonry::imaging::Painter;
use masonry::kurbo::{Axis, Size};
use masonry::layout::{LenReq, Length};
use tracing::{Span, trace_span};
use xilem::core::{MessageCtx, MessageResult, Mut, View, ViewMarker};
use xilem::{Pod, ViewCtx};

/// The Masonry widget.
pub struct ExternalSurface;

impl Widget for ExternalSurface {
    type Action = NoAction;

    fn register_children(&mut self, _ctx: &mut RegisterCtx<'_>) {}

    // Masonry resets a widget's layer mode at the start of every paint pass
    // and keeps an unrepainted widget out of the next plan, so the surface
    // repaints every frame. The page behind it is live content anyway.
    fn update(&mut self, ctx: &mut UpdateCtx<'_>, _props: &mut PropertiesMut<'_>, event: &Update) {
        if matches!(event, Update::WidgetAdded) {
            ctx.request_anim_frame();
        }
    }

    fn on_anim_frame(
        &mut self,
        ctx: &mut UpdateCtx<'_>,
        _props: &mut PropertiesMut<'_>,
        _interval: u64,
    ) {
        ctx.request_anim_frame();
        ctx.request_paint_only();
    }

    fn measure(
        &mut self,
        _ctx: &mut MeasureCtx<'_>,
        _props: &PropertiesRef<'_>,
        _axis: Axis,
        len_req: LenReq,
        _cross_length: Option<Length>,
    ) -> Length {
        match len_req {
            LenReq::MinContent | LenReq::MaxContent => Length::ZERO,
            LenReq::FitContent(space) => space,
        }
    }

    fn layout(&mut self, ctx: &mut LayoutCtx<'_>, _props: &PropertiesRef<'_>, size: Size) {
        ctx.set_clip_path(size.to_rect());
    }

    fn paint(
        &mut self,
        ctx: &mut PaintCtx<'_>,
        _props: &PropertiesRef<'_>,
        _painter: &mut Painter<'_>,
    ) {
        ctx.set_paint_layer_mode(PaintLayerMode::External);
    }

    fn accessibility_role(&self) -> Role {
        Role::Document
    }

    fn accessibility(
        &mut self,
        _ctx: &mut AccessCtx<'_>,
        _props: &PropertiesRef<'_>,
        _node: &mut Node,
    ) {
    }

    fn children_ids(&self) -> ChildrenIds {
        ChildrenIds::new()
    }

    fn make_trace_span(&self, id: WidgetId) -> Span {
        trace_span!("ExternalSurface", id = id.trace())
    }
}

/// The Xilem view for [`ExternalSurface`].
pub fn external_surface() -> ExternalSurfaceView {
    ExternalSurfaceView
}

#[must_use = "View values do nothing unless provided to Xilem."]
pub struct ExternalSurfaceView;

impl ViewMarker for ExternalSurfaceView {}

impl<State: 'static, Action> View<State, Action, ViewCtx> for ExternalSurfaceView {
    type Element = Pod<ExternalSurface>;
    type ViewState = ();

    fn build(&self, ctx: &mut ViewCtx, _: &mut State) -> (Self::Element, Self::ViewState) {
        (ctx.create_pod(ExternalSurface), ())
    }

    fn rebuild(
        &self,
        _: &Self,
        (): &mut Self::ViewState,
        _: &mut ViewCtx,
        _: Mut<'_, Self::Element>,
        _: &mut State,
    ) {
    }

    fn teardown(&self, (): &mut Self::ViewState, _: &mut ViewCtx, _: Mut<'_, Self::Element>) {}

    fn message(
        &self,
        (): &mut Self::ViewState,
        _message: &mut MessageCtx,
        _element: Mut<'_, Self::Element>,
        _: &mut State,
    ) -> MessageResult<Action> {
        MessageResult::Stale
    }
}
