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
    /// Time spent on the last flowfield recomputation pass (ms). 0 if not yet run.
    pub pathfinding_ms: f32,
    /// Total number of flowfield recomputes since startup.
    pub flowfield_recomputes: u32,
}

/// One unit's debug draw data, already projected to egui screen points.
pub struct UnitDebugDraw {
    /// Unit centre in egui screen points.
    pub pos: egui::Pos2,
    /// Tip of the velocity arrow (0.5 s ahead) in egui screen points.
    pub vel_tip: egui::Pos2,
    /// Avoidance-radius circle size in screen points.
    pub radius_px: f32,
}

/// One flowfield cell's direction arrow, already projected to egui screen points.
///
/// Built from every Nth nav cell's flowfield direction; rendered as a short
/// cyan line with a dot at the tip. Toggled with F5.
pub struct FlowfieldArrowDraw {
    /// Arrow tail — cell centre projected to screen.
    pub from: egui::Pos2,
    /// Arrow tip — cell centre + direction * arrow_len, projected to screen.
    pub to: egui::Pos2,
}

/// One density-heatmap cell, already projected to egui screen points.
///
/// Drawn as a semi-transparent orange square whose opacity scales with
/// unit density. Toggled with F5 alongside the flowfield arrows.
pub struct DensityCell {
    /// Cell centre in egui screen points.
    pub center: egui::Pos2,
    /// Projected side length of the cell in screen points.
    pub size_px: f32,
    /// Density intensity in [0, 1]. 1.0 = fully saturated colour.
    pub intensity: f32,
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

    /// Render one egui frame covering all optional debug layers:
    ///
    /// - `density_cells`    — F5 density heatmap squares (`None` = hidden).
    /// - `flowfield_arrows` — F5 per-cell flowfield direction arrows (`None` = hidden).
    /// - `unit_draws`       — F4 per-unit radius circles + velocity arrows (`None` = hidden).
    /// - `stats`            — F3 stats panel (`None` = hidden).
    ///
    /// All layers are tessellated in a single egui pass for efficiency.
    pub fn render(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        window: &winit::window::Window,
        view: &wgpu::TextureView,
        screen_descriptor: &egui_wgpu::ScreenDescriptor,
        stats: Option<&DebugStats>,
        unit_draws: Option<&[UnitDebugDraw]>,
        flowfield_arrows: Option<&[FlowfieldArrowDraw]>,
        density_cells: Option<&[DensityCell]>,
    ) {
        let raw_input = self.egui_state.take_egui_input(window);

        let full_output = self.egui_ctx.run(raw_input, |ctx| {
            // ── F5: density heatmap (drawn first — behind everything else) ────
            if let Some(cells) = density_cells {
                if !cells.is_empty() {
                    let painter = ctx.layer_painter(egui::LayerId::new(
                        egui::Order::Background,
                        egui::Id::new("density_heat"),
                    ));
                    for cell in cells {
                        let alpha = (cell.intensity * 140.0) as u8;
                        painter.rect_filled(
                            egui::Rect::from_center_size(
                                cell.center,
                                egui::vec2(cell.size_px, cell.size_px),
                            ),
                            0.0,
                            egui::Color32::from_rgba_unmultiplied(255, 80, 0, alpha),
                        );
                    }
                }
            }

            // ── F5: flowfield arrows (drawn above density) ────────────────────
            if let Some(arrows) = flowfield_arrows {
                if !arrows.is_empty() {
                    let painter = ctx.layer_painter(egui::LayerId::new(
                        egui::Order::Background,
                        egui::Id::new("flowfield_arrows"),
                    ));
                    let arrow_stroke = egui::Stroke::new(
                        1.0,
                        egui::Color32::from_rgba_unmultiplied(0, 220, 255, 180),
                    );
                    for arrow in arrows {
                        painter.line_segment([arrow.from, arrow.to], arrow_stroke);
                        // Small dot at the tip to indicate direction.
                        painter.circle_filled(
                            arrow.to,
                            2.0,
                            egui::Color32::from_rgba_unmultiplied(0, 220, 255, 220),
                        );
                    }
                }
            }

            // ── F4: unit debug circles drawn on a background layer ───────────
            if let Some(draws) = unit_draws {
                let painter = ctx.layer_painter(egui::LayerId::new(
                    egui::Order::Background,
                    egui::Id::new("unit_debug"),
                ));
                let radius_stroke = egui::Stroke::new(
                    1.0,
                    egui::Color32::from_rgba_unmultiplied(255, 220, 0, 160),
                );
                let vel_stroke = egui::Stroke::new(
                    2.0,
                    egui::Color32::from_rgba_unmultiplied(80, 255, 140, 220),
                );
                for draw in draws {
                    // Avoidance-radius circle
                    painter.circle_stroke(draw.pos, draw.radius_px, radius_stroke);
                    // Velocity direction arrow
                    painter.line_segment([draw.pos, draw.vel_tip], vel_stroke);
                    // Arrowhead dot at the tip
                    painter.circle_filled(
                        draw.vel_tip,
                        2.5,
                        egui::Color32::from_rgba_unmultiplied(80, 255, 140, 220),
                    );
                }
            }

            // ── F3: stats panel ──────────────────────────────────────────────
            if let Some(stats) = stats {
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
                                ui.label(format!(
                                    "Pathfinding: {:.2} ms  Recomputes: {}",
                                    stats.pathfinding_ms,
                                    stats.flowfield_recomputes,
                                ));
                            });
                    });
            }
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
