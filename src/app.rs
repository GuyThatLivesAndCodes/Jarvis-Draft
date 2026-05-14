use std::sync::mpsc;
use chrono::Local;
use egui::{
    Align2, Color32, FontFamily, FontId, Key, Painter, Pos2, Rect, Rounding,
    Stroke, Vec2, pos2, vec2,
};

use crate::{
    api_client,
    llm_detector::check_llm_status,
    models::{AIProvider, ChatMessage, LLMStatus},
    settings::Settings,
};

// ── Particles ────────────────────────────────────────────────────────────────

struct Particle {
    x: f32, y: f32,
    vx: f32, vy: f32,
    size: f32, alpha: f32,
    dir: f32, spd: f32,
}

impl Particle {
    fn new_around(cx: f32, cy: f32) -> Self {
        let a = rand_f32() * std::f32::consts::TAU;
        let r = 90.0 + rand_f32() * 90.0;
        Self {
            x: cx + a.cos() * r,
            y: cy + a.sin() * r,
            vx: (rand_f32() - 0.5) * 0.25,
            vy: (rand_f32() - 0.5) * 0.25,
            size: 0.8 + rand_f32() * 1.8,
            alpha: rand_f32(),
            dir: if rand_f32() < 0.5 { 1.0 } else { -1.0 },
            spd: 0.003 + rand_f32() * 0.006,
        }
    }

    fn update(&mut self, cx: f32, cy: f32) {
        self.alpha += self.dir * self.spd;
        if self.alpha >= 1.0 { self.alpha = 1.0; self.dir = -1.0; }
        if self.alpha <= 0.0 {
            *self = Self::new_around(cx, cy);
        } else {
            self.x += self.vx;
            self.y += self.vy;
        }
    }
}

fn rand_f32() -> f32 {
    use std::time::{SystemTime, UNIX_EPOCH};
    static SEED: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let mut s = SEED.load(std::sync::atomic::Ordering::Relaxed);
    if s == 0 {
        s = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().subsec_nanos() as u64;
    }
    s ^= s << 13; s ^= s >> 7; s ^= s << 17;
    SEED.store(s, std::sync::atomic::Ordering::Relaxed);
    (s & 0xFFFF) as f32 / 65535.0
}

// ── Notification ─────────────────────────────────────────────────────────────

struct Notification {
    message: String,
    born:    f64,
    ttl:     f64,
}

// ── App State ─────────────────────────────────────────────────────────────────

#[derive(PartialEq)]
enum Stage { Splash, Active }

pub struct JarvisApp {
    // state
    stage:        Stage,
    activated_at: Option<f64>,
    is_fullscreen: bool,

    // particles
    particles: Vec<Particle>,

    // settings
    settings: Settings,
    show_settings: bool,
    // field buffers for settings panel
    buf_anthropic: String,
    buf_openai:    String,
    buf_xai:       String,
    buf_admin_pw:  String,

    // ask input
    ask_text: String,

    // AI response
    ai_response: Option<String>,
    ai_pending:  bool,
    ai_rx:       mpsc::Receiver<Result<String, String>>,
    ai_tx:       mpsc::SyncSender<Result<String, String>>,

    // LLM status
    llm_status: LLMStatus,
    llm_rx:     mpsc::Receiver<LLMStatus>,

    // notifications
    notifications: Vec<Notification>,
}

impl JarvisApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        // Larger font sizes
        let mut style = (*cc.egui_ctx.style()).clone();
        style.text_styles.insert(
            egui::TextStyle::Body,
            FontId::new(14.0, FontFamily::Monospace),
        );
        style.text_styles.insert(
            egui::TextStyle::Button,
            FontId::new(12.0, FontFamily::Monospace),
        );
        cc.egui_ctx.set_style(style);

        let settings = Settings::load();

        // Load field buffers from saved credentials
        let buf_anthropic = settings.get_key(&AIProvider::Anthropic).unwrap_or("").to_string();
        let buf_openai    = settings.get_key(&AIProvider::OpenAI).unwrap_or("").to_string();
        let buf_xai       = settings.get_key(&AIProvider::XAI).unwrap_or("").to_string();

        // AI channel
        let (ai_tx, ai_rx) = mpsc::sync_channel(4);

        // LLM status channel — kick off background checker
        let (llm_tx, llm_rx) = mpsc::channel::<LLMStatus>();
        let ctx = cc.egui_ctx.clone();
        std::thread::spawn(move || {
            let rt = tokio::runtime::Runtime::new().expect("tokio");
            loop {
                let status = rt.block_on(check_llm_status());
                let _ = llm_tx.send(status);
                ctx.request_repaint();
                std::thread::sleep(std::time::Duration::from_secs(30));
            }
        });

        Self {
            stage:         Stage::Splash,
            activated_at:  None,
            is_fullscreen: true,
            particles:     vec![],       // populated on first frame once we know screen size
            settings,
            show_settings: false,
            buf_anthropic,
            buf_openai,
            buf_xai,
            buf_admin_pw:  String::new(),
            ask_text:      String::new(),
            ai_response:   None,
            ai_pending:    false,
            ai_rx,
            ai_tx,
            llm_status:    LLMStatus::default(),
            llm_rx,
            notifications: vec![],
        }
    }

    // ── Helpers ───────────────────────────────────────────────────────────────

    fn push_notif(&mut self, msg: impl Into<String>, t: f64) {
        self.notifications.push(Notification { message: msg.into(), born: t, ttl: 5.0 });
    }

    fn active_provider(&self) -> AIProvider {
        if self.llm_status.ollama_available    { return AIProvider::Ollama; }
        if self.llm_status.lm_studio_available { return AIProvider::LMStudio; }
        if self.settings.get_key(&AIProvider::Anthropic).is_some() { return AIProvider::Anthropic; }
        if self.settings.get_key(&AIProvider::OpenAI).is_some()    { return AIProvider::OpenAI; }
        if self.settings.get_key(&AIProvider::XAI).is_some()       { return AIProvider::XAI; }
        AIProvider::Ollama
    }

    fn send_query(&mut self, now: f64) {
        let text = self.ask_text.trim().to_string();
        if text.is_empty() || self.ai_pending { return; }
        self.ask_text.clear();
        self.ai_pending = true;
        self.push_notif("Sending query…", now);

        let provider = self.active_provider();
        let api_key  = self.settings.get_key(&provider).map(|s| s.to_string());
        let tx       = self.ai_tx.clone();
        let msgs     = vec![ChatMessage { role: "user".into(), content: text }];

        std::thread::spawn(move || {
            let rt = tokio::runtime::Runtime::new().expect("tokio");
            let res = rt.block_on(api_client::query(provider, msgs, api_key));
            let _ = tx.send(res.map(|r| r.content));
        });
    }

    // ── Drawing helpers ───────────────────────────────────────────────────────

    fn draw_background(painter: &Painter, rect: Rect) {
        painter.rect_filled(rect, 0.0, Color32::BLACK);
        // Corner radial glow
        for &(cx_f, cy_f) in &[(0.0f32,0.0f32),(1.0,0.0),(1.0,1.0),(0.0,1.0)] {
            let c = pos2(rect.left() + cx_f * rect.width(), rect.top() + cy_f * rect.height());
            let r = rect.width().min(rect.height()) * 0.6;
            painter.circle_filled(c, r, Color32::from_rgba_unmultiplied(30, 100, 180, 22));
        }
    }

    fn draw_grid(painter: &Painter, rect: Rect, alpha: f32) {
        let a   = (97.0 * alpha) as u8;
        let col = Color32::from_rgba_unmultiplied(35, 75, 115, a);
        let st  = Stroke::new(0.5, col);
        let step = 44.0f32;
        let mut x = rect.left();
        while x <= rect.right()  { painter.line_segment([pos2(x, rect.top()),  pos2(x, rect.bottom())], st); x += step; }
        let mut y = rect.top();
        while y <= rect.bottom() { painter.line_segment([pos2(rect.left(), y), pos2(rect.right(), y)], st);  y += step; }
    }

    fn draw_blob(painter: &Painter, center: Pos2, t: f64, hover: bool) {
        let t  = t as f32;
        let sz = if hover { 220.0 * 1.04 } else { 220.0 };

        // Build morphing polygon
        let n = 128usize;
        let pts: Vec<Pos2> = (0..n).map(|i| {
            let a = (i as f32 / n as f32) * std::f32::consts::TAU;
            let wobble = 1.0
                + 0.06 * (a * 3.0 + t * 0.8).sin()
                + 0.04 * (a * 5.0 + t * 0.5).cos()
                + 0.03 * (a * 2.0 - t * 0.6).sin();
            let r = sz * 0.5 * wobble;
            pos2(center.x + a.cos() * r, center.y + a.sin() * r)
        }).collect();

        // Glow rings
        for i in 0u8..4 {
            let expand = (4 - i) as f32 * 14.0;
            let alpha  = 30u8.saturating_sub(i * 8);
            let glow_pts: Vec<Pos2> = pts.iter().map(|p| {
                let d = *p - center;
                let factor = (d.length() + expand) / d.length().max(1.0);
                center + d * factor
            }).collect();
            painter.add(egui::Shape::convex_polygon(
                glow_pts,
                Color32::from_rgba_unmultiplied(60, 140, 210, alpha),
                Stroke::NONE,
            ));
        }

        // Main blob
        painter.add(egui::Shape::convex_polygon(
            pts,
            Color32::from_rgba_unmultiplied(90, 160, 220, 110),
            Stroke::NONE,
        ));

        // Inner highlight
        painter.circle_filled(
            center - vec2(sz * 0.08, sz * 0.1),
            sz * 0.18,
            Color32::from_rgba_unmultiplied(160, 210, 255, 28),
        );

        // "JARVIS" text
        painter.text(
            center,
            Align2::CENTER_CENTER,
            "JARVIS",
            FontId::new(sz * 0.155, FontFamily::Monospace),
            Color32::from_rgba_unmultiplied(200, 230, 255, 235),
        );
    }

    fn draw_particles(painter: &Painter, particles: &[Particle]) {
        for p in particles {
            let a = (p.alpha * 0.75 * 255.0) as u8;
            painter.circle_filled(
                pos2(p.x, p.y),
                p.size,
                Color32::from_rgba_unmultiplied(100, 170, 230, a),
            );
        }
    }

    fn draw_small_circle(painter: &Painter, center: Pos2, t: f64, scale: f32) {
        let r    = 45.0 * scale;
        let glow = Color32::from_rgba_unmultiplied(60, 140, 210, 60);
        for i in 1..=3u8 { painter.circle_filled(center, r + i as f32 * 8.0, glow); }
        painter.circle_filled(center, r, Color32::from_rgba_unmultiplied(90, 160, 220, 200));
        let pulse = (0.8 + 0.2 * (t * 2.0).sin() as f32).abs();
        painter.circle_stroke(center, r + 2.0, Stroke::new(1.5,
            Color32::from_rgba_unmultiplied(120, 190, 255, (120.0 * pulse) as u8)));
        painter.text(
            center, Align2::CENTER_CENTER, "JARVIS",
            FontId::new(10.0, FontFamily::Monospace),
            Color32::from_rgba_unmultiplied(200, 230, 255, 220),
        );
    }
}

// ── eframe::App ───────────────────────────────────────────────────────────────

impl eframe::App for JarvisApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let now = ctx.input(|i| i.time);

        // ── Collect LLM status updates ─────────────────────────────────────
        while let Ok(status) = self.llm_rx.try_recv() {
            if (status.ollama_available && !self.llm_status.ollama_available)
                || (status.lm_studio_available && !self.llm_status.lm_studio_available)
            {
                let who = if status.ollama_available { "Ollama" } else { "LM Studio" };
                if self.settings.notify_on_local_available {
                    self.push_notif(format!("Local LLM available: {who}"), now);
                }
            }
            self.llm_status = status;
        }

        // ── Collect AI responses ──────────────────────────────────────────
        while let Ok(res) = self.ai_rx.try_recv() {
            self.ai_pending = false;
            match res {
                Ok(content) => {
                    let preview = if content.len() > 120 { format!("{}…", &content[..120]) } else { content.clone() };
                    self.push_notif(preview, now);
                    self.ai_response = Some(content);
                }
                Err(e) => self.push_notif(format!("Error: {e}"), now),
            }
        }

        // ── Expire notifications ──────────────────────────────────────────
        self.notifications.retain(|n| now - n.born < n.ttl);

        // ── Keyboard ──────────────────────────────────────────────────────
        if ctx.input(|i| i.key_pressed(Key::F11)) {
            self.is_fullscreen = !self.is_fullscreen;
            ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(self.is_fullscreen));
        }
        if ctx.input(|i| i.key_pressed(Key::Escape)) && self.show_settings {
            self.show_settings = false;
        }

        // ── Settings side panel ───────────────────────────────────────────
        if self.show_settings {
            egui::SidePanel::right("settings_panel")
                .exact_width(380.0)
                .frame(egui::Frame {
                    fill:         Color32::from_rgba_unmultiplied(10, 25, 45, 245),
                    stroke:       Stroke::new(1.0, Color32::from_rgba_unmultiplied(50, 100, 160, 80)),
                    inner_margin: egui::Margin::same(20.0),
                    ..Default::default()
                })
                .show(ctx, |ui| {
                    self.draw_settings_panel(ui, now);
                });
        }

        // ── Main area ─────────────────────────────────────────────────────
        egui::CentralPanel::default()
            .frame(egui::Frame::none())
            .show(ctx, |ui| {
                let rect = ui.max_rect();
                let cx   = rect.center().x;
                let cy   = rect.center().y;

                // Init particles on first frame
                if self.particles.is_empty() {
                    self.particles = (0..55).map(|_| Particle::new_around(cx, cy)).collect();
                }

                // Clone painter so we can also use ui mutably for widgets
                let painter = ui.painter().clone();
                Self::draw_background(&painter, rect);

                // ── SPLASH ─────────────────────────────────────────────────
                if self.stage == Stage::Splash {
                    for p in &mut self.particles { p.update(cx, cy); }
                    Self::draw_particles(&painter, &self.particles);

                    let center    = pos2(cx, cy);
                    let blob_rect = Rect::from_center_size(center, Vec2::splat(260.0));
                    let hover     = ui.rect_contains_pointer(blob_rect);

                    Self::draw_blob(&painter, center, now, hover);

                    let clock = Local::now().format("%H:%M").to_string();
                    painter.text(
                        pos2(cx, cy + 160.0),
                        Align2::CENTER_CENTER,
                        &clock,
                        FontId::new(18.0, FontFamily::Monospace),
                        Color32::from_rgba_unmultiplied(130, 180, 230, 140),
                    );

                    if ui.allocate_rect(blob_rect, egui::Sense::click()).clicked() {
                        self.stage        = Stage::Active;
                        self.activated_at = Some(now);
                    }

                    ctx.request_repaint();
                }

                // ── ACTIVE ─────────────────────────────────────────────────
                if self.stage == Stage::Active {
                    let elapsed = (now - self.activated_at.unwrap_or(now)) as f32;

                    let grid_alpha = (elapsed - 0.15).max(0.0).min(0.6) / 0.6;
                    if grid_alpha > 0.0 { Self::draw_grid(&painter, rect, grid_alpha); }

                    let circle_scale = (elapsed - 0.35).max(0.0).min(0.75) / 0.75;
                    if circle_scale > 0.0 {
                        Self::draw_small_circle(&painter, pos2(cx, cy), now, circle_scale);
                    }

                    if elapsed > 0.5 {
                        self.draw_dashboard(ui, rect, now);
                    }

                    ctx.request_repaint();
                }

                // Repaint painter ref is already cloned, so re-clone for notifications
                let painter2 = ui.painter().clone();
                self.draw_notifications(&painter2, rect, now);
            });
    }
}

// ── Sub-drawing methods ────────────────────────────────────────────────────────

impl JarvisApp {
    fn draw_dashboard(&mut self, ui: &mut egui::Ui, rect: Rect, now: f64) {
        let bar_w = rect.width().min(600.0);
        let bar_h = 52.0;
        let bar_r = Rect::from_min_size(
            pos2(rect.center().x - bar_w / 2.0, rect.bottom() - bar_h - 18.0),
            vec2(bar_w, bar_h),
        );

        // Draw dashboard background
        let painter = ui.painter();
        painter.rect_filled(
            bar_r,
            Rounding::same(10.0),
            Color32::from_rgba_unmultiplied(10, 25, 45, 235),
        );
        painter.rect_stroke(
            bar_r,
            Rounding::same(10.0),
            Stroke::new(1.0, Color32::from_rgba_unmultiplied(50, 100, 160, 80)),
        );

        // Allocate inner layout area
        let mut inner = bar_r.shrink(8.0);
        let btn_sz    = vec2(36.0, 36.0);

        // Settings button
        let s_rect = Rect::from_min_size(inner.min, btn_sz);
        self.icon_button(ui, s_rect, "⚙", now, &mut |app: &mut JarvisApp, _| {
            app.show_settings = !app.show_settings;
        });
        inner.min.x += btn_sz.x + 6.0;

        // Admin button
        let a_rect = Rect::from_min_size(inner.min, btn_sz);
        self.icon_button(ui, a_rect, "🔒", now, &mut |app, n| {
            app.push_notif("Admin mode — set password in settings.", n);
        });
        inner.min.x += btn_sz.x + 8.0;

        // Right-side buttons (reserve space from right)
        let right_x = bar_r.right() - 8.0 - btn_sz.x * 2.0 - 6.0;

        // Text input for query — between left buttons and right buttons
        let input_r = Rect::from_min_max(
            pos2(inner.min.x, bar_r.min.y + 8.0),
            pos2(right_x - 6.0, bar_r.max.y - 8.0),
        );

        let response = ui.put(
            input_r,
            egui::TextEdit::singleline(&mut self.ask_text)
                .hint_text("Ask Jarvis…")
                .text_color(Color32::from_rgba_unmultiplied(180, 220, 255, 215))
                .frame(true)
                .font(FontId::new(11.0, FontFamily::Monospace)),
        );

        if response.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter)) {
            self.send_query(now);
        }

        // Mic / More buttons on right
        let mic_rect  = Rect::from_min_size(pos2(right_x, bar_r.min.y + 8.0), btn_sz);
        let more_rect = Rect::from_min_size(pos2(right_x + btn_sz.x + 6.0, bar_r.min.y + 8.0), btn_sz);
        self.icon_button(ui, mic_rect, "🎙", now, &mut |_, _| {});
        self.icon_button(ui, more_rect, "⋯", now, &mut |_, _| {});
    }

    fn icon_button<F>(&mut self, ui: &mut egui::Ui, rect: Rect, icon: &str, now: f64, action: &mut F)
    where F: FnMut(&mut JarvisApp, f64) {
        let resp = ui.allocate_rect(rect, egui::Sense::click());
        let bg   = if resp.hovered() {
            Color32::from_rgba_unmultiplied(70, 140, 200, 160)
        } else {
            Color32::from_rgba_unmultiplied(30, 65, 110, 130)
        };
        ui.painter().rect_filled(rect, Rounding::same(6.0), bg);
        ui.painter().rect_stroke(rect, Rounding::same(6.0),
            Stroke::new(1.0, Color32::from_rgba_unmultiplied(60, 120, 190, 60)));
        ui.painter().text(rect.center(), Align2::CENTER_CENTER, icon,
            FontId::new(16.0, FontFamily::Proportional),
            Color32::from_rgba_unmultiplied(180, 220, 255, 200));
        if resp.clicked() { action(self, now); }
    }

    fn draw_notifications(&self, painter: &Painter, rect: Rect, now: f64) {
        let mut top = rect.top() + 20.0;
        for n in &self.notifications {
            let age     = (now - n.born) as f32;
            let fade_in  = (age / 0.3).min(1.0);
            let fade_out = if (n.ttl - (now - n.born) as f64) < 0.5 {
                (n.ttl as f32 - age) / 0.5
            } else { 1.0 };
            let alpha = (fade_in * fade_out * 255.0) as u8;

            let notif_w = 320.0f32;
            let notif_r = Rect::from_min_size(
                pos2(rect.right() - notif_w - 20.0, top),
                vec2(notif_w, 60.0),
            );
            painter.rect_filled(notif_r, Rounding::same(8.0),
                Color32::from_rgba_unmultiplied(20, 60, 120, alpha.saturating_sub(30)));
            painter.rect_stroke(notif_r, Rounding::same(8.0),
                Stroke::new(1.0, Color32::from_rgba_unmultiplied(80, 160, 255, alpha / 2)));

            // Wrap text manually at ~45 chars
            let line1: String = n.message.chars().take(45).collect();
            let line2: String = n.message.chars().skip(45).take(45).collect();
            painter.text(
                pos2(notif_r.left() + 12.0, notif_r.top() + 14.0),
                Align2::LEFT_TOP,
                &line1,
                FontId::new(11.0, FontFamily::Monospace),
                Color32::from_rgba_unmultiplied(200, 230, 255, alpha),
            );
            if !line2.is_empty() {
                painter.text(
                    pos2(notif_r.left() + 12.0, notif_r.top() + 30.0),
                    Align2::LEFT_TOP,
                    &line2,
                    FontId::new(11.0, FontFamily::Monospace),
                    Color32::from_rgba_unmultiplied(200, 230, 255, alpha),
                );
            }
            top += 68.0;
        }
    }

    fn draw_settings_panel(&mut self, ui: &mut egui::Ui, now: f64) {
        let heading = Color32::from_rgba_unmultiplied(200, 230, 255, 235);
        let subtle  = Color32::from_rgba_unmultiplied(130, 180, 230, 180);
        let accent  = Color32::from_rgba_unmultiplied(80, 150, 220, 255);

        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("SETTINGS").color(heading).size(18.0).monospace());
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button(egui::RichText::new("✕").color(subtle)).clicked() {
                    self.show_settings = false;
                }
            });
        });

        ui.add_space(12.0);
        ui.separator();
        ui.add_space(8.0);

        // ── Local LLMs ────────────────────────────────────────────────────
        ui.label(egui::RichText::new("LOCAL LLMs").color(subtle).size(11.0).monospace());
        ui.add_space(4.0);
        self.llm_row(ui, "Ollama",    self.llm_status.ollama_available,
            &self.llm_status.ollama_models.clone());
        self.llm_row(ui, "LM Studio", self.llm_status.lm_studio_available,
            &self.llm_status.lm_studio_models.clone());
        ui.add_space(10.0);
        ui.separator();
        ui.add_space(8.0);

        // ── API Keys ──────────────────────────────────────────────────────
        // Anthropic
        ui.label(egui::RichText::new("ANTHROPIC API KEY").color(subtle).size(11.0).monospace());
        let save_anthropic = ui.horizontal(|ui| {
            ui.add(egui::TextEdit::singleline(&mut self.buf_anthropic)
                .password(true).desired_width(230.0)
                .font(FontId::new(12.0, FontFamily::Monospace)));
            ui.button(egui::RichText::new("Save").color(accent).monospace()).clicked()
        }).inner;
        if save_anthropic {
            let v = self.buf_anthropic.clone();
            self.settings.set_key(AIProvider::Anthropic, v);
            self.settings.save();
            self.push_notif("Anthropic key saved", now);
        }
        ui.add_space(8.0);

        // OpenAI
        ui.label(egui::RichText::new("OPENAI API KEY").color(subtle).size(11.0).monospace());
        let save_openai = ui.horizontal(|ui| {
            ui.add(egui::TextEdit::singleline(&mut self.buf_openai)
                .password(true).desired_width(230.0)
                .font(FontId::new(12.0, FontFamily::Monospace)));
            ui.button(egui::RichText::new("Save").color(accent).monospace()).clicked()
        }).inner;
        if save_openai {
            let v = self.buf_openai.clone();
            self.settings.set_key(AIProvider::OpenAI, v);
            self.settings.save();
            self.push_notif("OpenAI key saved", now);
        }
        ui.add_space(8.0);

        // xAI
        ui.label(egui::RichText::new("XAI API KEY").color(subtle).size(11.0).monospace());
        let save_xai = ui.horizontal(|ui| {
            ui.add(egui::TextEdit::singleline(&mut self.buf_xai)
                .password(true).desired_width(230.0)
                .font(FontId::new(12.0, FontFamily::Monospace)));
            ui.button(egui::RichText::new("Save").color(accent).monospace()).clicked()
        }).inner;
        if save_xai {
            let v = self.buf_xai.clone();
            self.settings.set_key(AIProvider::XAI, v);
            self.settings.save();
            self.push_notif("xAI key saved", now);
        }
        ui.add_space(8.0);

        ui.separator();
        ui.add_space(8.0);

        // ── Preferences ───────────────────────────────────────────────────
        ui.label(egui::RichText::new("PREFERENCES").color(subtle).size(11.0).monospace());
        ui.add_space(4.0);
        ui.checkbox(&mut self.settings.auto_switch_to_local,
            egui::RichText::new("Auto-switch to local LLM when available").color(heading).size(12.0));
        ui.checkbox(&mut self.settings.notify_on_local_available,
            egui::RichText::new("Notify when local LLM comes online").color(heading).size(12.0));

        ui.add_space(10.0);
        if ui.button(egui::RichText::new("Save Preferences").color(accent).monospace()).clicked() {
            self.settings.save();
            self.push_notif("Preferences saved.", now);
        }

        ui.add_space(10.0);
        ui.separator();
        ui.add_space(8.0);

        // ── Admin ─────────────────────────────────────────────────────────
        ui.label(egui::RichText::new("ADMIN MODE").color(subtle).size(11.0).monospace());
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            ui.add(egui::TextEdit::singleline(&mut self.buf_admin_pw)
                .hint_text("Admin password")
                .password(true)
                .desired_width(180.0)
                .font(FontId::new(12.0, FontFamily::Monospace)));
            if ui.button(egui::RichText::new("Enable").color(accent).monospace()).clicked() {
                if !self.buf_admin_pw.is_empty() {
                    self.settings.admin_password = self.buf_admin_pw.clone();
                    self.settings.admin_enabled  = true;
                    self.settings.save();
                    self.push_notif("Admin mode enabled.", now);
                }
            }
        });
        let admin_col = if self.settings.admin_enabled {
            Color32::from_rgb(76, 175, 80)
        } else {
            Color32::from_rgb(200, 80, 80)
        };
        ui.label(egui::RichText::new(
            if self.settings.admin_enabled { "● Admin ACTIVE" } else { "● Admin disabled" }
        ).color(admin_col).size(12.0));
    }

    fn llm_row(&self, ui: &mut egui::Ui, name: &str, online: bool, models: &[String]) {
        ui.horizontal(|ui| {
            let (col, symbol) = if online {
                (Color32::from_rgb(76, 175, 80), "●")
            } else {
                (Color32::from_rgb(200, 80, 80), "●")
            };
            ui.label(egui::RichText::new(symbol).color(col).size(12.0));
            let info = if online {
                format!("{name}: Online ({} model{})", models.len(), if models.len() == 1 {""} else {"s"})
            } else {
                format!("{name}: Offline")
            };
            ui.label(egui::RichText::new(info)
                .color(Color32::from_rgba_unmultiplied(200, 230, 255, 200))
                .size(12.0).monospace());
        });
        ui.add_space(2.0);
    }
}
