use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;
use tauri::{AppHandle, Manager};

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct GlobalSettings {
    pub recent_workspaces: Vec<String>,
    pub last_opened_path: Option<String>,
}

pub struct SettingsState {
    pub settings: Mutex<GlobalSettings>,
    pub file_path: PathBuf,
}

impl SettingsState {
    pub fn new(app_handle: &AppHandle) -> Self {
        // Resolve: AppData/code.blast.radius/settings.json
        let config_dir = app_handle.path().app_config_dir().unwrap();
        let _ = fs::create_dir_all(&config_dir);
        let file_path = config_dir.join("settings.json");

        let settings = if file_path.exists() {
            let content = fs::read_to_string(&file_path).unwrap_or_default();
            serde_json::from_str(&content).unwrap_or_default()
        } else {
            GlobalSettings::default()
        };

        Self {
            settings: Mutex::new(settings),
            file_path,
        }
    }

    pub fn save(&self) -> Result<(), String> {
        let settings = self.settings.lock().unwrap();
        let json = serde_json::to_string_pretty(&*settings).map_err(|e| e.to_string())?;
        fs::write(&self.file_path, json).map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn update_recent(&self, path: String) {
        let mut settings = self.settings.lock().unwrap();
        
        // Update Last Opened
        settings.last_opened_path = Some(path.clone());

        // Update Recents (Remove if exists, push to front, limit to 10)
        settings.recent_workspaces.retain(|p| p != &path);
        settings.recent_workspaces.insert(0, path);
        if settings.recent_workspaces.len() > 10 {
            settings.recent_workspaces.truncate(10);
        }
        
        // Release lock before saving to avoid deadlocks (save re-locks internally if we weren't careful, 
        // but here save() takes &self and locks again. To be safe, we drop the guard explicitly).
        drop(settings); 
        
        let _ = self.save();
    }
    
    pub fn clear_recent(&self) {
        let mut settings = self.settings.lock().unwrap();
        settings.recent_workspaces.clear();
        settings.last_opened_path = None;
        drop(settings);
        let _ = self.save();
    }

    pub fn remove_recent(&self, path: &str) {
        let mut settings = self.settings.lock().unwrap();
        
        // Retain only paths that do NOT match the one being removed
        settings.recent_workspaces.retain(|p| p != path);
        
        // If the removed path was the last opened, clear it
        if let Some(last) = &settings.last_opened_path {
            if last == path {
                settings.last_opened_path = None;
            }
        }

        drop(settings); // Explicit unlock before saving
        let _ = self.save();
    }
}