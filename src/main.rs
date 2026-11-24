mod app;
mod ui;

use explorer::config::AppConfig;
use relm4::RelmApp;

use crate::app::AppModel;

const APP_ID: &str = "org.cachyos.explorer";
const APP_CSS: &str = include_str!("../data/style.css");

fn main() {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("failed to initialize Tokio runtime");
    let _rt_guard = runtime.enter();

    let config = AppConfig::load();
    let app = RelmApp::new(APP_ID);
    relm4::set_global_css(APP_CSS);
    app.run::<AppModel>(config);
}
