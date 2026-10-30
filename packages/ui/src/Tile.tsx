import type { HTMLAttributes } from 'react'

type Props = HTMLAttributes<HTMLElement> & {
  as?: 'article' | 'footer' | 'li' | 'div'
  focused?: boolean
}

/** Ventana de un mosaico: sin barra, borde que marca el foco. */
export function Tile({ as: Tag = 'article', focused, className, ...rest }: Props) {
  return <Tag className={['ms-tile', focused && 'is-focus', className].filter(Boolean).join(' ')} {...rest} />
}
