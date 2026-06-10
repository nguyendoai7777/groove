import { SVGSrc } from '@groovex/ui/svg-sprite'

export interface IconBtnSize {
  icon?: CastString<'sm' | 'md' | 'lg'>
  box?: CastString<'sm' | 'md' | 'lg'>
}

export interface IconBtnProps {
  src: SVGSrc
  size?: IconBtnSize
}
