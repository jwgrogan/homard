use tauri::{
    image::Image,
    menu::{MenuBuilder, MenuItemBuilder},
    tray::TrayIconBuilder,
    App, Manager,
};

pub fn create_tray(app: &App) -> tauri::Result<()> {
    let open = MenuItemBuilder::new("Open arcctl")
        .id("open")
        .build(app)?;
    let quit = MenuItemBuilder::new("Quit arcctl")
        .id("quit")
        .build(app)?;

    let menu = MenuBuilder::new(app).item(&open).item(&quit).build()?;

    let icon = Image::from_bytes(include_bytes!("../icons/tray-icon.png"))?;

    TrayIconBuilder::new()
        .icon(icon)
        .icon_as_template(true)
        .tooltip("arcctl")
        .menu(&menu)
        .on_menu_event(|app, event| match event.id().as_ref() {
            "quit" => {
                app.exit(0);
            }
            "open" => {
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.show();
                    let _ = window.set_focus();
                }
                #[cfg(target_os = "macos")]
                {
                    let _ = app.set_activation_policy(tauri::ActivationPolicy::Regular);
                }
            }
            _ => {}
        })
        .build(app)?;

    Ok(())
}
