use std::path::PathBuf;
use std::fs;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FileDialogMode {
    Open,
    Save,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FileDialogPurpose {
    SaveLayout,
    LoadLayout,
    SaveThenNew,
    SaveThenQuit,
    SaveThenFollowLink,
    ExportText,
    ExportJson,
}

#[derive(Debug, Clone)]
pub struct FileEntry {
    pub name:   String,
    pub is_dir: bool,
    pub path:   PathBuf,
}

#[derive(Debug, Clone)]
pub struct FileDialogState {
    pub mode:           FileDialogMode,
    pub purpose:        FileDialogPurpose,
    pub current_dir:    PathBuf,
    pub entries:        Vec<FileEntry>,
    pub selected:       usize,
    pub filename_input: String,
    pub focus_input:    bool,
    pub error_msg:      Option<String>,
}

impl FileDialogState {
    pub fn new(
        mode:    FileDialogMode,
        purpose: FileDialogPurpose,
        start_dir: PathBuf,
        initial_filename: &str,
    ) -> Self {
        let mut s = Self {
            mode,
            purpose,
            current_dir: start_dir,
            entries: Vec::new(),
            selected: 0,
            filename_input: initial_filename.to_string(),
            focus_input: false,
            error_msg: None,
        };
        s.reload_entries();
        s
    }

    pub fn reload_entries(&mut self) {
        let mut dirs:  Vec<FileEntry> = Vec::new();
        let mut files: Vec<FileEntry> = Vec::new();

        // Synthetic parent-directory entry
        if let Some(parent) = self.current_dir.parent() {
            dirs.push(FileEntry {
                name:   "..".to_string(),
                is_dir: true,
                path:   parent.to_path_buf(),
            });
        }

        if let Ok(rd) = fs::read_dir(&self.current_dir) {
            for e in rd.flatten() {
                let path = e.path();
                let name = e.file_name().to_string_lossy().to_string();
                if path.is_dir() {
                    dirs.push(FileEntry { name, is_dir: true, path });
                } else {
                    files.push(FileEntry { name, is_dir: false, path });
                }
            }
        }

        // Sort: dirs (excluding ..) by name, then files by name
        if dirs.len() > 1 {
            dirs[1..].sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
        }
        files.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));

        self.entries = dirs;
        self.entries.extend(files);
        self.selected = self.selected.min(self.entries.len().saturating_sub(1));
    }

    pub fn nav_up(&mut self) {
        if self.selected > 0 {
            self.selected -= 1;
        }
    }

    pub fn nav_down(&mut self) {
        if self.selected + 1 < self.entries.len() {
            self.selected += 1;
        }
    }

    pub fn go_parent(&mut self) {
        if let Some(parent) = self.current_dir.parent().map(|p| p.to_path_buf()) {
            self.current_dir = parent;
            self.selected = 0;
            self.reload_entries();
        }
    }

    pub fn enter_selected(&mut self) -> EnterResult {
        let Some(entry) = self.entries.get(self.selected) else {
            return EnterResult::None;
        };
        if entry.is_dir {
            let path = entry.path.clone();
            self.current_dir = path;
            self.selected = 0;
            self.reload_entries();
            EnterResult::EnteredDir
        } else {
            EnterResult::SelectedFile(entry.path.clone())
        }
    }

    /// Populate filename_input from the selected file (for save mode).
    pub fn populate_filename_from_selection(&mut self) {
        if let Some(entry) = self.entries.get(self.selected) {
            if !entry.is_dir {
                self.filename_input = entry.name.clone();
            }
        }
    }

    /// Full path for saving, auto-appending `.json` if no extension.
    pub fn save_path(&self) -> Option<PathBuf> {
        let name = self.filename_input.trim();
        if name.is_empty() {
            return None;
        }
        let name = if std::path::Path::new(name).extension().is_some() {
            name.to_string()
        } else {
            format!("{name}.json")
        };
        Some(self.current_dir.join(name))
    }
}

pub enum EnterResult {
    None,
    EnteredDir,
    SelectedFile(PathBuf),
}
