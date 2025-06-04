//! Minimal application.

// Don't show a console window when opening the app.
#![windows_subsystem = "windows"]

use masonry_winit::app::{AppDriver, DriverCtx};
use masonry_winit::core::{Action, StyleProperty, WidgetId};
use masonry_winit::dpi::LogicalSize;
use masonry_winit::parley::{Alignment, FontWeight};
use masonry_winit::widgets::{Axis, Button, Flex, Label, RootWidget, Split};
use tracing_subscriber::prelude::*;
use winit::window::Window;

mod dxf_viewer;
use dxf_viewer::TabulonDxfViewer;

struct Driver;

impl AppDriver for Driver {
    fn on_action(&mut self, ctx: &mut DriverCtx<'_>, widget_id: WidgetId, action: Action) {
        let dv = ctx.render_root.get_root_widget().children()[0].children()[1];

        use dxf_viewer::EntityInfoPanelAction;

        match action {
            Action::Other(e) => {
                if let Ok(a) = e.downcast::<EntityInfoPanelAction>() {
                    match *a {
                        EntityInfoPanelAction::Close => {
                            let flx = dv.children()[1];
                            let i = flx.children().iter().position(|x| x.id() == widget_id);
                            ctx.render_root.edit_widget(flx.id(), |mut x| {
                                Flex::remove_child(&mut x.downcast(), i.unwrap());
                            });
                        }
                        EntityInfoPanelAction::Hover(e) => {
                            ctx.render_root.edit_widget(dv.id(), |mut x| {
                                TabulonDxfViewer::set_pick(&mut x.downcast(), e)
                            });
                        }
                        EntityInfoPanelAction::HoverEnd => {
                            ctx.render_root.edit_widget(dv.id(), |mut x| {
                                TabulonDxfViewer::clear_pick(&mut x.downcast())
                            });
                        }
                    }
                }
            }
            _ => {}
        }
    }
}

#[cfg(feature = "tracing-tracy-memory")]
#[global_allocator]
static GLOBAL: tracy_client::ProfiledAllocator<std::alloc::System> =
    tracy_client::ProfiledAllocator::new(std::alloc::System, 100);

fn main() {
    #[cfg(feature = "tracing-tracy")]
    let subscriber = tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer())
        .with(
            tracing_subscriber::EnvFilter::builder()
                .with_default_directive(tracing::level_filters::LevelFilter::WARN.into())
                .from_env_lossy(),
        );

    #[cfg(feature = "tracing-tracy")]
    subscriber.init();

    let label = Label::new("")
        .with_style(StyleProperty::FontSize(32.0))
        .with_style(StyleProperty::FontWeight(FontWeight::BOLD))
        .with_alignment(Alignment::Middle);

    let Some(path_arg) = std::env::args().next_back() else {
        eprintln!("Please provide a DXF file path as the first argument.");
        return;
    };

    let dxf_viewer = TabulonDxfViewer::from_path_sync(path_arg, 1.0);

    // Arrange the two widgets vertically, with some padding
    let main_widget = Split::new(label, dxf_viewer)
        .min_size(128., 0.)
        .split_point(0.);

    let window_size = LogicalSize::new(400.0, 400.0);
    let window_attributes = Window::default_attributes()
        .with_title("Hello World!")
        .with_resizable(true)
        .with_min_inner_size(window_size);

    masonry_winit::app::run(
        masonry_winit::app::EventLoop::with_user_event(),
        window_attributes,
        RootWidget::new(main_widget),
        Driver,
    )
    .unwrap();
}
