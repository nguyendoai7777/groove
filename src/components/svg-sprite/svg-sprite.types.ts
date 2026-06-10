export type SVGSrc =
  | 'VMute'
  | 'AngleLeft'
  | 'AngleRight'
  | 'Close'
  | 'Minus'
  | 'ArrowsExpand'
  | 'ArrowsCompress'

export interface SVGSpriteProps {
  src: CastString<SVGSrc>
  className?: string
}
