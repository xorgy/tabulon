// Copyright 2024 the Vello Authors
// Copyright 2025 the Tabulon Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! DXF viewer

use masonry_winit::vello;
use vello::{kurbo, peniko};

use accesskit::{Node, Role};
use anyhow::Result;
use core::num::NonZeroU64;
use joto_constants::u64::{INCH, MICROMETER};
use kurbo::{
    Affine, DEFAULT_ACCURACY, ParamCurveNearest, PathSeg, Point, Rect, RoundedRectRadii, Shape,
    Size, Stroke, Vec2,
};
use masonry_winit::core::{
    AccessCtx, Action, BoxConstraints, EventCtx, LayoutCtx, PaintCtx, PointerButton, PointerEvent,
    PointerId, PointerInfo, PointerType, PointerUpdate, PropertiesMut, PropertiesRef, RegisterCtx,
    ScrollDelta, TextEvent, Update, UpdateCtx, Widget, WidgetId, WidgetMut, WidgetPod,
};
use masonry_winit::widgets::{CrossAxisAlignment, Flex, Label, SizedBox};
use peniko::{Brush, Color, Compose, Fill, color::palette};
use smallvec::SmallVec;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;
use tabulon_dxf::{EntityHandle, RestrokePaint, TDDrawing, dxf};
use tracing_subscriber::prelude::*;
use vello::Scene;

use tabulon::{
    GraphicsBag, GraphicsItem, ItemHandle, PaintHandle,
    render_layer::RenderLayer,
    shape::{FatPaint, FatShape},
};

extern crate alloc;
use alloc::collections::BTreeSet;

#[derive(Default)]
struct GestureState {
    /// Pointer currently panning.
    pan: Option<PointerId>,
    /// Cursor position.
    cursor_pos: Point,
    /// Pan start position.
    pan_start: Option<Point>,
}

struct DrawingViewer {
    /// `tabulon_dxf` drawing.
    td: TDDrawing,

    /// Index of bounding boxes for hit testing.
    picking_index: EntityIndex,
    /// Which shape is closest to the cursor?
    pick: Option<EntityHandle>,

    /// Index of bounding boxes for culling texts.
    text_cull_index: TextCullIndex,

    /// View transform of the drawing.
    view_transform: Affine,
    /// View scale of the drawing.
    view_scale: f64,

    /// State of gesture processing (e.g. panning, zooming).
    gestures: GestureState,
}

pub struct TabulonDxfViewer {
    /// A vello Scene which is a data structure which allows one to build up a description a scene to be
    /// drawn (with paths, fills, images, text, etc) which is then passed to a renderer for rendering.
    scene: Scene,

    /// Tabulon Vello environment.
    tv_environment: tabulon_vello::Environment,

    /// State related to viewing a specific drawing.
    viewer: Option<DrawingViewer>,

    /// Status label.
    status_label: WidgetPod<SizedBox>,

    /// The info panels.
    info_panels: WidgetPod<Flex>,
}

impl Widget for TabulonDxfViewer {
    fn on_pointer_event(
        &mut self,
        ctx: &mut EventCtx,
        _props: &mut PropertiesMut<'_>,
        event: &PointerEvent,
    ) {
        let Some(viewer) = &mut self.viewer else {
            return;
        };

        let mut reproject = false;

        match event {
            PointerEvent::Down {
                pointer:
                    PointerInfo {
                        pointer_id,
                        pointer_type: PointerType::Mouse,
                        ..
                    },
                button: Some(PointerButton::Primary),
                state,
            }
            | PointerEvent::Down {
                pointer:
                    PointerInfo {
                        pointer_id,
                        pointer_type: PointerType::Touch,
                        ..
                    },
                state,
                ..
            } => {
                if viewer.gestures.pan.is_none() {
                    viewer.gestures.pan = *pointer_id;
                    let pt = (ctx.local_position(state.position).to_vec2()
                        * ctx.get_scale_factor())
                    .to_point();
                    viewer.gestures.cursor_pos = pt;
                    viewer.gestures.pan_start = Some(pt);
                    ctx.capture_pointer();
                    ctx.set_handled();
                }
            }
            PointerEvent::Move(PointerUpdate {
                pointer: PointerInfo { pointer_id, .. },
                current,
                ..
            }) => {
                let p = (ctx.local_position(current.position).to_vec2() * ctx.get_scale_factor())
                    .to_point();

                let dp = viewer.view_transform.inverse() * p;

                if viewer.gestures.pan == *pointer_id {
                    viewer.view_transform = viewer
                        .view_transform
                        .then_translate(-(viewer.gestures.cursor_pos - p));
                    reproject = true;
                    ctx.set_handled();
                } else if *pointer_id == Some(PointerId::PRIMARY) {
                    let pick_dist: f64 = ctx.get_scale_factor() * 4.242;
                    let pick_started = Instant::now();

                    let pick = viewer
                        .picking_index
                        .pick(dp, pick_dist * viewer.view_scale.recip());

                    if viewer.pick != pick {
                        if let Some(pick) = pick {
                            let pick_duration =
                                Instant::now().saturating_duration_since(pick_started);
                            let entityinfo = match &viewer.td.info.get_entity(pick).specific {
                                dxf::entities::EntityType::Insert(i) => i.name.clone(),
                                e => format!("{:?}", e),
                            };

                            ctx.mutate_later(&mut self.status_label, |mut w| {
                                if let Some(mut l) = SizedBox::child_mut(&mut w) {
                                    Label::set_text(&mut l.downcast(), entityinfo)
                                }
                            });
                            eprintln!("Pick took {pick_duration:?}");
                        } else {
                            ctx.mutate_later(&mut self.status_label, |mut w| {
                                if let Some(mut l) = SizedBox::child_mut(&mut w) {
                                    Label::set_text(&mut l.downcast(), "")
                                }
                            });
                        }
                        viewer.pick = pick;
                        reproject = true;
                    }
                    ctx.set_handled();
                }

                viewer.gestures.cursor_pos = p;
            }
            PointerEvent::Up {
                pointer: PointerInfo { pointer_id, .. },
                ..
            } => {
                if viewer.gestures.pan == *pointer_id {
                    viewer.gestures.pan = None;
                    if let Some(pt) = viewer.gestures.pan_start {
                        if pt.distance(viewer.gestures.cursor_pos) < 4. {
                            if let Some(pick) = viewer.pick {
                                let e = EntityInfo::from_entity(viewer.td.info.get_entity(pick));
                                ctx.mutate_later(&mut self.info_panels, move |mut w| {
                                    Flex::add_child(
                                        &mut w.downcast::<Flex>(),
                                        EntityInfoPanel::from_info(e),
                                    );
                                });
                            }
                        }
                    }
                    ctx.set_handled();
                }
            }
            PointerEvent::Leave(PointerInfo { pointer_id, .. })
            | PointerEvent::Cancel(PointerInfo { pointer_id, .. }) => {
                if viewer.gestures.pan == *pointer_id {
                    viewer.gestures.pan = None;
                    viewer.gestures.pan_start = None;
                    ctx.set_handled();
                }
            }
            PointerEvent::Scroll { delta, .. } => {
                let d = match delta {
                    ScrollDelta::LineDelta(_, y) => *y as f64 * 0.1,
                    ScrollDelta::PixelDelta(pd) => pd.y * 0.05,
                    _ => 0.,
                };

                viewer.view_transform = viewer
                    .view_transform
                    .then_scale_about(1. + d, viewer.gestures.cursor_pos);
                viewer.view_scale *= 1. + d;
                reproject = true;
                ctx.set_handled();
            }
            _ => {}
        }

        if reproject {
            ctx.request_render();
        }
    }

    fn on_text_event(
        &mut self,
        _ctx: &mut EventCtx,
        _props: &mut PropertiesMut<'_>,
        _event: &TextEvent,
    ) {
    }

    fn register_children(&mut self, ctx: &mut RegisterCtx) {
        ctx.register_child(&mut self.status_label);
        ctx.register_child(&mut self.info_panels);
    }

    fn layout(
        &mut self,
        ctx: &mut LayoutCtx,
        _props: &mut PropertiesMut<'_>,
        bc: &BoxConstraints,
    ) -> Size {
        let max = if bc.is_width_bounded() && bc.is_height_bounded() {
            bc.max()
        } else {
            let size = Size::new(512.0, 512.0);
            bc.constrain(size)
        };

        let label_size = ctx.run_layout(&mut self.status_label, &bc.loosen());

        let bottom = Point {
            x: 0.,
            y: max.height - label_size.height,
        };

        ctx.place_child(&mut self.status_label, bottom);

        ctx.run_layout(&mut self.info_panels, &bc.loosen());
        ctx.place_child(&mut self.info_panels, Point::ZERO);

        max
    }

    fn paint(&mut self, ctx: &mut PaintCtx, _props: &PropertiesRef<'_>, scene: &mut Scene) {
        // Clear the whole widget with the color of your choice
        // (ctx.size() returns the size of the layout rect we're painting in)
        // Note: ctx also has a `clear` method, but that clears the whole context,
        // and we only want to clear this widget's area.
        let size = ctx.size();
        let rect = size.to_rect();

        tracing::info_span!("reproject").in_scope(|| {
            let Some(viewer) = &mut self.viewer else {
                return;
            };
            let reproject_started = Instant::now();
            update_transform(
                &mut viewer.td.graphics,
                viewer.td.restroke_paints.clone(),
                viewer.view_transform,
                viewer.view_scale,
                ctx.get_scale_factor(),
            );

            let tl = viewer.view_transform.inverse() * Point { x: 0., y: 0. };
            let br = viewer.view_transform.inverse()
                * Point {
                    x: ctx.size().width * ctx.get_scale_factor(),
                    y: ctx.size().height * ctx.get_scale_factor(),
                };

            #[allow(
                clippy::cast_possible_truncation,
                reason = "The loss of range and precision is acceptable."
            )]
            let visible = viewer.picking_index.query_items(
                tl.x as f32,
                tl.y as f32,
                br.x as f32,
                br.y as f32,
            );

            #[allow(
                clippy::cast_possible_truncation,
                reason = "The loss of range and precision is acceptable."
            )]
            let visible_text = viewer.text_cull_index.query_items(
                tl.x as f32,
                tl.y as f32,
                br.x as f32,
                br.y as f32,
            );

            let culled_render_layer =
                viewer
                    .td
                    .render_layer
                    .filter(|ih| match viewer.td.graphics.get(*ih) {
                        Some(GraphicsItem::FatShape(..)) => visible.binary_search(ih).is_ok(),
                        Some(GraphicsItem::FatText(..)) => visible_text.contains(ih),
                        _ => false,
                    });
            self.scene.reset();
            self.tv_environment.add_render_layer_to_scene(
                &mut self.scene,
                &viewer.td.graphics,
                &culled_render_layer,
            );

            if let Some(pick) = viewer.pick {
                let mut gb = GraphicsBag::default();
                let mut rl = RenderLayer::default();

                gb.update_transform(Default::default(), viewer.view_transform);

                let paint = gb.register_paint(FatPaint {
                    stroke: Stroke::new(1.414 / viewer.view_scale),
                    stroke_paint: Some(palette::css::GOLDENROD.into()),
                    fill_paint: None,
                });

                culled_render_layer
                    .indices
                    .iter()
                    .filter(|ih| viewer.td.item_entity_map[ih] == pick)
                    .for_each(|ih| {
                        let Some(GraphicsItem::FatShape(FatShape {
                            transform, path, ..
                        })) = viewer.td.graphics.get(*ih)
                        else {
                            return;
                        };
                        rl.push_with_bag(
                            &mut gb,
                            FatShape {
                                transform: *transform,
                                path: path.clone(),
                                paint,
                            },
                        );
                    });

                self.tv_environment
                    .add_render_layer_to_scene(&mut self.scene, &gb, &rl);
            }

            let reproject_duration = Instant::now().saturating_duration_since(reproject_started);
            eprintln!("Reprojection/reencoding took {reproject_duration:?}");
        });

        scene.push_layer(Compose::SrcOver, 1.0, Affine::IDENTITY, &rect);
        scene.fill(
            Fill::NonZero,
            Affine::IDENTITY,
            palette::css::WHITE,
            None,
            &rect,
        );
        scene.append(
            &self.scene,
            Some(Affine::scale(1.0 / ctx.get_scale_factor())),
        );
        scene.pop_layer();
    }

    fn children_ids(&self) -> SmallVec<[WidgetId; 16]> {
        SmallVec::from_slice(&[self.status_label.id(), self.info_panels.id()])
    }

    fn accessibility_role(&self) -> Role {
        Role::Canvas
    }

    fn accessibility(&mut self, _ctx: &mut AccessCtx, _props: &PropertiesRef<'_>, node: &mut Node) {
        node.set_label(format!(
            "This is a DXF Viewer, it has no useful accessibility information at the moment."
        ));
    }
}

/// Update the transform/scale in all the items in a `GraphicsBag`.
///
/// This also adapts line widths from the drawing so they are the correct
/// size after scaling.
#[tracing::instrument(skip_all)]
fn update_transform(
    graphics: &mut GraphicsBag,
    restroke_paints: Arc<[RestrokePaint]>,
    transform: Affine,
    view_scale: f64,
    scale_factor: f64,
) {
    // Update root transform.
    graphics.update_transform(Default::default(), transform);

    // Update default stroke.
    graphics.update_paint(
        Default::default(),
        FatPaint {
            // Unfortunately, post-transform stroke widths are not supported.
            stroke: Stroke::new(1.0 / view_scale),
            stroke_paint: Some(Color::BLACK.into()),
            fill_paint: None,
        },
    );

    #[allow(clippy::cast_possible_truncation, reason = "Deliberate truncation.")]
    let pixel_pitch = INCH / (96_f64 * scale_factor).trunc() as u64;

    for r in restroke_paints.iter() {
        r.adapt(graphics, pixel_pitch, view_scale, 1.0, f64::INFINITY);
    }
}

/// Light adapt paints.
///
/// The ACI palette and drawings using it assume a black background,
/// this adapts colors to have a reasonable degree of contrast for the
/// time being, until a more permanent solution is found.
fn light_adapt_paints(graphics: &mut GraphicsBag, render_layer: &RenderLayer) {
    let paint_handles: BTreeSet<PaintHandle> = render_layer
        .indices
        .iter()
        .flat_map(|ih| {
            graphics.get(*ih).map(|i| match i {
                GraphicsItem::FatShape(s) => s.paint,
                GraphicsItem::FatText(t) => t.paint,
            })
        })
        .collect();

    for handle in paint_handles {
        let p = graphics.get_paint_mut(handle);
        if let Some(Brush::Solid(c)) = p.stroke_paint {
            p.stroke_paint = Some(Brush::Solid(c.map_lightness(|x| 1.2 - x)));
        }
        if let Some(Brush::Solid(c)) = p.fill_paint {
            p.fill_paint = Some(Brush::Solid(c.map_lightness(|x| 1.2 - x)));
        }
    }
}

use static_aabb2d_index::{StaticAABB2DIndex, StaticAABB2DIndexBuilder};

/// Bounding box index for entities.
struct EntityIndex {
    bounds_index: StaticAABB2DIndex<f32>,
    lines: Box<[PathSeg]>,
    entity_mapping: Box<[EntityHandle]>,
    item_mapping: Box<[ItemHandle]>,
}

#[allow(
    clippy::cast_possible_truncation,
    reason = "The loss of range and precision is acceptable."
)]
impl EntityIndex {
    fn new(d: &TDDrawing) -> Self {
        let build_started = Instant::now();

        let mut lines: Vec<PathSeg> = vec![];
        let mut entity_mapping = vec![];
        let mut item_mapping = vec![];
        for (k, v) in d.item_entity_map.iter() {
            let Some(GraphicsItem::FatShape(FatShape { path, .. })) = d.graphics.get(*k) else {
                continue;
            };

            for seg in path.segments() {
                entity_mapping.push(*v);
                item_mapping.push(*k);
                lines.push(seg);
            }
        }
        let lines = Box::from(lines.as_slice());
        let entity_mapping = Box::from(entity_mapping.as_slice());
        let item_mapping = Box::from(item_mapping.as_slice());

        let bounds_index = compute_bounds_index(&lines);

        let build_duration = Instant::now().saturating_duration_since(build_started);
        eprintln!("Bounds index took {build_duration:?} to build.");

        Self {
            bounds_index,
            lines,
            entity_mapping,
            item_mapping,
        }
    }

    /// Pick entity that is closest to dp.
    #[tracing::instrument(skip_all)]
    fn pick(&self, dp: Point, sp: f64) -> Option<EntityHandle> {
        self.bounds_index
            .query(
                (dp.x - sp) as f32,
                (dp.y - sp) as f32,
                (dp.x + sp) as f32,
                (dp.y + sp) as f32,
            )
            .into_iter()
            .fold((f64::INFINITY, None), |(dsq, i), b| {
                let ndsq = self.lines[b].nearest(dp, DEFAULT_ACCURACY).distance_sq;
                if ndsq < dsq && ndsq < (sp * sp) {
                    (ndsq, Some(b))
                } else {
                    (dsq, i)
                }
            })
            .1
            .map(|i| self.entity_mapping[i])
    }

    /// Query which entities' geometry overlaps with the bounds.
    #[tracing::instrument(skip_all)]
    fn query_items(&self, left: f32, top: f32, right: f32, bottom: f32) -> Vec<ItemHandle> {
        let mut is: Vec<ItemHandle> = vec![];
        for ih in self
            .bounds_index
            .query(left, top, right, bottom)
            .iter()
            .map(|&i| self.item_mapping[i])
        {
            if let Err(i) = is.binary_search(&ih) {
                is.insert(i, ih);
            }
        }
        is
    }

    fn bounds(&self) -> Rect {
        self.bounds_index
            .bounds()
            .map_or(Rect::default(), |b| Rect {
                x0: b.min_x as f64,
                y0: b.min_y as f64,
                x1: b.max_x as f64,
                y1: b.max_y as f64,
            })
    }
}

/// Compute an index of bounding boxes for shapes.
#[allow(
    clippy::cast_possible_truncation,
    reason = "The loss of range and precision is acceptable."
)]
#[tracing::instrument(skip_all)]
fn compute_bounds_index(lines: &[PathSeg]) -> StaticAABB2DIndex<f32> {
    let mut builder = StaticAABB2DIndexBuilder::<f32>::new(lines.len());
    for shape in lines.iter() {
        let bbox = Shape::bounding_box(&shape);
        builder.add(
            bbox.min_x() as f32,
            bbox.min_y() as f32,
            bbox.max_x() as f32,
            bbox.max_y() as f32,
        );
    }
    builder.build().unwrap()
}

/// Index for culling text items.
struct TextCullIndex {
    bounds_index: StaticAABB2DIndex<f32>,
    item_mapping: Box<[ItemHandle]>,
}

#[allow(
    clippy::cast_possible_truncation,
    reason = "The loss of range and precision is acceptable."
)]
impl TextCullIndex {
    fn new(tv_env: &mut tabulon_vello::Environment, d: &TDDrawing) -> Self {
        let measurements = tv_env.measure_text_items(&d.graphics, &d.render_layer);
        let mut builder = StaticAABB2DIndexBuilder::<f32>::new(measurements.len());
        let mut item_mapping = vec![];

        for (ih, (di, s)) in measurements {
            item_mapping.push(ih);
            let bbox = (Affine::from(di)
                * Rect::from_origin_size(Point::ZERO, s).to_path(DEFAULT_ACCURACY))
            .bounding_box();
            builder.add(
                bbox.min_x() as f32,
                bbox.min_y() as f32,
                bbox.max_x() as f32,
                bbox.max_y() as f32,
            );
        }

        Self {
            bounds_index: builder.build().unwrap(),
            item_mapping: item_mapping.into(),
        }
    }

    /// Query which text layouts overlap with the bounds.
    #[tracing::instrument(skip_all)]
    fn query_items(&self, left: f32, top: f32, right: f32, bottom: f32) -> BTreeSet<ItemHandle> {
        self.bounds_index
            .query(left, top, right, bottom)
            .iter()
            .map(|&l| self.item_mapping[l])
            .collect()
    }
}

/// Load a drawing file into a drawing, and print some stats.
fn load_drawing(p: impl AsRef<Path>) -> Result<TDDrawing> {
    let drawing_load_started = Instant::now();
    let mut drawing = tabulon_dxf::load_file_default_layers(p)?;

    let drawing_load_duration = Instant::now().saturating_duration_since(drawing_load_started);
    eprintln!("Drawing took {drawing_load_duration:?} to load and translate.");

    light_adapt_paints(&mut drawing.graphics, &drawing.render_layer);

    {
        let mut segment_count = 0;
        let mut text_count = 0;
        for item_handle in drawing.item_entity_map.keys() {
            match drawing.graphics.get(*item_handle) {
                Some(GraphicsItem::FatShape(FatShape { path, .. })) => {
                    segment_count += path.segments().count();
                }
                Some(GraphicsItem::FatText(_)) => text_count += 1,
                None => {}
            }
        }
        eprintln!(
            "Loaded {} unique entities, {} path segments, {} text blocks.",
            drawing.item_entity_map.len(),
            segment_count,
            text_count
        );
        let linewidths: BTreeSet<u64> = drawing.restroke_paints.iter().map(|r| r.weight).collect();
        eprintln!(
            "There are {} unique linewidths, between {} µm and {} µm.",
            linewidths.len(),
            linewidths.first().unwrap() / MICROMETER,
            linewidths.last().unwrap() / MICROMETER,
        );
    }

    Ok(drawing)
}

impl TabulonDxfViewer {
    /// Construct a [`TabulonDxfViewer`] loading the drawing synchronously.
    pub fn from_path_sync(p: impl Into<PathBuf>, scale_factor: f64) -> Self {
        let mut scene = Scene::new();
        let mut tv_environment = tabulon_vello::Environment::default();
        let viewer = load_drawing(p.into()).ok().map(|mut drawing| {
            let picking_index = EntityIndex::new(&drawing);
            let bounds = picking_index.bounds();

            let text_cull_index = TextCullIndex::new(&mut tv_environment, &drawing);

            let view_scale = (512.0 / bounds.size().height).min(512.0 / bounds.size().width);

            let view_transform = Affine::translate(Vec2 {
                x: -bounds.min_x(),
                y: -bounds.min_y(),
            })
            .then_scale(view_scale);
            update_transform(
                &mut drawing.graphics,
                drawing.restroke_paints.clone(),
                view_transform,
                view_scale,
                scale_factor,
            );
            scene.reset();

            let encode_started = Instant::now();
            tv_environment.add_render_layer_to_scene(
                &mut scene,
                &drawing.graphics,
                &drawing.render_layer,
            );
            let encode_duration = Instant::now().saturating_duration_since(encode_started);
            eprintln!("Initial projection/encode took {encode_duration:?}");
            DrawingViewer {
                td: drawing,
                picking_index,
                view_scale,
                view_transform,
                text_cull_index,
                gestures: GestureState::default(),
                pick: None,
            }
        });

        Self {
            scene,
            tv_environment,
            viewer,
            status_label: WidgetPod::new(
                SizedBox::new(Label::new("").with_brush(Color::BLACK))
                    .background(palette::css::ALICE_BLUE)
                    .padding(4.)
                    .rounded(RoundedRectRadii {
                        top_right: 3.,
                        ..RoundedRectRadii::default()
                    }),
            ),
            info_panels: WidgetPod::new(
                Flex::column().cross_axis_alignment(CrossAxisAlignment::Start),
            ),
        }
    }

    pub fn set_pick(this: &mut WidgetMut<'_, Self>, eh: EntityHandle) {
        let Some(viewer) = &mut this.widget.viewer else {
            return;
        };
        viewer.pick = Some(eh);
    }

    pub fn clear_pick(this: &mut WidgetMut<'_, Self>) {
        let Some(viewer) = &mut this.widget.viewer else {
            return;
        };
        viewer.pick = None;
    }
}

/// Cloneable self-contained information about an [`Entity`]
#[derive(Clone)]
pub struct EntityInfo {
    pub name: String,
    pub handle: EntityHandle,
}

impl EntityInfo {
    fn from_entity(e: &dxf::entities::Entity) -> Self {
        let name = match &e.specific {
            dxf::entities::EntityType::Insert(i) => i.name.clone(),
            e => format!("unhandled {:?}", e),
        };
        let handle = (&e.common).try_into().unwrap();
        Self { name, handle }
    }
}

pub struct EntityInfoPanel {
    /// Panel is expanded.
    expanded: bool,
    /// Name label.
    name_label: WidgetPod<SizedBox>,
    /// Entity handle.
    pub info: EntityInfo,
}

impl EntityInfoPanel {
    /// Convert an [`EntityInfo`] into an [`EntityInfoPanel`].
    fn from_info(e: EntityInfo) -> Self {
        Self {
            expanded: false,
            info: e.clone(),
            name_label: WidgetPod::new(
                SizedBox::new(
                    Flex::row()
                        .with_child(Label::new(e.name.as_str()).with_brush(Color::BLACK))
                        .with_child(Label::new("❌︎").with_brush(Color::BLACK))
                        .cross_axis_alignment(CrossAxisAlignment::Fill),
                )
                .background(palette::css::ALICE_BLUE)
                .padding(4.)
                .rounded(RoundedRectRadii {
                    top_left: 3.,
                    top_right: 3.,
                    ..RoundedRectRadii::default()
                })
                .border(palette::css::LEMON_CHIFFON, 1.),
            ),
        }
    }
}

impl Widget for EntityInfoPanel {
    fn on_pointer_event(
        &mut self,
        ctx: &mut EventCtx,
        _props: &mut PropertiesMut<'_>,
        event: &PointerEvent,
    ) {
        match event {
            PointerEvent::Down { .. } => {
                ctx.submit_action(Action::Other(Box::new(EntityInfoPanelAction::Close)));
                ctx.capture_pointer();
            }
            _ => {}
        }
        ctx.set_handled();
    }

    fn update(&mut self, ctx: &mut UpdateCtx, _props: &mut PropertiesMut, update: &Update) {
        match update {
            Update::HoveredChanged(h) | Update::ChildHoveredChanged(h) => {
                if *h {
                    ctx.submit_action(Action::Other(Box::new(EntityInfoPanelAction::Hover(
                        self.info.handle,
                    ))));
                } else {
                    ctx.submit_action(Action::Other(Box::new(EntityInfoPanelAction::HoverEnd)));
                }
                let h = *h;
                ctx.mutate_later(&mut self.name_label, move |mut w| {
                    SizedBox::set_border(
                        &mut w.downcast(),
                        if h {
                            palette::css::GOLDENROD
                        } else {
                            palette::css::LEMON_CHIFFON
                        },
                        1.,
                    );
                })
            }
            _ => {}
        }
    }

    fn register_children(&mut self, ctx: &mut RegisterCtx) {
        ctx.register_child(&mut self.name_label);
    }

    fn layout(
        &mut self,
        ctx: &mut LayoutCtx,
        _props: &mut PropertiesMut<'_>,
        bc: &BoxConstraints,
    ) -> Size {
        ctx.run_layout(&mut self.name_label, &bc.loosen())
    }

    fn paint(&mut self, ctx: &mut PaintCtx, _props: &PropertiesRef<'_>, scene: &mut Scene) {}

    fn accessibility_role(&self) -> Role {
        Role::Details
    }

    fn accessibility(
        &mut self,
        _ctx: &mut AccessCtx,
        _props: &PropertiesRef<'_>,
        _node: &mut Node,
    ) {
    }

    fn children_ids(&self) -> SmallVec<[WidgetId; 16]> {
        SmallVec::from_slice(&[self.name_label.id()])
    }
}

#[derive(Debug, Clone)]
pub enum EntityInfoPanelAction {
    Close,
    Hover(EntityHandle),
    HoverEnd,
}

struct StatusPanel {
    lebox: WidgetPod<SizedBox>,
}

impl Widget for StatusPanel {
    fn on_pointer_event(
        &mut self,
        ctx: &mut EventCtx,
        _props: &mut PropertiesMut<'_>,
        event: &PointerEvent,
    ) {
        ctx.set_handled();
    }

    fn register_children(&mut self, ctx: &mut RegisterCtx) {
        ctx.register_child(&mut self.lebox);
    }

    fn layout(
        &mut self,
        ctx: &mut LayoutCtx,
        _props: &mut PropertiesMut<'_>,
        bc: &BoxConstraints,
    ) -> Size {
        ctx.run_layout(&mut self.lebox, &bc.loosen())
    }

    fn paint(&mut self, ctx: &mut PaintCtx, _props: &PropertiesRef<'_>, scene: &mut Scene) {}

    fn accessibility_role(&self) -> Role {
        Role::Status
    }

    fn accessibility(
        &mut self,
        _ctx: &mut AccessCtx,
        _props: &PropertiesRef<'_>,
        _node: &mut Node,
    ) {
    }

    fn children_ids(&self) -> SmallVec<[WidgetId; 16]> {
        SmallVec::from_slice(&[self.lebox.id()])
    }
}

impl StatusPanel {
    fn new() -> Self {
        Self {
            lebox: WidgetPod::new(
                SizedBox::new(
                    Flex::row()
                        .with_child(Label::new("Le status left"))
                        .with_child(Label::new("le status right."))
                        .cross_axis_alignment(CrossAxisAlignment::Fill),
                )
                .background(palette::css::ALICE_BLUE)
                .padding(4.),
            ),
        }
    }
}
