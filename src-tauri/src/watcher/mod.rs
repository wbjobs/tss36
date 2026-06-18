pub mod watcher;

pub use watcher::{
    WatcherController, FileEvent, FileEventType, start_watcher,
    scan_initial_folder, is_supported_file,
};
