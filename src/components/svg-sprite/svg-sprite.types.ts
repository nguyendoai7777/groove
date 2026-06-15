export type SVGSrc =
  | 'VMute'
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

export interface SVGSpriteProps {
  src: CastString<SVGSrc>
  className?: string
}
