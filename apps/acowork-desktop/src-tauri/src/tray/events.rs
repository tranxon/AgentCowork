//! Tray event handlers

use crate::state::AppState;
use tauri::{AppHandle, Manager, menu::MenuEvent, tray::TrayIconEvent};

/// Handle tray menu events
pub fn on_menu_event(app: &AppHandle, event: MenuEvent) {
    match event.id().as_ref() {
        "show_dashboard" | "agent_chat" => {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.show();
                let _ = window.set_focus();
            }
        }
        "quit" => {
            // Kill local Gateway process tree before exit.
            // On Windows, taskkill /T /F kills the Gateway AND all its child
            // processes (Runtime + Embed) in one shot, preventing orphans.
            // On Unix, kill -INT sends SIGINT which triggers the Gateway's
            // ctrl_c handler to clean up children before exiting.
            let state = app.state::<AppState>();
            let gateway_handle = state.gateway_process.clone();
            tauri::async_runtime::spawn(async move {
                if let Ok(mut proc) = gateway_handle.try_lock() {
                    if let Some(child) = proc.take() {
                        let pid = child.id();
                        tracing::info!(pid = pid, "Killing Gateway process tree on quit");
                        #[cfg(target_os = "windows")]
                        {
                            let _ = std::process::Command::new("taskkill")
                                .args(["/PID", &pid.to_string(), "/T", "/F"])
                                .output();
                        }
                        #[cfg(not(target_os = "windows"))]
                        {
                            // Send SIGINT so Gateway's signal handler cleans up children
                            let _ = std::process::Command::new("kill")
                                .args(["-INT", &pid.to_string()])
                                .output();
                        }
                        // Reap the child process
                        let mut child = child; // Child::wait needs &mut
                        let _ = child.wait();
                    }
                }
            });
            // Give a short moment for the kill to propagate
            std::thread::sleep(std::time::Duration::from_millis(500));
            app.exit(0);
        }
        _ => {}
    }
}

/// Handle tray icon click events
pub fn on_tray_icon_event(tray: &tauri::tray::TrayIcon, event: TrayIconEvent) {
    if let TrayIconEvent::Click { .. } = event {
        let app = tray.app_handle();
        if let Some(window) = app.get_webview_window("main") {
            let _ = window.show();
            let _ = window.set_focus();
        }
    }
}
