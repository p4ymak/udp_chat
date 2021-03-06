mod app;
mod chat;
use app::ChatApp;
use eframe::egui::Vec2;

fn main() {
    let start_state = ChatApp::default();
    let options = eframe::NativeOptions {
        always_on_top: false,
        decorated: true,
        resizable: true,
        maximized: false,
        drag_and_drop_support: true,
        transparent: true,
        // icon_data: Some(icon),
        initial_window_size: Some(Vec2 { x: 350.0, y: 550.0 }),
        ..Default::default()
    };
    env_logger::init();
    eframe::run_native(Box::new(start_state), options);
}
