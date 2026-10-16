import type { HTMLAttributes } from "react";

type SectionPanelProps = HTMLAttributes<HTMLElement> & {
  title: string;
  description: string;
};

export default function SectionPanel({ title, description, children, ...props }: SectionPanelProps) {
  return <article {...props}><div><h2>{title}</h2><p>{description}</p>{children}</div></article>;
}
