//! AniLot — неофициальный клиент Anixart на Rust + Iced.

pub mod aniskip;
pub mod api;
pub mod app;
pub mod content;
pub mod hls;
pub mod models;
pub mod platform;
pub mod player;
pub mod shikimori;
pub mod storage;
pub mod subtitles;
pub mod update;
pub mod views;
pub mod views2;

use app::App;

pub fn run() -> iced::Result {
    let mut app = iced::application(App::new, App::update, App::view)
        .title(|_: &App| "AniLot".to_string())
        .subscription(App::subscription)
        .theme(views::make_theme)
        .window_size((440.0, 820.0));

    #[cfg(target_os = "android")]
    {
        app = app.default_font(iced_android::DEFAULT_FONT);
        for font in iced_android::fonts() {
            app = app.font(font);
        }
    }
    #[cfg(not(target_os = "android"))]
    {
        app = app.default_font(iced::Font::with_name("Fira Sans"));
    }
    app.run()
}

#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
fn android_main(app: iced_android::AndroidApp) {
    android_logger::init_once(android_logger::Config::default().with_max_level(log::LevelFilter::Info).with_tag("AniLot"));
    std::panic::set_hook(Box::new(|info| log::error!("{info}")));
    if let Some(path) = app.internal_data_path() {
        platform::set_data_dir(path);
    }
    iced_android::init(app);
    if let Err(error) = run() {
        log::error!("iced stopped: {error}");
    }
}
