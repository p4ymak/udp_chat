use super::chat::{message::Message, Recepients, UdpChat};
use directories::ProjectDirs;
use eframe::{egui, epi};
use egui::*;
use epi::Storage;

pub struct ChatApp {
    chat: UdpChat,
    text: String,
}

impl epi::App for ChatApp {
    fn name(&self) -> &str {
        "UDP Chat"
    }
    fn warm_up_enabled(&self) -> bool {
        true
    }
    fn persist_native_window(&self) -> bool {
        false
    }
    fn persist_egui_memory(&self) -> bool {
        false
    }
    fn setup(
        &mut self,
        _ctx: &egui::CtxRef,
        frame: &mut epi::Frame<'_>,
        _storage: Option<&dyn Storage>,
    ) {
        self.chat.prelude(frame.repaint_signal());
    }
    fn on_exit(&mut self) {
        self.chat.message = Message::exit();
        self.chat.send(Recepients::All);
    }

    fn update(&mut self, ctx: &egui::CtxRef, _frame: &mut epi::Frame<'_>) {
        self.chat.receive();
        self.draw(ctx);
        self.handle_keys(ctx);
        // ctx.request_repaint();
    }
}

impl Default for ChatApp {
    fn default() -> Self {
        let db_path = ProjectDirs::from("com", "p4ymak", env!("CARGO_PKG_NAME")).map(|p| {
            std::fs::create_dir_all(&p.data_dir()).ok();
            p.data_dir().join("history.db")
        });
        ChatApp {
            chat: UdpChat::new("XXX".to_string(), 4444, db_path),
            text: String::new(),
        }
    }
}
impl ChatApp {
    fn handle_keys(&mut self, ctx: &egui::CtxRef) {
        for event in &ctx.input().raw.events {
            match event {
                Event::Key {
                    key: egui::Key::Enter,
                    pressed: true,
                    ..
                } => self.send(),
                Event::Key {
                    key: egui::Key::Escape,
                    pressed: true,
                    ..
                } => self.chat.clear_history(),

                _ => (),
            }
        }
    }
    fn send(&mut self) {
        if !self.text.trim().is_empty() {
            self.chat.message = Message::text(&self.text);
            self.chat.send(Recepients::Peers);
        }
        self.text = String::new();
    }
    fn draw(&mut self, ctx: &egui::CtxRef) {
        egui::TopBottomPanel::top("socket").show(ctx, |ui| {
            ui.with_layout(egui::Layout::left_to_right(), |ui| {
                ui.add(
                    egui::Label::new(format!("Online: {}", self.chat.peers.len()))
                        .wrap(false)
                        .strong(),
                );
                ui.label(format!("{}:{}", self.chat.ip, self.chat.port));
                ui.label(&self.chat.db_status);
            });
        });
        egui::TopBottomPanel::bottom("my_panel").show(ctx, |ui| {
            let message_box = ui.add(
                egui::TextEdit::multiline(&mut self.text)
                    .desired_width(f32::INFINITY)
                    .text_style(egui::TextStyle::Heading)
                    .id(egui::Id::new("text_input")),
            );
            message_box.request_focus();
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            egui::ScrollArea::vertical()
                .max_width(f32::INFINITY)
                .stick_to_bottom()
                .show(ui, |ui| {
                    self.chat.history.iter().for_each(|m| {
                        let (direction, fill_color) = match &m.0 {
                            x if x == &self.chat.ip => (
                                egui::Direction::RightToLeft,
                                egui::Color32::from_rgb(70, 70, 70),
                            ),
                            _ => (
                                egui::Direction::LeftToRight,
                                egui::Color32::from_rgb(42, 42, 42),
                            ),
                        };
                        ui.with_layout(
                            egui::Layout::from_main_dir_and_cross_align(
                                direction,
                                egui::Align::Min,
                            ),
                            |line| {
                                if m.0 != self.chat.ip {
                                    line.add(
                                        egui::Label::new(&m.0)
                                            .wrap(false)
                                            .strong()
                                            .sense(Sense::click()),
                                    )
                                    .clicked();
                                }
                                if line
                                    .add(
                                        egui::Button::new(&m.1)
                                            .wrap(true)
                                            .text_style(egui::TextStyle::Heading)
                                            .fill(fill_color),
                                    )
                                    .clicked()
                                {
                                    self.text.push_str(&m.1);
                                }
                            },
                        );
                    });
                });
        });
    }
}
