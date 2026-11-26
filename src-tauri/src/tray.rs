use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{
    AppHandle, Emitter, Manager, PhysicalPosition, WebviewUrl, WebviewWindowBuilder, WindowEvent,
};

pub const TRAY_ID: &str = "portline-tray";
pub const TRAY_POPUP_LABEL: &str = "tray-popup";
const POPUP_LOGICAL_W: f64 = 304.0;
const POPUP_LOGICAL_H: f64 = 408.0;

pub fn setup_tray(app: &AppHandle) -> tauri::Result<()> {
    // Pre-create hidden custom tray popover window so opening is instantaneous.
    let _ = ensure_tray_popup(app);

    let mut builder = TrayIconBuilder::with_id(TRAY_ID)
        .tooltip("Portline")
        .show_menu_on_left_click(false)
        .on_tray_icon_event(|tray, event| match event {
            TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } => {
                hide_tray_popup(tray.app_handle());
                toggle_main_window(tray.app_handle());
            }
            TrayIconEvent::Click {
                button: MouseButton::Right,
                button_state: MouseButtonState::Up,
                position,
                ..
            } => {
                toggle_tray_popup(tray.app_handle(), position);
            }
            _ => {}
        });

    if let Some(icon) = app.default_window_icon().cloned() {
        builder = builder.icon(icon);
    }

    builder.build(app)?;
    Ok(())
}

fn ensure_tray_popup(app: &AppHandle) -> tauri::Result<tauri::WebviewWindow> {
    if let Some(existing) = app.get_webview_window(TRAY_POPUP_LABEL) {
        return Ok(existing);
    }
    let popup = WebviewWindowBuilder::new(
        app,
        TRAY_POPUP_LABEL,
        WebviewUrl::App("index.html?view=tray".into()),
    )
    .title("Portline Tray")
    .inner_size(POPUP_LOGICAL_W, POPUP_LOGICAL_H)
    .resizable(false)
    .decorations(false)
    .shadow(true)
    .always_on_top(true)
    .skip_taskbar(true)
    .visible(false)
    .build()?;

    let handle = app.clone();
    popup.on_window_event(move |event| match event {
        WindowEvent::Focused(false) => {
            if let Some(w) = handle.get_webview_window(TRAY_POPUP_LABEL) {
                let _ = w.hide();
            }
        }
        WindowEvent::CloseRequested { api, .. } => {
            api.prevent_close();
            if let Some(w) = handle.get_webview_window(TRAY_POPUP_LABEL) {
                let _ = w.hide();
            }
        }
        _ => {}
    });

    Ok(popup)
}

fn toggle_tray_popup(app: &AppHandle, click_pos: PhysicalPosition<f64>) {
    let Ok(popup) = ensure_tray_popup(app) else {
        return;
    };
    if popup.is_visible().unwrap_or(false) {
        let _ = popup.hide();
        return;
    }

    // Position popup anchored neatly above (or below) the taskbar tray icon within monitor bounds.
    let monitor = app
        .available_monitors()
        .ok()
        .and_then(|list| {
            list.into_iter().find(|m| {
                let pos = m.position();
                let size = m.size();
                click_pos.x >= pos.x as f64
                    && click_pos.x <= (pos.x + size.width as i32) as f64
                    && click_pos.y >= pos.y as f64
                    && click_pos.y <= (pos.y + size.height as i32) as f64
            })
        })
        .or_else(|| popup.current_monitor().ok().flatten())
        .or_else(|| app.primary_monitor().ok().flatten());

    if let Some(mon) = monitor {
        let scale = mon.scale_factor();
        let work = mon.work_area();
        let margin = (10.0 * scale).round() as i32;
        let pop_w = (POPUP_LOGICAL_W * scale).round() as i32;
        let pop_h = (POPUP_LOGICAL_H * scale).round() as i32;

        let min_x = work.position.x + margin;
        let max_x = (work.position.x + work.size.width as i32 - pop_w - margin).max(min_x);
        let desired_x = click_pos.x.round() as i32 - pop_w / 2;
        let x = desired_x.clamp(min_x, max_x);

        let min_y = work.position.y + margin;
        let max_y = (work.position.y + work.size.height as i32 - pop_h - margin).max(min_y);
        let mid_y = work.position.y + (work.size.height as i32) / 2;
        let desired_y = if (click_pos.y as i32) >= mid_y {
            max_y.min(click_pos.y.round() as i32 - pop_h - margin)
        } else {
            min_y.max(click_pos.y.round() as i32 + margin)
        };
        let y = desired_y.clamp(min_y, max_y);
        let _ = popup.set_position(PhysicalPosition::new(x, y));
    }

    let _ = app.emit("portline-state-sync", ());
    let _ = popup.show();
    let _ = popup.unminimize();
    let _ = popup.set_focus();
}

pub fn hide_tray_popup(app: &AppHandle) {
    if let Some(w) = app.get_webview_window(TRAY_POPUP_LABEL) {
        let _ = w.hide();
    }
}

pub fn show_main_window(app: &AppHandle) {
    hide_tray_popup(app);
    if let Some(win) = app.get_webview_window("main") {
        let _ = win.show();
        let _ = win.unminimize();
        let _ = win.set_focus();
    }
}

pub fn toggle_main_window(app: &AppHandle) {
    if let Some(win) = app.get_webview_window("main") {
        let visible = win.is_visible().unwrap_or(false);
        let minimized = win.is_minimized().unwrap_or(false);
        if visible && !minimized {
            let _ = win.hide();
        } else {
            let _ = win.show();
            let _ = win.unminimize();
            let _ = win.set_focus();
        }
    }
}

pub fn update_tray_tooltip(app: &AppHandle, text: &str) {
    if let Some(tray) = app.tray_by_id(TRAY_ID) {
        let _ = tray.set_tooltip(Some(text));
    }
}
