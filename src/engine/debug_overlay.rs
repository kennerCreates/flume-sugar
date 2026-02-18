use egui::epaint::Shadow;

pub struct DebugStats {
    pub fps: u32,
    pub frame_time_avg_ms: f32,
    pub frame_time_min_ms: f32,
    pub frame_time_max_ms: f32,
    pub entity_count: usize,
    pub draw_calls: u32,
    pub resolution: (u32, u32),
    pub camera_target: (f32, f32),
    pub camera_distance: f32,
    pub camera_zoom_pct: f32,
}

pub struct DebugOverlay {
    pub visible: bool,
    egui_ctx: egui::Context,
    egui_state: egui_winit::State,
    egui_renderer: egui_wgpu::Renderer,
}

impl DebugOverlay {
    pub fn new(
        window: &winit::window::Window,
        device: &wgpu::Device,
        surface_format: wgpu::TextureFormat,
    ) -> Self {
        let egui_ctx = egui::Context::default();

        // Style: dark, semi-transparent, small monospace white font
        let mut visuals = egui::Visuals::dark();
        visuals.window_fill = egui::Color32::from_rgba_premultiplied(0, 0, 0, 180);
        visuals.window_stroke = egui::Stroke::NONE;
        visuals.window_shadow = Shadow::NONE;
        visuals.override_text_color = Some(egui::Color32::WHITE);
        egui_ctx.set_visuals(visuals);

        let mut style = (*egui_ctx.style()).clone();
        style.override_font_id = Some(egui::FontId::monospace(13.0));
        egui_ctx.set_style(style);

        let egui_state = egui_winit::State::new(
            egui_ctx.clone(),
            egui::ViewportId::ROOT,
            window,
            Some(window.scale_factor() as f32),
            None,
            None,
        );

        let egui_renderer = egui_wgpu::Renderer::new(
            device,
            surface_format,
            None,  // no depth
            1,     // msaa samples
            false, // no dithering
        );

        Self {
            visible: false,
            egui_ctx,
            egui_state,
            egui_renderer,
        }
    }

    pub fn toggle(&mut self) {
        self.visible = !self.visible;
    }

    pub fn handle_window_event(
        &mut self,
        window: &winit::window::Window,
        event: &winit::event::WindowEvent,
    ) -> egui_winit::EventResponse {
        self.egui_state.on_window_event(window, event)
    }

    pub fn render(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        window: &winit::window::Window,
        view: &wgpu::TextureView,
        screen_descriptor: &egui_wgpu::ScreenDescriptor,
        stats: &DebugStats,
    ) {
        let raw_input = self.egui_state.take_egui_input(window);

        let full_output = self.egui_ctx.run(raw_input, |ctx| {
            egui::Area::new(egui::Id::new("debug_overlay"))
                .fixed_pos(egui::pos2(10.0, 10.0))
                .show(ctx, |ui| {
                    egui::Frame::none()
                        .fill(egui::Color32::from_rgba_premultiplied(0, 0, 0, 180))
                        .inner_margin(egui::Margin::same(8.0))
                        .rounding(4.0)
                        .show(ui, |ui: &mut egui::Ui| {
                            ui.label(format!("FPS: {}", stats.fps));
                            ui.label(format!(
                                "Frame: {:.2} ms (min: {:.1} | max: {:.1})",
                                stats.frame_time_avg_ms,
                                stats.frame_time_min_ms,
                                stats.frame_time_max_ms
                            ));
                            ui.label(format!("Entities: {}", stats.entity_count));
                            ui.label(format!("Draw calls: {}", stats.draw_calls));
                            ui.label(format!(
                                "Resolution: {} x {}",
                                stats.resolution.0, stats.resolution.1
                            ));
                            ui.label(format!(
                                "Camera: ({:.1}, {:.1})  dist {:.1}  zoom {:.0}%",
                                stats.camera_target.0, stats.camera_target.1,
                                stats.camera_distance, stats.camera_zoom_pct
                            ));
                        });
                });
        });

        self.egui_state
            .handle_platform_output(window, full_output.platform_output);

        let tris = self
            .egui_ctx
            .tessellate(full_output.shapes, full_output.pixels_per_point);

        for (id, image_delta) in &full_output.textures_delta.set {
            self.egui_renderer
                .update_texture(device, queue, *id, image_delta);
        }

        self.egui_renderer
            .update_buffers(device, queue, encoder, &tris, screen_descriptor);

        {
            let render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("egui Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                occlusion_query_set: None,
                timestamp_writes: None,
            });

            self.egui_renderer
                .render(&mut render_pass.forget_lifetime(), &tris, screen_descriptor);
        }

        for id in &full_output.textures_delta.free {
            self.egui_renderer.free_texture(id);
        }
    }
}
