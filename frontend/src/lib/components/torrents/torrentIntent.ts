/**
 * What the /torrents page asks TorrentActionModals to show. The queue table,
 * the inspector and the settings hub open one intent at a time; `null` closes it.
 */
import type { UnifiedTorrentRow } from '$lib/stores/qbitReducer';

export type TorrentRenameMode = 'torrent' | 'file' | 'folder';

export type TorrentIntent =
  | { kind: 'add' }
  | { kind: 'create' }
  | { kind: 'prefs' }
  | { kind: 'labels'; mode: 'category' | 'tags'; torrent?: UnifiedTorrentRow | null }
  | { kind: 'delete'; torrent?: UnifiedTorrentRow | null }
  | { kind: 'limits'; torrent?: UnifiedTorrentRow | null }
  | { kind: 'move'; torrent?: UnifiedTorrentRow | null }
  | {
      kind: 'rename';
      mode: TorrentRenameMode;
      hash: string;
      label: string;
      currentName?: string;
      oldPath?: string;
    }
  | { kind: 'cross-seed'; torrent: UnifiedTorrentRow };
