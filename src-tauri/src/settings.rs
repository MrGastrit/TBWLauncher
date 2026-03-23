use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use sysinfo::System;
use tauri::Manager;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LauncherSettings {
    pub ram_mb: i32,
    pub theme: String,
    pub java_args: String,
    #[serde(default = "default_auto_updates")]
    pub auto_updates: bool,
    pub close_on_launch: bool,
    pub show_logs: bool,
}

fn default_auto_updates() -> bool {
    true
}

impl Default for LauncherSettings {
    fn default() -> Self {
        Self {
            ram_mb: 4096,
            theme: "dark".to_string(),
            java_args: "-XX:+UseG1GC -XX:+UnlockExperimentalVMOptions".to_string(),
            auto_updates: default_auto_updates(),
            close_on_launch: false,
            show_logs: false,
        }
    }
}

#[tauri::command]
pub fn load_launcher_settings(app: tauri::AppHandle) -> Result<LauncherSettings, String> {
    let path = settings_path(&app)?;

    if !path.exists() {
        return Ok(LauncherSettings::default());
    }

    let raw = fs::read_to_string(&path)
        .map_err(|error| format!("Не удалось прочитать settings.json: {error}"))?;

    serde_json::from_str::<LauncherSettings>(&raw)
        .map_err(|error| format!("settings.json поврежден: {error}"))
}

#[tauri::command]
pub fn save_launcher_settings(
    app: tauri::AppHandle,
    settings: LauncherSettings,
) -> Result<(), String> {
    let path = settings_path(&app)?;

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("Не удалось создать папку настроек: {error}"))?;
    }

    let json = serde_json::to_string_pretty(&settings)
        .map_err(|error| format!("Не удалось сериализовать настройки: {error}"))?;

    fs::write(&path, json).map_err(|error| format!("Не удалось записать settings.json: {error}"))
}

#[tauri::command]
pub fn get_total_ram_mb() -> Result<i32, String> {
    let mut system = System::new();
    system.refresh_memory();

    let total_raw = system.total_memory();

    //
    let total_mb = if total_raw > 1024_u64 * 1024 * 1024 {
        total_raw / (1024 * 1024)
    } else {
        total_raw / 1024
    };

    i32::try_from(total_mb).map_err(|_| "Объем ОЗУ не помещается в i32".to_string())
}

fn settings_path(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    let config_dir = app
        .path()
        .app_config_dir()
        .map_err(|error| format!("Не удалось получить app_config_dir: {error}"))?;

    Ok(config_dir.join("settings.json"))
}
