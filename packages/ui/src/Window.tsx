import type { HTMLAttributes, ReactNode, Ref } from 'react'

type Props = Omit<HTMLAttributes<HTMLElement>, 'title'> & {
  title: ReactNode
  /** Props de la barra de título, por ejemplo para arrastrar la ventana. */
  barProps?: HTMLAttributes<HTMLDivElement>
  bodyClassName?: string
  as?: 'div' | 'article' | 'section'
  ref?: Ref<HTMLElement>
}

/** Ventana con barra de título, como las del panel de la app. */
export function Window({ title, barProps, bodyClassName, as: Tag = 'div', className, children, ref, ...rest }: Props) {
  return (
    <Tag ref={ref as never} className={['ms-win', className].filter(Boolean).join(' ')} {...rest}>
      <div {...barProps} className={['ms-win-bar', barProps?.className].filter(Boolean).join(' ')}>
        <span>{title}</span>
        <span className="ms-dots" aria-hidden="true"><i /><i /><i /></span>
      </div>
      <div className={['ms-win-body', bodyClassName].filter(Boolean).join(' ')}>{children}</div>
    </Tag>
  )
}
