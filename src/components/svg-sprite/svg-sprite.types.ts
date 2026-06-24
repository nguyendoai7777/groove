export type SVGSrc =
  | 'VMute'
  | 'VMin'
  | 'VMedium'
  | 'VMax'
  | 'AngleLeft'
  | 'AngleRight'
  | 'Close'
  | 'Minus'
  | 'ArrowsExpand'
  | 'ArrowsCompress'
  | 'Command'
  | 'Song'
  | 'Artist'
  | 'Album'
  | 'Action'
  | 'Search'
  | 'Settings'
  | 'Play'
  | 'Folder'
  | 'Shuffle'
  | 'Loop'
  | 'LoopBadge'
  | 'Next'
  | 'Prev'
  | 'Delete'
  | 'Edit'
  | 'LyricsBubble'
  | 'HelpCircle'
  | 'NowPlaying';

export interface SVGSpriteProps {
  src: CastString<SVGSrc>;
  className?: string;
}
