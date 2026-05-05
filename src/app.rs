use chrono::{Local, NaiveDate};
use eframe::egui::{self, Color32, FontFamily, FontId, RichText, Stroke, Vec2};
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver};

use crate::fetcher::Article;
use crate::llm::check_ollama;
use crate::pipeline::{run_pipeline, PipelineConfig};
use crate::progress::{BriefStats, ProgressEvent};
use crate::storage::{Storage, StoredBrief};

const BG: Color32 = Color32::from_rgb(14, 13, 10);
const BG_RAISED: Color32 = Color32::from_rgb(22, 20, 15);
const BG_PAPER: Color32 = Color32::from_rgb(28, 26, 20);
const INK: Color32 = Color32::from_rgb(244, 237, 224);
const INK_DIM: Color32 = Color32::from_rgb(184, 175, 155);
const INK_FAINT: Color32 = Color32::from_rgb(107, 101, 85);
const RULE: Color32 = Color32::from_rgb(42, 38, 32);
const ACCENT: Color32 = Color32::from_rgb(255, 87, 34);
const GOLD: Color32 = Color32::from_rgb(212, 168, 87);
const GREEN: Color32 = Color32::from_rgb(126, 184, 143);

#[derive(PartialEq)]
enum View { Idle, Loading, Results }

pub struct TechBriefApp {
    runtime: Arc<tokio::runtime::Runtime>,
    storage: Storage,
    view: View,

    model: String,
    hours: i64,
    top_n: usize,

    progress_rx: Option<UnboundedReceiver<ProgressEvent>>,
    progress_log: Arc<Mutex<Vec<String>>>,
    current_stage: String,
    current_message: String,
    current_percent: u8,

    current_brief: Option<DisplayedBrief>,
    topic_filter: String,

    ollama_ok: bool,
    last_ollama_check: std::time::Instant,
    ollama_check_rx: Option<tokio::sync::oneshot::Receiver<bool>>,

    available_dates: Vec<NaiveDate>,
}

#[derive(Clone)]
struct DisplayedBrief {
    date: NaiveDate,
    brief: String,
    articles: Vec<Article>,
    stats: BriefStats,
    model: String,
}

impl DisplayedBrief {
    fn from_stored(s: StoredBrief) -> Self {
        Self { date: s.date, brief: s.brief, articles: s.articles, stats: s.stats, model: s.model }
    }
}

impl TechBriefApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        configure_fonts(&cc.egui_ctx);
        configure_style(&cc.egui_ctx);

        let runtime = Arc::new(
            tokio::runtime::Builder::new_multi_thread().enable_all().build().expect("tokio runtime")
        );
        let storage = Storage::open().expect("open storage");
        let available_dates = storage.all_dates().unwrap_or_default();
        let today = Local::now().date_naive();
        let current_brief = storage.load(today).ok().flatten().map(DisplayedBrief::from_stored);
        let view = if current_brief.is_some() { View::Results } else { View::Idle };

        Self {
            runtime, storage, view,
            model: "llama3.1:8b".to_string(),
            hours: 24, top_n: 20,
            progress_rx: None,
            progress_log: Arc::new(Mutex::new(Vec::new())),
            current_stage: String::new(),
            current_message: String::new(),
            current_percent: 0,
            current_brief, topic_filter: "all".to_string(),
            ollama_ok: false,
            last_ollama_check: std::time::Instant::now() - std::time::Duration::from_secs(60),
            ollama_check_rx: None,
            available_dates,
        }
    }

    fn start_fetch(&mut self) {
        let (tx, rx) = unbounded_channel();
        self.progress_rx = Some(rx);
        self.progress_log.lock().unwrap().clear();
        self.current_stage = "INIT".to_string();
        self.current_message = "Starting…".to_string();
        self.current_percent = 0;
        self.view = View::Loading;

        let cfg = PipelineConfig { model: self.model.clone(), hours: self.hours, top_n: self.top_n };
        self.runtime.spawn(async move { run_pipeline(cfg, tx).await; });
    }

    fn poll_progress(&mut self, ctx: &egui::Context) {
        let mut completed = false;
        if let Some(rx) = self.progress_rx.as_mut() {
            while let Ok(event) = rx.try_recv() {
                match event {
                    ProgressEvent::Stage { stage, message, percent } => {
                        self.current_stage = stage.clone();
                        self.current_message = message.clone();
                        self.current_percent = percent;
                        self.progress_log.lock().unwrap().push(format!("[{}] {}", stage, message));
                    }
                    ProgressEvent::Done { brief, articles, stats } => {
                        let today = Local::now().date_naive();
                        let _ = self.storage.save(today, &brief, &articles, &stats, &self.model);
                        self.available_dates = self.storage.all_dates().unwrap_or_default();
                        self.current_brief = Some(DisplayedBrief {
                            date: today, brief, articles, stats, model: self.model.clone(),
                        });
                        self.topic_filter = "all".to_string();
                        self.view = View::Results;
                        completed = true;
                    }
                    ProgressEvent::Error(e) => {
                        self.current_stage = "ERROR".to_string();
                        self.current_message = e;
                        self.current_percent = 0;
                    }
                }
            }
        }
        if completed { self.progress_rx = None; }
        if self.view == View::Loading {
            ctx.request_repaint_after(std::time::Duration::from_millis(100));
        }
    }

    fn poll_ollama(&mut self) {
        // Receive previous check result if any
        if let Some(rx) = self.ollama_check_rx.as_mut() {
            if let Ok(result) = rx.try_recv() {
                self.ollama_ok = result;
                self.ollama_check_rx = None;
            }
        }

        // Kick off a new check periodically
        if self.ollama_check_rx.is_none()
            && self.last_ollama_check.elapsed() > std::time::Duration::from_secs(10)
        {
            let model = self.model.clone();
            let (tx, rx) = tokio::sync::oneshot::channel();
            self.ollama_check_rx = Some(rx);
            self.last_ollama_check = std::time::Instant::now();
            self.runtime.spawn(async move {
                let ok = check_ollama(&model).await;
                let _ = tx.send(ok);
            });
        }
    }

    fn navigate(&mut self, target: NaiveDate) {
        if let Ok(Some(stored)) = self.storage.load(target) {
            self.current_brief = Some(DisplayedBrief::from_stored(stored));
            self.topic_filter = "all".to_string();
            self.view = View::Results;
        }
    }
}

impl eframe::App for TechBriefApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.poll_progress(ctx);
        self.poll_ollama();

        egui::CentralPanel::default()
            .frame(egui::Frame::none().fill(BG).inner_margin(egui::Margin::ZERO))
            .show(ctx, |ui| {
                draw_masthead(ui, self.ollama_ok, &self.model);
                egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                    match self.view {
                        View::Idle => self.draw_idle(ui),
                        View::Loading => self.draw_loading(ui),
                        View::Results => self.draw_results(ui),
                    }
                });
            });
    }
}

impl TechBriefApp {
    fn draw_idle(&mut self, ui: &mut egui::Ui) {
        ui.add_space(60.0);
        ui.vertical_centered(|ui| {
            ui.set_max_width(720.0);
            ui.vertical(|ui| {
                ui.label(overline("TODAY'S BRIEFING"));
                ui.add_space(14.0);
                ui.label(RichText::new("What matters")
                    .font(FontId::new(72.0, FontFamily::Name("serif".into())))
                    .color(INK));
                ui.label(RichText::new("in your world.")
                    .font(FontId::new(72.0, FontFamily::Name("serif-italic".into())))
                    .color(GOLD));
                ui.add_space(20.0);
                ui.label(RichText::new("A synthesis of AI, research, startups, hardware, security and emerging tech — distilled by a local model that never leaves your machine.")
                    .font(FontId::new(17.0, FontFamily::Name("serif".into())))
                    .color(INK_DIM));

                ui.add_space(36.0);
                draw_rule(ui);
                ui.add_space(14.0);

                ui.horizontal(|ui| {
                    control_select(ui, "MODEL", &mut self.model,
                        &["llama3.1:8b", "qwen2.5:7b", "qwen2.5:14b", "mistral:7b", "gemma2:9b"]);
                    ui.add_space(28.0);

                    let mut hours_str = self.hours.to_string();
                    control_select(ui, "WINDOW (HRS)", &mut hours_str, &["12", "24", "48", "72"]);
                    self.hours = hours_str.parse().unwrap_or(24);
                    ui.add_space(28.0);

                    let mut top_str = self.top_n.to_string();
                    control_select(ui, "DEPTH (TOP N)", &mut top_str, &["10", "15", "20", "30"]);
                    self.top_n = top_str.parse().unwrap_or(20);
                });

                ui.add_space(14.0);
                draw_rule(ui);
                ui.add_space(36.0);

                if fetch_button(ui, "FETCH WHAT MATTERS TODAY  →").clicked() {
                    self.start_fetch();
                }
                ui.add_space(16.0);
                ui.label(RichText::new("⌘ Make sure Ollama is running locally with the chosen model pulled.")
                    .font(FontId::new(11.0, FontFamily::Monospace)).color(INK_FAINT));

                if !self.available_dates.is_empty() {
                    ui.add_space(40.0);
                    ui.label(overline("HISTORY"));
                    ui.add_space(8.0);
                    ui.horizontal_wrapped(|ui| {
                        let dates = self.available_dates.clone();
                        for date in dates.iter().rev().take(14) {
                            let label = date.format("%b %d").to_string();
                            if history_pill(ui, &label).clicked() {
                                self.navigate(*date);
                            }
                        }
                    });
                }
            });
        });
        ui.add_space(60.0);
    }

    fn draw_loading(&mut self, ui: &mut egui::Ui) {
        ui.add_space(40.0);
        ui.vertical_centered(|ui| {
            ui.set_max_width(820.0);
            ui.vertical(|ui| {
                ui.label(overline(&self.current_stage));
                ui.add_space(10.0);
                ui.label(RichText::new(&self.current_message)
                    .font(FontId::new(22.0, FontFamily::Name("serif-italic".into())))
                    .color(INK));
                ui.add_space(28.0);

                let track_h = 3.0;
                let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), track_h), egui::Sense::hover());
                ui.painter().rect_filled(rect, 0.0, RULE);
                let fill_w = rect.width() * (self.current_percent as f32 / 100.0);
                let fill_rect = egui::Rect::from_min_size(rect.min, Vec2::new(fill_w, track_h));
                ui.painter().rect_filled(fill_rect, 0.0, ACCENT);

                ui.add_space(8.0);
                ui.label(RichText::new(format!("{}%", self.current_percent))
                    .font(FontId::new(11.0, FontFamily::Monospace))
                    .color(INK_FAINT));

                ui.add_space(36.0);
                draw_rule(ui);
                ui.add_space(20.0);
                ui.label(overline("LIVE LOG"));
                ui.add_space(8.0);

                let log = self.progress_log.lock().unwrap().clone();
                let visible: Vec<&String> = log.iter().rev().take(15).collect();
                for line in visible.iter().rev() {
                    ui.label(RichText::new(line.as_str())
                        .font(FontId::new(11.5, FontFamily::Monospace))
                        .color(INK_DIM));
                }
            });
        });
        ui.add_space(60.0);
    }

    fn draw_results(&mut self, ui: &mut egui::Ui) {
        let brief = match &self.current_brief { Some(b) => b.clone(), None => return };

        ui.add_space(20.0);
        ui.vertical_centered(|ui| {
            ui.set_max_width(1000.0);
            ui.vertical(|ui| {
                self.draw_date_nav(ui, brief.date);

                ui.add_space(20.0);
                draw_double_rule(ui);
                ui.add_space(28.0);

                ui.label(overline("EXECUTIVE BRIEFING"));
                ui.add_space(16.0);
                ui.label(RichText::new(&brief.brief)
                    .font(FontId::new(20.0, FontFamily::Name("serif".into())))
                    .color(INK));

                ui.add_space(20.0);
                ui.label(RichText::new(format!(
                    "{} feeds · {} articles scanned · {} surfaced · model {}",
                    brief.stats.feeds_fetched, brief.stats.total_articles, brief.stats.articles_kept, brief.model,
                ))
                    .font(FontId::new(10.0, FontFamily::Monospace))
                    .color(INK_FAINT));

                ui.add_space(24.0);
                draw_rule(ui);
                ui.add_space(20.0);

                self.draw_filter_bar(ui, &brief.articles);
                ui.add_space(24.0);
                self.draw_articles(ui, &brief.articles);

                ui.add_space(40.0);
                ui.vertical_centered(|ui| {
                    if ghost_button(ui, "FETCH AGAIN").clicked() {
                        self.start_fetch();
                    }
                });
                ui.add_space(40.0);
            });
        });
    }

    fn draw_date_nav(&mut self, ui: &mut egui::Ui, current: NaiveDate) {
        let prev = self.storage.previous_date(current).ok().flatten();
        let next = self.storage.next_date(current).ok().flatten();
        let today = Local::now().date_naive();

        ui.horizontal(|ui| {
            let prev_label = match prev {
                Some(d) => format!("←  {}", d.format("%b %d")),
                None => "←  no earlier".into(),
            };
            let prev_resp = ui.add_enabled(prev.is_some(), egui::Button::new(
                RichText::new(prev_label).font(FontId::new(10.5, FontFamily::Monospace)).color(INK_DIM)
            ).fill(Color32::TRANSPARENT).stroke(Stroke::NONE));
            if prev_resp.clicked() { if let Some(d) = prev { self.navigate(d); } }

            ui.add_space(12.0);
            ui.label(RichText::new(current.format("%A · %B %d, %Y").to_string())
                .font(FontId::new(20.0, FontFamily::Name("serif-italic".into())))
                .color(GOLD));
            ui.add_space(12.0);

            let next_label = match next {
                Some(d) => format!("{}  →", d.format("%b %d")),
                None => "no later  →".into(),
            };
            let next_resp = ui.add_enabled(next.is_some(), egui::Button::new(
                RichText::new(next_label).font(FontId::new(10.5, FontFamily::Monospace)).color(INK_DIM)
            ).fill(Color32::TRANSPARENT).stroke(Stroke::NONE));
            if next_resp.clicked() { if let Some(d) = next { self.navigate(d); } }

            ui.add_space(20.0);
            if current != today && ghost_button(ui, "JUMP TO TODAY").clicked() {
                if let Ok(Some(stored)) = self.storage.load(today) {
                    self.current_brief = Some(DisplayedBrief::from_stored(stored));
                } else {
                    self.view = View::Idle;
                }
            }
        });
    }

    fn draw_filter_bar(&mut self, ui: &mut egui::Ui, articles: &[Article]) {
        let mut topics: Vec<String> = articles.iter().filter_map(|a| a.topic_tag.clone()).collect();
        topics.sort();
        topics.dedup();

        ui.horizontal_wrapped(|ui| {
            if topic_pill(ui, "all", self.topic_filter == "all").clicked() {
                self.topic_filter = "all".to_string();
            }
            for t in &topics {
                if topic_pill(ui, t, self.topic_filter == *t).clicked() {
                    self.topic_filter = t.clone();
                }
            }
        });
    }

    fn draw_articles(&mut self, ui: &mut egui::Ui, articles: &[Article]) {
        let filter = self.topic_filter.clone();
        let filtered: Vec<&Article> = articles.iter()
            .filter(|a| filter == "all" || a.topic_tag.as_deref() == Some(&filter))
            .collect();

        let col_w = (ui.available_width() - 24.0) / 2.0;
        for chunk in filtered.chunks(2) {
            ui.horizontal_top(|ui| {
                for article in chunk {
                    ui.allocate_ui_with_layout(
                        Vec2::new(col_w, 0.0),
                        egui::Layout::top_down(egui::Align::LEFT),
                        |ui| draw_article_card(ui, article),
                    );
                    ui.add_space(20.0);
                }
            });
            ui.add_space(20.0);
        }
    }
}

fn draw_masthead(ui: &mut egui::Ui, ollama_ok: bool, model: &str) {
    egui::Frame::none().fill(BG)
        .inner_margin(egui::Margin { left: 36.0, right: 36.0, top: 22.0, bottom: 16.0 })
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new("▲").font(FontId::proportional(24.0)).color(ACCENT));
                ui.add_space(12.0);
                ui.vertical(|ui| {
                    ui.label(RichText::new("TECHBRIEF")
                        .font(FontId::new(26.0, FontFamily::Name("serif-bold".into())))
                        .color(INK));
                    ui.label(RichText::new("PERSONAL INTELLIGENCE · VOL. I")
                        .font(FontId::new(9.5, FontFamily::Monospace))
                        .color(INK_FAINT));
                });
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let dot_color = if ollama_ok { GREEN } else { ACCENT };
                    let status_text = if ollama_ok {
                        format!("OLLAMA · {} READY", model.to_uppercase())
                    } else {
                        "OLLAMA OFFLINE / MODEL MISSING".to_string()
                    };
                    ui.label(RichText::new(status_text)
                        .font(FontId::new(10.0, FontFamily::Monospace)).color(INK_FAINT));
                    ui.add_space(8.0);
                    let (rect, _) = ui.allocate_exact_size(Vec2::splat(8.0), egui::Sense::hover());
                    ui.painter().circle_filled(rect.center(), 4.0, dot_color);
                    ui.add_space(16.0);
                    ui.label(RichText::new(Local::now().format("%A · %B %d, %Y").to_string())
                        .font(FontId::new(13.0, FontFamily::Name("serif-italic".into())))
                        .color(INK_DIM));
                });
            });
        });
    // Bottom rule under the masthead
    draw_rule(ui);
}

fn draw_rule(ui: &mut egui::Ui) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 1.0), egui::Sense::hover());
    ui.painter().line_segment([rect.left_center(), rect.right_center()], Stroke::new(1.0, RULE));
}

fn draw_double_rule(ui: &mut egui::Ui) {
    for _ in 0..2 {
        let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 1.0), egui::Sense::hover());
        ui.painter().line_segment([rect.left_center(), rect.right_center()], Stroke::new(1.0, RULE));
        ui.add_space(2.0);
    }
}

fn overline(text: &str) -> RichText {
    RichText::new(text).font(FontId::new(10.5, FontFamily::Monospace)).color(ACCENT)
}

fn control_select(ui: &mut egui::Ui, label: &str, value: &mut String, options: &[&str]) {
    ui.vertical(|ui| {
        ui.label(RichText::new(label)
            .font(FontId::new(9.5, FontFamily::Monospace)).color(INK_FAINT));
        egui::ComboBox::from_id_salt(label)
            .selected_text(RichText::new(value.as_str())
                .font(FontId::new(15.0, FontFamily::Name("serif-italic".into())))
                .color(GOLD))
            .show_ui(ui, |ui| {
                for opt in options {
                    if ui.selectable_label(value == opt, *opt).clicked() {
                        *value = opt.to_string();
                    }
                }
            });
    });
}

fn fetch_button(ui: &mut egui::Ui, text: &str) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(Vec2::new(360.0, 56.0), egui::Sense::click());
    let bg = if response.hovered() { GOLD } else { ACCENT };
    ui.painter().rect_filled(rect, 0.0, bg);
    let galley = ui.painter().layout_no_wrap(
        text.to_string(),
        FontId::new(13.0, FontFamily::Monospace),
        BG,
    );
    let pos = rect.center() - galley.size() / 2.0;
    ui.painter().galley(pos, galley, BG);
    if response.hovered() { ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand); }
    response
}

fn ghost_button(ui: &mut egui::Ui, text: &str) -> egui::Response {
    ui.add(egui::Button::new(
        RichText::new(text).font(FontId::new(10.5, FontFamily::Monospace)).color(INK_DIM)
    ).fill(Color32::TRANSPARENT).stroke(Stroke::new(1.0, RULE)).min_size(Vec2::new(0.0, 36.0)))
}

fn topic_pill(ui: &mut egui::Ui, text: &str, active: bool) -> egui::Response {
    let bg = if active { INK } else { Color32::TRANSPARENT };
    let fg = if active { BG } else { INK_DIM };
    let resp = ui.add(egui::Button::new(
        RichText::new(text.to_uppercase())
            .font(FontId::new(9.5, FontFamily::Monospace))
            .color(fg)
    ).fill(bg).stroke(Stroke::new(1.0, RULE)));
    ui.add_space(4.0);
    resp
}

fn history_pill(ui: &mut egui::Ui, text: &str) -> egui::Response {
    let resp = ui.add(egui::Button::new(
        RichText::new(text.to_uppercase())
            .font(FontId::new(10.0, FontFamily::Monospace))
            .color(INK_DIM)
    ).fill(BG_RAISED).stroke(Stroke::new(1.0, RULE)));
    ui.add_space(6.0);
    resp
}

fn draw_article_card(ui: &mut egui::Ui, article: &Article) {
    egui::Frame::none()
        .fill(BG_PAPER)
        .inner_margin(egui::Margin::same(20.0))
        .stroke(Stroke::new(1.0, RULE))
        .show(ui, |ui| {
            if let Some(topic) = &article.topic_tag {
                ui.horizontal(|ui| {
                    egui::Frame::none()
                        .fill(Color32::from_rgba_premultiplied(255, 87, 34, 30))
                        .inner_margin(egui::Margin { left: 8.0, right: 8.0, top: 3.0, bottom: 3.0 })
                        .show(ui, |ui| {
                            ui.label(RichText::new(topic.to_uppercase())
                                .font(FontId::new(9.0, FontFamily::Monospace)).color(GOLD));
                        });
                });
                ui.add_space(8.0);
            }

            ui.horizontal(|ui| {
                let date_str = article.published.with_timezone(&Local).format("%b %d · %H:%M").to_string();
                ui.label(RichText::new(format!("{} · {}", article.source.to_uppercase(), date_str))
                    .font(FontId::new(9.5, FontFamily::Monospace)).color(INK_FAINT));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let score = article.relevance.unwrap_or(0.0);
                    ui.label(RichText::new(format!("{:.1}/10", score))
                        .font(FontId::new(10.0, FontFamily::Monospace)).color(ACCENT));
                });
            });

            ui.add_space(10.0);
            ui.label(RichText::new(&article.title)
                .font(FontId::new(18.0, FontFamily::Name("serif-bold".into()))).color(INK));
            ui.add_space(10.0);

            let summary = article.ai_summary.clone().unwrap_or_else(|| article.summary.clone());
            ui.label(RichText::new(summary)
                .font(FontId::new(13.5, FontFamily::Name("serif".into()))).color(INK_DIM));

            ui.add_space(14.0);
            let resp = ui.add(egui::Label::new(
                RichText::new("READ AT SOURCE  →")
                    .font(FontId::new(9.5, FontFamily::Monospace)).color(ACCENT)
            ).sense(egui::Sense::click()));
            if resp.hovered() { ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand); }
            if resp.clicked() { let _ = open::that(&article.url); }
        });
}

fn configure_style(ctx: &egui::Context) {
    let mut style = (*ctx.style()).clone();
    style.visuals.panel_fill = BG;
    style.visuals.window_fill = BG;
    style.visuals.extreme_bg_color = BG;
    style.visuals.faint_bg_color = BG_RAISED;
    style.visuals.override_text_color = Some(INK);
    style.visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0, RULE);
    style.visuals.widgets.inactive.bg_fill = BG_RAISED;
    style.visuals.widgets.inactive.weak_bg_fill = BG_RAISED;
    style.visuals.widgets.inactive.bg_stroke = Stroke::new(1.0, RULE);
    style.visuals.widgets.hovered.bg_fill = BG_PAPER;
    style.visuals.widgets.hovered.weak_bg_fill = BG_PAPER;
    style.visuals.widgets.hovered.bg_stroke = Stroke::new(1.0, INK_DIM);
    style.visuals.widgets.active.bg_fill = ACCENT;
    style.visuals.widgets.active.weak_bg_fill = ACCENT;
    style.visuals.selection.bg_fill = ACCENT;
    style.visuals.selection.stroke = Stroke::new(1.0, ACCENT);
    style.spacing.item_spacing = Vec2::new(4.0, 6.0);
    ctx.set_style(style);
}

fn configure_fonts(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();

    let try_load = |name: &str, path: &std::path::Path, fonts: &mut egui::FontDefinitions| -> bool {
        match std::fs::read(path) {
            Ok(bytes) => {
                fonts.font_data.insert(name.to_string(), egui::FontData::from_owned(bytes));
                true
            }
            Err(_) => false,
        }
    };

    let assets = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("assets");
    let has_serif        = try_load("serif",        &assets.join("Fraunces-Regular.ttf"),    &mut fonts);
    let has_serif_bold   = try_load("serif_bold",   &assets.join("Fraunces-Bold.ttf"),       &mut fonts);
    let has_serif_italic = try_load("serif_italic", &assets.join("Fraunces-Italic.ttf"),     &mut fonts);
    let has_mono         = try_load("mono",         &assets.join("JetBrainsMono-Regular.ttf"), &mut fonts);

    let default_prop: Vec<String> = fonts.families.get(&FontFamily::Proportional)
        .cloned().unwrap_or_default();

    if has_serif {
        fonts.families.insert(FontFamily::Name("serif".into()), vec!["serif".into()]);
    } else {
        fonts.families.insert(FontFamily::Name("serif".into()), default_prop.clone());
    }

    if has_serif_bold {
        fonts.families.insert(FontFamily::Name("serif-bold".into()), vec!["serif_bold".into()]);
    } else {
        let fallback = fonts.families.get(&FontFamily::Name("serif".into())).cloned().unwrap_or(default_prop.clone());
        fonts.families.insert(FontFamily::Name("serif-bold".into()), fallback);
    }

    if has_serif_italic {
        fonts.families.insert(FontFamily::Name("serif-italic".into()), vec!["serif_italic".into()]);
    } else {
        let fallback = fonts.families.get(&FontFamily::Name("serif".into())).cloned().unwrap_or(default_prop);
        fonts.families.insert(FontFamily::Name("serif-italic".into()), fallback);
    }

    if has_mono {
        fonts.families.entry(FontFamily::Monospace).or_default().insert(0, "mono".into());
    }

    ctx.set_fonts(fonts);
}
