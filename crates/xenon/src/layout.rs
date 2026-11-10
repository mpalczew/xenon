use std::cell::Cell;
use std::collections::HashMap;
use std::rc::{Rc, Weak};

use adw::prelude::*;
use xenon_core::{PaneId, PaneTree, SplitAxis, WorkspaceId};

use crate::app::App;
use crate::model::Tab;
use crate::pane::PaneView;

pub(crate) fn shape(node: &PaneTree<Tab>) -> String {
    match node {
        PaneTree::Leaf(leaf) => format!("L{}", leaf.id.0),
        PaneTree::Split { axis, children, .. } => {
            let inner: Vec<String> = children.iter().map(shape).collect();
            format!(
                "S{}[{}]",
                if *axis == SplitAxis::Horizontal {
                    'h'
                } else {
                    'v'
                },
                inner.join(",")
            )
        }
    }
}

pub(crate) fn detach(widget: &gtk::Widget) {
    if let Some(paned) = widget.downcast_ref::<gtk::Paned>() {
        for child in [paned.start_child(), paned.end_child()]
            .into_iter()
            .flatten()
        {
            detach(&child);
        }
        paned.set_start_child(None::<&gtk::Widget>);
        paned.set_end_child(None::<&gtk::Widget>);
    }
}

pub(crate) struct Ctx<'a> {
    pub app: &'a Rc<App>,
    pub ws: WorkspaceId,
    pub panes: &'a HashMap<PaneId, Rc<PaneView>>,
    pub busy: &'a Rc<Cell<bool>>,
}

pub(crate) fn build(ctx: &Ctx, node: &PaneTree<Tab>) -> gtk::Widget {
    match node {
        PaneTree::Leaf(leaf) => ctx.panes.get(&leaf.id).map_or_else(
            || gtk::Box::new(gtk::Orientation::Vertical, 0).upcast(),
            |p| p.root.clone().upcast(),
        ),
        PaneTree::Split {
            axis,
            children,
            sizes,
        } => {
            let widgets: Vec<gtk::Widget> = children.iter().map(|c| build(ctx, c)).collect();
            let keys: Vec<PaneId> = children
                .iter()
                .filter_map(|c| c.leaf_ids().last().copied())
                .collect();
            chain(ctx, *axis, widgets, sizes, &keys)
        }
    }
}

fn chain(
    ctx: &Ctx,
    axis: SplitAxis,
    mut widgets: Vec<gtk::Widget>,
    sizes: &[f32],
    keys: &[PaneId],
) -> gtk::Widget {
    let orientation = if axis == SplitAxis::Horizontal {
        gtk::Orientation::Horizontal
    } else {
        gtk::Orientation::Vertical
    };
    let mut acc = widgets.pop().expect("a split has children");
    for i in (0..widgets.len()).rev() {
        let paned = gtk::Paned::new(orientation);
        paned.set_wide_handle(false);
        paned.set_shrink_start_child(false);
        paned.set_shrink_end_child(false);
        paned.set_start_child(Some(&widgets[i]));
        paned.set_end_child(Some(&acc));
        let before: f32 = sizes[..i].iter().sum();
        let rest: f32 = sizes[i..].iter().sum::<f32>().max(0.01);
        place(&paned, orientation, sizes[i] / rest, ctx.busy);
        watch(ctx, &paned, orientation, before, keys[i]);
        acc = paned.upcast();
    }
    acc
}

fn extent(paned: &gtk::Paned, orientation: gtk::Orientation) -> i32 {
    if orientation == gtk::Orientation::Horizontal {
        paned.width()
    } else {
        paned.height()
    }
}

fn place(paned: &gtk::Paned, orientation: gtk::Orientation, fraction: f32, busy: &Rc<Cell<bool>>) {
    let busy = busy.clone();
    paned.add_tick_callback(move |p, _| {
        let size = extent(p, orientation);
        if size <= 0 {
            return gtk::glib::ControlFlow::Continue;
        }
        busy.set(true);
        p.set_position((f64::from(size) * f64::from(fraction)) as i32);
        busy.set(false);
        gtk::glib::ControlFlow::Break
    });
}

fn watch(ctx: &Ctx, paned: &gtk::Paned, orientation: gtk::Orientation, before: f32, key: PaneId) {
    let (weak, busy, ws): (Weak<App>, _, _) = (Rc::downgrade(ctx.app), ctx.busy.clone(), ctx.ws);
    paned.connect_position_notify(move |p| {
        let Some(app) = weak.upgrade() else { return };
        let (busy, paned) = (busy.clone(), p.clone());
        gtk::glib::idle_add_local_once(move || {
            let size = extent(&paned, orientation);
            if busy.get() || size <= 0 {
                return;
            }
            let within = f64::from(paned.position()) / f64::from(size);
            let cumulative = before + (1.0 - before) * within as f32;
            app.divider_moved(ws, key, cumulative);
        });
    });
}
