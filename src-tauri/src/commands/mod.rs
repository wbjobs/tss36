pub mod files;
pub mod versions;
pub mod search;
pub mod watcher;
pub mod sync;

pub use files::{get_file_tree, read_file_content, FileNode};
pub use versions::{get_file_versions, get_file_version_content, restore_version, VersionInfo};
pub use search::{semantic_search, SearchFiltersDto, SearchResultDto};
pub use watcher::{watch_folder_v2 as watch_folder, stop_watching, get_watched_folder};
pub use sync::{
    start_sync_server, stop_sync_server, connect_to_server, disconnect_sync,
    publish_local_versions, pull_remote_versions, get_sync_stats, get_conflicts,
    resolve_conflict, get_connected_peers,
};
