// Imports
use super::{ModifyState, ResizeCorner, Selector, SelectorState};
use crate::engine::EngineViewMut;
use crate::pens::pensconfig::selectorconfig::SelectorStyle;
use crate::snap::SnapCorner;
use crate::{DrawableOnDoc, WidgetFlags};
use p2d::bounding_volume::Aabb;
use p2d::query::PointQuery;
use rnote_compose::eventresult::{EventPropagation, EventResult};
use rnote_compose::ext::{AabbExt, Vector2Ext};
use rnote_compose::penevent::{KeyboardKey, ModifierKey, PenProgress};
use rnote_compose::penpath::Element;
use std::time::Instant;

impl Selector {
    pub(super) fn handle_pen_event_down(
        &mut self,
        element: Element,
        modifier_keys: Vec<ModifierKey>,
        _now: Instant,
        engine_view: &mut EngineViewMut,
    ) -> (EventResult<PenProgress>, WidgetFlags) {
        let mut widget_flags = WidgetFlags::default();

        let event_result = match &mut self.state {
            SelectorState::Idle => {
                // Deselect on start
                let selection_keys = engine_view.store.selection_keys_as_rendered();
                if !selection_keys.is_empty() {
                    engine_view.store.set_selected_keys(&selection_keys, false);
                    widget_flags.store_modified = true;
                }

                self.state = SelectorState::Selecting {
                    path: vec![element],
                };

                EventResult {
                    handled: true,
                    propagate: EventPropagation::Stop,
                    progress: PenProgress::InProgress,
                }
            }
            SelectorState::Selecting { path } => {
                Self::add_to_select_path(
                    engine_view.pens_config.selector_config.style,
                    path,
                    element,
                );
                // possibly nudge camera
                widget_flags |= engine_view
                    .camera
                    .nudge_w_pos(element.pos, engine_view.document);
                widget_flags |= engine_view.document.expand_autoexpand(engine_view.camera);
                engine_view.store.regenerate_rendering_in_viewport_threaded(
                    engine_view.tasks_tx.clone(),
                    false,
                    engine_view.camera.viewport(),
                    engine_view.camera.image_scale(),
                );

                EventResult {
                    handled: true,
                    propagate: EventPropagation::Stop,
                    progress: PenProgress::InProgress,
                }
            }
            SelectorState::ModifySelection {
                modify_state,
                selection,
                selection_bounds,
            } => {
                let mut progress = PenProgress::InProgress;

                match modify_state {
                    ModifyState::Up | ModifyState::Hover(_) => {
                        tracing::debug!("up or hover state for modify");
                        // ok, will be the time to reset the cached size
                        // now this cached size should be set when elements (strokes) get selected

                        // If we click on another, not-already selected stroke while in separate style or
                        // while pressing Shift, we add it to the selection
                        let key_to_add = engine_view
                            .store
                            .stroke_hitboxes_contain_coord(
                                engine_view.camera.viewport(),
                                element.pos,
                            )
                            .pop();

                        if (engine_view.pens_config.selector_config.style == SelectorStyle::Single
                            || modifier_keys.contains(&ModifierKey::KeyboardShift))
                            && key_to_add
                                .and_then(|key| engine_view.store.selected(key).map(|s| !s))
                                .unwrap_or(false)
                        {
                            let key_to_add = key_to_add.unwrap();
                            // special case here ? what about a resize of 2 elements and then we add a third in the mix
                            // we need the ref to be added like the rest as well for the resize to work
                            tracing::debug!("adding an element to the selection with shift");

                            // we change the size of the selection, we need to update the ghost stroke to the real stroke width
                            // in order not to have any issue here
                            engine_view.store.copy_ghost_stroke_width(selection);

                            engine_view.store.set_selected(key_to_add, true);
                            selection.push(key_to_add);
                            if let Some(new_bounds) =
                                engine_view.store.bounds_for_strokes(selection)
                            {
                                *selection_bounds = new_bounds;
                            }
                        } else if Self::rotate_node_sphere(*selection_bounds, engine_view.camera)
                            .contains_local_point(&element.pos.into())
                        {
                            // clicking on the rotate node
                            let rotation_angle = {
                                let vec = element.pos - selection_bounds.center().coords;
                                na::Vector2::x().angle_ahead(&vec)
                            };

                            *modify_state = ModifyState::Rotate {
                                rotation_center: selection_bounds.center(),
                                start_rotation_angle: rotation_angle,
                                current_rotation_angle: rotation_angle,
                            };
                            // clicking on one of the resize nodes at the corners
                        } else if Self::resize_node_bounds(
                            ResizeCorner::TopLeft,
                            *selection_bounds,
                            engine_view.camera,
                        )
                        .contains_local_point(&element.pos.into())
                        {
                            *modify_state = ModifyState::Resize {
                                from_corner: ResizeCorner::TopLeft,
                                start_bounds: *selection_bounds,
                                start_pos: element.pos,
                            }
                        } else if Self::resize_node_bounds(
                            ResizeCorner::TopRight,
                            *selection_bounds,
                            engine_view.camera,
                        )
                        .contains_local_point(&element.pos.into())
                        {
                            *modify_state = ModifyState::Resize {
                                from_corner: ResizeCorner::TopRight,
                                start_bounds: *selection_bounds,
                                start_pos: element.pos,
                            }
                        } else if Self::resize_node_bounds(
                            ResizeCorner::BottomLeft,
                            *selection_bounds,
                            engine_view.camera,
                        )
                        .contains_local_point(&element.pos.into())
                        {
                            *modify_state = ModifyState::Resize {
                                from_corner: ResizeCorner::BottomLeft,
                                start_bounds: *selection_bounds,
                                start_pos: element.pos,
                            }
                        } else if Self::resize_node_bounds(
                            ResizeCorner::BottomRight,
                            *selection_bounds,
                            engine_view.camera,
                        )
                        .contains_local_point(&element.pos.into())
                        {
                            *modify_state = ModifyState::Resize {
                                from_corner: ResizeCorner::BottomRight,
                                start_bounds: *selection_bounds,
                                start_pos: element.pos,
                            }
                        } else if selection_bounds.contains_local_point(&element.pos.into()) {
                            let snap_corner =
                                SnapCorner::determine_from_bounds(*selection_bounds, element.pos);

                            // clicking inside the selection bounds, triggering translation
                            *modify_state = ModifyState::Translate {
                                start_pos: element.pos,
                                current_pos: element.pos,
                                snap_corner,
                            };
                        } else {
                            // when clicking outside the selection bounds, reset
                            // where we deselect from our resize
                            tracing::debug!("resizing selection cancelled");

                            // copy ghost sizes
                            engine_view.store.copy_ghost_stroke_width(selection);

                            engine_view.store.set_selected_keys(selection, false);
                            self.state = SelectorState::Idle;

                            progress = PenProgress::Finished;
                        }
                    }
                    ModifyState::Translate {
                        start_pos: _,
                        current_pos,
                        snap_corner,
                    } => {
                        let snap_corner_pos = match snap_corner {
                            SnapCorner::TopLeft => selection_bounds.mins.coords,
                            SnapCorner::TopRight => {
                                na::vector![selection_bounds.maxs[0], selection_bounds.mins[1]]
                            }
                            SnapCorner::BottomLeft => {
                                na::vector![selection_bounds.mins[0], selection_bounds.maxs[1]]
                            }
                            SnapCorner::BottomRight => selection_bounds.maxs.coords,
                        };

                        let offset = engine_view
                            .document
                            .snap_position(snap_corner_pos + (element.pos - *current_pos))
                            - snap_corner_pos;

                        if offset.magnitude()
                            > Self::TRANSLATE_OFFSET_THRESHOLD / engine_view.camera.total_zoom()
                        {
                            // move selection
                            engine_view.store.translate_strokes(selection, offset);
                            engine_view
                                .store
                                .translate_strokes_images(selection, offset);
                            *selection_bounds = selection_bounds.translate(offset);
                            *current_pos += offset;
                        }

                        // possibly nudge camera
                        widget_flags |= engine_view
                            .camera
                            .nudge_w_pos(element.pos, engine_view.document);
                        widget_flags |= engine_view.document.expand_autoexpand(engine_view.camera);
                        engine_view.store.regenerate_rendering_in_viewport_threaded(
                            engine_view.tasks_tx.clone(),
                            false,
                            engine_view.camera.viewport(),
                            engine_view.camera.image_scale(),
                        );
                    }
                    ModifyState::Rotate {
                        rotation_center,
                        start_rotation_angle: _,
                        current_rotation_angle,
                    } => {
                        let new_rotation_angle = {
                            let vec = element.pos - rotation_center.coords;
                            na::Vector2::x().angle_ahead(&vec)
                        };
                        let angle_delta = new_rotation_angle - *current_rotation_angle;

                        if angle_delta.abs() > Self::ROTATE_ANGLE_THRESHOLD {
                            engine_view.store.rotate_strokes(
                                selection,
                                angle_delta,
                                *rotation_center,
                            );
                            engine_view.store.rotate_strokes_images(
                                selection,
                                angle_delta,
                                *rotation_center,
                            );

                            if let Some(new_bounds) =
                                engine_view.store.bounds_for_strokes(selection)
                            {
                                *selection_bounds = new_bounds;
                            }
                            *current_rotation_angle = new_rotation_angle;
                        }
                    }
                    ModifyState::Resize {
                        from_corner,
                        start_bounds,
                        start_pos,
                    } => {
                        tracing::debug!("resize state event");
                        let lock_aspectratio = engine_view
                            .pens_config
                            .selector_config
                            .resize_lock_aspectratio
                            || modifier_keys.contains(&ModifierKey::KeyboardCtrl);
                        let snap_corner_pos = match from_corner {
                            ResizeCorner::TopLeft => start_bounds.mins.coords,
                            ResizeCorner::TopRight => na::vector![
                                start_bounds.maxs.coords[0],
                                start_bounds.mins.coords[1]
                            ],
                            ResizeCorner::BottomLeft => na::vector![
                                start_bounds.mins.coords[0],
                                start_bounds.maxs.coords[1]
                            ],
                            ResizeCorner::BottomRight => start_bounds.maxs.coords,
                        };
                        let pivot = match from_corner {
                            ResizeCorner::TopLeft => start_bounds.maxs.coords,
                            ResizeCorner::TopRight => na::vector![
                                start_bounds.mins.coords[0],
                                start_bounds.maxs.coords[1]
                            ],
                            ResizeCorner::BottomLeft => na::vector![
                                start_bounds.maxs.coords[0],
                                start_bounds.mins.coords[1]
                            ],
                            ResizeCorner::BottomRight => start_bounds.mins.coords,
                        };
                        let mut offset_to_start = element.pos - *start_pos;
                        if !lock_aspectratio {
                            offset_to_start = engine_view
                                .document
                                .snap_position(snap_corner_pos + offset_to_start)
                                - snap_corner_pos;
                        }
                        offset_to_start = match from_corner {
                            ResizeCorner::TopLeft => -offset_to_start,
                            ResizeCorner::TopRight => {
                                na::vector![offset_to_start[0], -offset_to_start[1]]
                            }
                            ResizeCorner::BottomLeft => {
                                na::vector![-offset_to_start[0], offset_to_start[1]]
                            }
                            ResizeCorner::BottomRight => offset_to_start,
                        };
                        if lock_aspectratio {
                            let start_extents = start_bounds.extents();
                            let start_mean = start_extents.mean();
                            let offset_mean = offset_to_start.mean();
                            offset_to_start = start_extents * (offset_mean / start_mean);
                        }

                        // need to set more reasonable defaults for min size (based on stroke width ? + actual size, NOT just min and max multipliers)

                        // find why this issue only occurs when we start having negative values for the start coordinates
                        // a.k.a. the start_bounds.extents() + offset_to_start

                        // affect only scale_resize
                        let min_extents = na::vector![
                            1e-2f64 / selection_bounds.extents().x,
                            1e-2f64 / selection_bounds.extents().y
                        ];
                        let hundred_lim = na::vector![5f64, 5f64]; // in a frame, noticeable ?
                                                                   // 2 : 9 frames to catch up
                                                                   // 5 : 4 frames to catch up if 100 jump
                        let set_positive = na::vector![1e-15f64, 1e-15f64];

                        let scale_resize = (start_bounds.extents() + offset_to_start)
                            .maxs(&set_positive) // force positive before division
                            .component_div(&selection_bounds.extents()) // some dangerous unwrap here ...
                            .map(|x| if !x.is_finite() { 0.0f64 } else { x })
                            .maxs(&min_extents); //apply the extent and then we should not be smaller than 0.01 in either directions
                                                 //.mins(&hundred_lim); // for now commented, would bound the max resize factor
                        
                        if scale_resize.x > 2.0f64 || scale_resize.y > 2.0f64 {
                            tracing::debug!("large resize that could activate that intermittent stretched image");
                        }

                        // only affects stroke width here
                        let min_multiplier = na::vector![1e-5f64, 1e-5f64]; // or limit stroke width into the general sizes limits
                                                                            // check if this is the case or not : NOT checked
                        let scale_stroke = (start_bounds.extents() + offset_to_start)
                            .component_div(&engine_view.store.initial_size_selection.unwrap())
                            .maxs(&min_multiplier); // some dangerous unwrap here ...

                        // debug traces here just for info
                        tracing::debug!(
                            "start coordinates {:?}",
                            start_bounds.extents() + offset_to_start
                        );
                        tracing::debug!(
                            "initial size {:?}",
                            engine_view.store.initial_size_selection
                        );
                        tracing::debug!("selection bounds {:?}", selection_bounds.extents());
                        tracing::debug!("coordinates maxes {:?}", min_extents);
                        tracing::debug!("size {:?}", selection_bounds.extents());
                        tracing::debug!("scale {:?} {:?}", scale_stroke, scale_resize);

                        // resize strokes
                        // [5] : we do that on the width directly. Needs to change
                        // but we have to have a "resize has finished" to be in place
                        engine_view.store.scale_strokes_with_pivot(
                            selection,
                            scale_stroke,
                            scale_resize,
                            pivot,
                        ); // [4].
                           // this should distinguish between end of resize and resize in progress
                           // we also need the original size of the elements in addition to their displayed sizes
                           // scale_strokes_with_pivot is also used in the resize_image part. So we need to copy the ghost values in that case (to do on merge)

                        engine_view.store.scale_strokes_images_with_pivot(
                            selection,
                            scale_resize,
                            pivot,
                        );
                        *selection_bounds = selection_bounds
                            .translate(-pivot)
                            .scale_non_uniform(scale_resize)
                            .translate(pivot);

                        // possibly nudge camera
                        widget_flags |= engine_view
                            .camera
                            .nudge_w_pos(element.pos, engine_view.document);
                        widget_flags |= engine_view.document.expand_autoexpand(engine_view.camera);

                        tracing::debug!("regenerate rendering viewport");
                        engine_view.store.regenerate_rendering_in_viewport_threaded(
                            engine_view.tasks_tx.clone(),
                            false,
                            engine_view.camera.viewport(),
                            engine_view.camera.image_scale(),
                        );
                    }
                }

                widget_flags.store_modified = true;

                EventResult {
                    handled: true,
                    propagate: EventPropagation::Stop,
                    progress,
                }
            }
        };

        (event_result, widget_flags)
    }

    pub(super) fn handle_pen_event_up(
        &mut self,
        element: Element,
        _modifier_keys: Vec<ModifierKey>,
        _now: Instant,
        engine_view: &mut EngineViewMut,
    ) -> (EventResult<PenProgress>, WidgetFlags) {
        let mut widget_flags = WidgetFlags::default();
        let selector_bounds = self.bounds_on_doc(&engine_view.as_im());

        let event_result = match &mut self.state {
            SelectorState::Idle => EventResult {
                handled: false,
                propagate: EventPropagation::Proceed,
                progress: PenProgress::Idle,
            },
            SelectorState::Selecting { path } => {
                let mut progress = PenProgress::Finished;

                let new_selection = match engine_view.pens_config.selector_config.style {
                    SelectorStyle::Polygon => {
                        if path.len() >= 3 {
                            engine_view
                                .store
                                .strokes_hitboxes_contained_in_path_polygon(
                                    path,
                                    engine_view.camera.viewport(),
                                )
                        } else {
                            vec![]
                        }
                    }
                    SelectorStyle::Rectangle => {
                        if let (Some(first), Some(last)) = (path.first(), path.last()) {
                            let aabb = Aabb::new_positive(first.pos.into(), last.pos.into());
                            engine_view.store.strokes_hitboxes_contained_in_aabb(
                                aabb,
                                engine_view.camera.viewport(),
                            )
                        } else {
                            vec![]
                        }
                    }
                    SelectorStyle::Single => {
                        if let Some(key) = path.last().and_then(|last| {
                            engine_view
                                .store
                                .stroke_hitboxes_contain_coord(
                                    engine_view.camera.viewport(),
                                    last.pos,
                                )
                                .pop()
                        }) {
                            vec![key]
                        } else {
                            vec![]
                        }
                    }
                    SelectorStyle::IntersectingPath => {
                        if path.len() >= 3 {
                            engine_view.store.strokes_hitboxes_intersect_path(
                                path,
                                engine_view.camera.viewport(),
                            )
                        } else {
                            vec![]
                        }
                    }
                };
                if !new_selection.is_empty() {
                    // we made a new selection
                    tracing::debug!("new selection made");
                    engine_view.store.set_selected_keys(&new_selection, true);
                    widget_flags.store_modified = true;
                    widget_flags.deselect_color_setters = true;

                    if let Some(new_bounds) = engine_view.store.bounds_for_strokes(&new_selection) {
                        // Change to the modify state
                        self.state = SelectorState::ModifySelection {
                            modify_state: ModifyState::default(),
                            selection: new_selection,
                            selection_bounds: new_bounds,
                        };
                        progress = PenProgress::InProgress;
                    }
                }

                EventResult {
                    handled: true,
                    propagate: EventPropagation::Stop,
                    progress,
                }
            }
            SelectorState::ModifySelection {
                modify_state,
                selection,
                selection_bounds,
            } => {
                match modify_state {
                    ModifyState::Translate { .. }
                    | ModifyState::Rotate { .. }
                    | ModifyState::Resize { .. } => {
                        engine_view.store.update_geometry_for_strokes(selection);
                        widget_flags |= engine_view
                            .document
                            .resize_autoexpand(engine_view.store, engine_view.camera);
                        engine_view.store.regenerate_rendering_in_viewport_threaded(
                            engine_view.tasks_tx.clone(),
                            false,
                            engine_view.camera.viewport(),
                            engine_view.camera.image_scale(),
                        );

                        if let Some(new_bounds) = engine_view.store.bounds_for_strokes(selection) {
                            *selection_bounds = new_bounds;
                        }

                        widget_flags |= engine_view.store.record(Instant::now());
                        widget_flags.store_modified = true;
                    }
                    _ => {}
                }

                *modify_state = if selector_bounds
                    .map(|b| b.contains_local_point(&element.pos.into()))
                    .unwrap_or(false)
                {
                    ModifyState::Hover(element.pos)
                } else {
                    ModifyState::Up
                };

                EventResult {
                    handled: true,
                    propagate: EventPropagation::Stop,
                    progress: PenProgress::InProgress,
                }
            }
        };

        (event_result, widget_flags)
    }

    pub(super) fn handle_pen_event_proximity(
        &mut self,
        element: Element,
        _modifier_keys: Vec<ModifierKey>,
        _now: Instant,
        engine_view: &mut EngineViewMut,
    ) -> (EventResult<PenProgress>, WidgetFlags) {
        let widget_flags = WidgetFlags::default();
        let selector_bounds = self.bounds_on_doc(&engine_view.as_im());

        let event_result = match &mut self.state {
            SelectorState::Idle => EventResult {
                handled: false,
                propagate: EventPropagation::Proceed,
                progress: PenProgress::Idle,
            },
            SelectorState::Selecting { .. } => EventResult {
                handled: true,
                propagate: EventPropagation::Stop,
                progress: PenProgress::InProgress,
            },
            SelectorState::ModifySelection { modify_state, .. } => {
                *modify_state = if selector_bounds
                    .map(|b| b.contains_local_point(&element.pos.into()))
                    .unwrap_or(false)
                {
                    ModifyState::Hover(element.pos)
                } else {
                    ModifyState::Up
                };
                EventResult {
                    handled: true,
                    propagate: EventPropagation::Stop,
                    progress: PenProgress::InProgress,
                }
            }
        };

        (event_result, widget_flags)
    }

    pub(super) fn handle_pen_event_keypressed(
        &mut self,
        keyboard_key: KeyboardKey,
        modifier_keys: Vec<ModifierKey>,
        _now: Instant,
        engine_view: &mut EngineViewMut,
    ) -> (EventResult<PenProgress>, WidgetFlags) {
        let mut widget_flags = WidgetFlags::default();

        let event_result = match &mut self.state {
            SelectorState::Idle => match keyboard_key {
                KeyboardKey::Unicode('a') => {
                    self.select_all(modifier_keys, engine_view, &mut widget_flags);
                    EventResult {
                        handled: true,
                        propagate: EventPropagation::Stop,
                        progress: PenProgress::InProgress,
                    }
                }
                _ => EventResult {
                    handled: false,
                    propagate: EventPropagation::Proceed,
                    progress: PenProgress::InProgress,
                },
            },
            SelectorState::Selecting { .. } => match keyboard_key {
                KeyboardKey::Unicode('a') => {
                    self.select_all(modifier_keys, engine_view, &mut widget_flags);
                    EventResult {
                        handled: true,
                        propagate: EventPropagation::Stop,
                        progress: PenProgress::InProgress,
                    }
                }
                _ => EventResult {
                    handled: false,
                    propagate: EventPropagation::Proceed,
                    progress: PenProgress::InProgress,
                },
            },
            SelectorState::ModifySelection { selection, .. } => {
                match keyboard_key {
                    KeyboardKey::Unicode('a') => {
                        self.select_all(modifier_keys, engine_view, &mut widget_flags);
                        EventResult {
                            handled: true,
                            propagate: EventPropagation::Stop,
                            progress: PenProgress::InProgress,
                        }
                    }
                    KeyboardKey::Unicode('d') => {
                        //Duplicate selection
                        if modifier_keys.contains(&ModifierKey::KeyboardCtrl) {
                            let duplicated = engine_view.store.duplicate_selection();
                            engine_view.store.update_geometry_for_strokes(&duplicated);
                            engine_view.store.regenerate_rendering_for_strokes_threaded(
                                engine_view.tasks_tx.clone(),
                                &duplicated,
                                engine_view.camera.viewport(),
                                engine_view.camera.image_scale(),
                            );

                            widget_flags |= engine_view.store.record(Instant::now());
                            widget_flags.resize = true;
                            widget_flags.store_modified = true;
                        }
                        EventResult {
                            handled: true,
                            propagate: EventPropagation::Stop,
                            progress: PenProgress::Finished,
                        }
                    }
                    KeyboardKey::Delete | KeyboardKey::BackSpace => {
                        engine_view.store.set_trashed_keys(selection, true);
                        widget_flags |= super::cancel_selection(selection, engine_view);
                        self.state = SelectorState::Idle;
                        EventResult {
                            handled: true,
                            propagate: EventPropagation::Stop,
                            progress: PenProgress::Finished,
                        }
                    }
                    KeyboardKey::Escape => {
                        widget_flags |= super::cancel_selection(selection, engine_view);
                        self.state = SelectorState::Idle;
                        EventResult {
                            handled: true,
                            propagate: EventPropagation::Stop,
                            progress: PenProgress::Finished,
                        }
                    }
                    _ => EventResult {
                        handled: false,
                        propagate: EventPropagation::Proceed,
                        progress: PenProgress::InProgress,
                    },
                }
            }
        };

        (event_result, widget_flags)
    }

    pub(super) fn handle_pen_event_text(
        &mut self,
        _text: String,
        _now: Instant,
        _engine_view: &mut EngineViewMut,
    ) -> (EventResult<PenProgress>, WidgetFlags) {
        let widget_flags = WidgetFlags::default();

        let event_result = match &mut self.state {
            SelectorState::Idle => EventResult {
                handled: false,
                propagate: EventPropagation::Proceed,
                progress: PenProgress::Idle,
            },
            SelectorState::Selecting { .. } => EventResult {
                handled: false,
                propagate: EventPropagation::Proceed,
                progress: PenProgress::InProgress,
            },
            SelectorState::ModifySelection { .. } => EventResult {
                handled: false,
                propagate: EventPropagation::Proceed,
                progress: PenProgress::InProgress,
            },
        };

        (event_result, widget_flags)
    }

    pub(super) fn handle_pen_event_cancel(
        &mut self,
        _now: Instant,
        engine_view: &mut EngineViewMut,
    ) -> (EventResult<PenProgress>, WidgetFlags) {
        let mut widget_flags = WidgetFlags::default();

        let event_result = match &mut self.state {
            SelectorState::Idle => EventResult {
                handled: false,
                propagate: EventPropagation::Proceed,
                progress: PenProgress::Idle,
            },
            SelectorState::Selecting { .. } => {
                self.state = SelectorState::Idle;
                EventResult {
                    handled: true,
                    propagate: EventPropagation::Stop,
                    progress: PenProgress::Finished,
                }
            }
            SelectorState::ModifySelection { selection, .. } => {
                widget_flags |= super::cancel_selection(selection, engine_view);
                self.state = SelectorState::Idle;
                EventResult {
                    handled: true,
                    propagate: EventPropagation::Stop,
                    progress: PenProgress::Finished,
                }
            }
        };

        (event_result, widget_flags)
    }
}
