// Copyright 2024 the Vello Authors
// Copyright 2025 the Tabulon Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Simple example.

#[path = "../../render_context.rs"]
mod render_context;

use anyhow::Result;
use std::sync::Arc;
use vello_common::kurbo::{Circle, DEFAULT_ACCURACY, Ellipse, Line, RoundedRect, Shape, Stroke};
use vello_common::peniko::Color;
use vello_hybrid::{RenderSize, Renderer, Scene};
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::window::Window;

use render_context::{RenderContext, RenderSurface, create_vello_renderer, create_winit_window};

enum RenderState<'s> {
    /// `RenderSurface` and `Window` for active rendering.
    Active {
        // The `RenderSurface` and the `Window` must be in this order, so that the surface is dropped first.
        surface: Box<RenderSurface<'s>>,
        window: Arc<Window>,
    },
    /// Cache a window so that it can be reused when the app is resumed after being suspended.
    Suspended(Option<Arc<Window>>),
}

struct SimpleVelloApp<'s> {
    // The vello RenderContext which is a global context that lasts for the
    // lifetime of the application
    context: RenderContext,

    // An array of renderers, one per wgpu device
    renderers: Vec<Option<Renderer>>,

    // State for our example where we store the winit Window and the wgpu Surface
    state: RenderState<'s>,

    // A vello Scene which is a data structure which allows one to build up a
    // description a scene to be drawn (with paths, fills, images, text, etc)
    // which is then passed to a renderer for rendering
    scene: Scene,

    /// Tabulon Vello environment.
    tv_environment: tabulon_vello::Environment,
}

impl ApplicationHandler for SimpleVelloApp<'_> {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let RenderState::Suspended(cached_window) = &mut self.state else {
            return;
        };

        // Get the winit window cached in a previous Suspended event or else create a new window
        let window = cached_window
            .take()
            .unwrap_or_else(|| create_winit_window(event_loop, 1044, 800, "Vello Shapes"));

        // Create a Vello Hybrid surface.
        let size = window.inner_size();
        let surface_future = {
            let surface = self
                .context
                .instance
                .create_surface(wgpu::SurfaceTarget::from(window.clone()))
                .expect("Error creating surface");
            let dev_id = pollster::block_on(self.context.device(Some(&surface)))
                .expect("No compatible device");
            let device_handle = &self.context.devices[dev_id];
            let capabilities = surface.get_capabilities(&device_handle.adapter);
            let format = preferred_surface_format(&capabilities.formats);
            self.context.create_render_surface(
                surface,
                size.width,
                size.height,
                wgpu::PresentMode::AutoVsync,
                format,
            )
        };
        let surface = pollster::block_on(surface_future);
        self.scene = scene_for_surface(size.width, size.height);

        // Create a Vello Hybrid renderer for the surface (using its device id).
        self.renderers
            .resize_with(self.context.devices.len(), || None);
        self.renderers[surface.dev_id] = Some(create_vello_renderer(&self.context, &surface));

        // Save the Window and Surface to a state variable
        self.state = RenderState::Active {
            surface: Box::new(surface),
            window,
        };
    }

    fn suspended(&mut self, _event_loop: &ActiveEventLoop) {
        if let RenderState::Active { window, .. } = &self.state {
            self.state = RenderState::Suspended(Some(window.clone()));
        }
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        window_id: winit::window::WindowId,
        event: WindowEvent,
    ) {
        let surface = match &mut self.state {
            RenderState::Active { surface, window } if window.id() == window_id => surface,
            _ => return,
        };

        match event {
            WindowEvent::CloseRequested => event_loop.exit(),

            WindowEvent::Resized(size) => {
                self.context
                    .resize_surface(surface, size.width, size.height);
                self.scene = scene_for_surface(size.width, size.height);
            }

            WindowEvent::RedrawRequested => {
                // Empty the scene of objects to draw. You could create a new Scene each time, but in this case
                // the same Scene is reused so that the underlying memory allocation can also be reused.
                self.scene.reset();

                add_shapes_to_scene(&mut self.tv_environment, &mut self.scene);

                let wgpu::SurfaceConfiguration { width, height, .. } = surface.config;

                let device_handle = &self.context.devices[surface.dev_id];
                let render_size = RenderSize { width, height };

                let surface_texture = surface
                    .surface
                    .get_current_texture()
                    .expect("failed to get surface texture");
                let texture_view = surface_texture
                    .texture
                    .create_view(&wgpu::TextureViewDescriptor::default());
                let mut encoder =
                    device_handle
                        .device
                        .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                            label: Some("Vello Hybrid Render to Surface"),
                        });

                clear_texture(
                    &mut encoder,
                    &texture_view,
                    wgpu::Color {
                        r: 0.0,
                        g: 0.0,
                        b: 0.0,
                        a: 1.0,
                    },
                );

                self.renderers[surface.dev_id]
                    .as_mut()
                    .unwrap()
                    .render(
                        &self.scene,
                        &device_handle.device,
                        &device_handle.queue,
                        &mut encoder,
                        &render_size,
                        &texture_view,
                    )
                    .expect("failed to render to surface");

                device_handle.queue.submit([encoder.finish()]);
                surface_texture.present();

                device_handle
                    .device
                    .poll(wgpu::PollType::Poll)
                    .expect("failed to poll device");
            }
            _ => {}
        }
    }
}

fn main() -> Result<()> {
    let mut app = SimpleVelloApp {
        context: RenderContext::new(),
        renderers: vec![],
        state: RenderState::Suspended(None),
        scene: scene_for_surface(1044, 800),
        tv_environment: Default::default(),
    };

    let event_loop = EventLoop::new()?;
    event_loop
        .run_app(&mut app)
        .expect("Couldn't run event loop");
    Ok(())
}

fn preferred_surface_format(formats: &[wgpu::TextureFormat]) -> wgpu::TextureFormat {
    formats
        .iter()
        .copied()
        .find(|format| {
            matches!(
                format,
                wgpu::TextureFormat::Bgra8Unorm | wgpu::TextureFormat::Rgba8Unorm
            )
        })
        .unwrap_or_else(|| formats[0])
}

fn scene_for_surface(width: u32, height: u32) -> Scene {
    Scene::new(
        u16::try_from(width.max(1)).expect("surface width exceeds Scene limits"),
        u16::try_from(height.max(1)).expect("surface height exceeds Scene limits"),
    )
}

fn clear_texture(encoder: &mut wgpu::CommandEncoder, view: &wgpu::TextureView, color: wgpu::Color) {
    let _pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some("Clear Surface"),
        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
            view,
            resolve_target: None,
            ops: wgpu::Operations {
                load: wgpu::LoadOp::Clear(color),
                store: wgpu::StoreOp::Store,
            },
            depth_slice: None,
        })],
        depth_stencil_attachment: None,
        occlusion_query_set: None,
        timestamp_writes: None,
        multiview_mask: None,
    });
}

/// Add shapes to a vello scene. This does not actually render the shapes, but adds them
/// to the Scene data structure which represents a set of objects to draw.
fn add_shapes_to_scene(tv_environment: &mut tabulon_vello::Environment, scene: &mut Scene) {
    use tabulon::shape::{FatPaint, FatShape};
    use tabulon::{graphics_bag::GraphicsBag, render_layer::RenderLayer};

    let mut rl = RenderLayer::default();
    let mut gb = GraphicsBag::default();

    // Draw an outlined rectangle
    let paint = gb.register_paint(FatPaint {
        stroke: Stroke::new(6.0),
        stroke_paint: Some(Color::new([0.9804, 0.702, 0.5294, 1.]).into()),
        fill_paint: None,
    });
    rl.push_with_bag(
        &mut gb,
        FatShape {
            transform: Default::default(),
            paint,
            path: Arc::from(
                RoundedRect::new(10.0, 10.0, 240.0, 240.0, 20.0).to_path(DEFAULT_ACCURACY),
            ),
        },
    );

    // Draw a filled circle
    let paint = gb.register_paint(FatPaint {
        stroke: Default::default(),
        stroke_paint: None,
        fill_paint: Some(Color::new([0.9529, 0.5451, 0.6588, 1.]).into()),
    });
    rl.push_with_bag(
        &mut gb,
        FatShape {
            transform: Default::default(),
            paint,
            path: Arc::from(Circle::new((420.0, 200.0), 120.0).to_path(DEFAULT_ACCURACY)),
        },
    );

    // Draw a filled ellipse
    let paint = gb.register_paint(FatPaint {
        stroke: Default::default(),
        stroke_paint: None,
        fill_paint: Some(Color::new([0.7961, 0.651, 0.9686, 1.]).into()),
    });
    rl.push_with_bag(
        &mut gb,
        FatShape {
            transform: Default::default(),
            paint,
            path: Arc::from(
                Ellipse::new((250.0, 420.0), (100.0, 160.0), -90.0).to_path(DEFAULT_ACCURACY),
            ),
        },
    );

    // Draw a straight line
    let paint = gb.register_paint(FatPaint {
        stroke: Stroke::new(6.0),
        stroke_paint: Some(Color::new([0.5373, 0.7059, 0.9804, 1.]).into()),
        fill_paint: None,
    });
    rl.push_with_bag(
        &mut gb,
        FatShape {
            transform: Default::default(),
            paint,
            path: Arc::from(Line::new((260.0, 20.0), (620.0, 100.0)).to_path(DEFAULT_ACCURACY)),
        },
    );

    tv_environment.add_render_layer_to_scene(scene, &gb, &rl, None);
}
