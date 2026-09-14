export interface Song {
  id: number;
  title: string | null;
  artist: string | null;
  album_id: number | null;
  folder_id: number;
  file_path: string;
  filename: string;
  duration: number;
  lyrics?: string | null;
  timeline?: string | null;
}

/**
 * Every tag field the editor can write. All optional, and the distinction matters:
 * `undefined` means "not supplied" (inherited from the file in `clean_keep` mode),
 * while `''` means "explicitly clear this field" — that is how a stubborn value
 * such as copyright gets deleted rather than carried forward.
 */
export interface SongMetadata {
  title?: string;
  artist?: string;
  album_name?: string;
  /** TPE2. The key iTunes, Plex and the Windows shell actually group albums by. */
  album_artist?: string;
  track_number?: string;
  track_total?: string;
  disc_number?: string;
  disc_total?: string;
  year?: string;
  genre?: string;
  composer?: string;
  publisher?: string;
  copyright?: string;
  comment?: string;
  bpm?: string;
  grouping?: string;
  lyrics?: string;
  /** TCMP. Set across a multi-artist album so iTunes keeps its tracks together. */
  compilation?: boolean;
  /** APIC, as a data: URI. `''` removes the cover. */
  thumbnail?: string;
}

/**
 * How a save treats frames already in the file.
 * - `clean_keep` — drop junk frames, keep known fields, apply the edits (default).
 * - `clean` — write only what the form contains; everything else is discarded.
 * - `merge` — touch only edited fields and leave the rest of the tag alone.
 */
export type MetadataWriteMode = 'clean_keep' | 'clean' | 'merge';

/** A frame exactly as it sits in the file, for the inspector panel. */
export interface RawFrame {
  id: string;
  name: string;
  value: string;
  /** False for frames a clean write would drop. */
  known: boolean;
}

export interface FullMetadata extends SongMetadata {
  filename: string;
  file_path: string;
  timeline?: string | null;
  raw_frames: RawFrame[];
}

export interface BulkMetadataResult {
  updated: number;
  failed: string[];
}

/** Payload of the `library-synced` event, emitted when watched folders change. */
export interface LibrarySyncReport {
  added: number;
  updated: number;
  removed: number;
}

/** A folder being kept in sync by the watcher. */
export interface WatchedRoot {
  path: string;
  /** False when the folder has been moved or unplugged. */
  exists: boolean;
}
