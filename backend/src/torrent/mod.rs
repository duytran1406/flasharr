pub(crate) mod evidence;
mod info_hash;
pub(crate) mod metainfo;
mod normalize;
pub(crate) use info_hash::{extract_btih, normalize_info_hash, torrent_info_hash};
#[allow(unused_imports)]
pub(crate) use normalize::{
    build_magnet_url, derive_info_hash_from_resolved_torrent_url,
    derive_info_hash_from_torrent_url, is_upload_torrent_url, materialize_download,
    normalize_torrent_url, resolve_download_ref, MaterializedDownload, TorrentAddInput,
};
pub mod instance_manager;
pub mod qbittorrent;
pub mod state;
