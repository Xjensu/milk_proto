import type { ButtonHTMLAttributes, PropsWithChildren } from "react";

export function Button({ className = "", children, ...props }: ButtonHTMLAttributes<HTMLButtonElement>) {
  return <button className={`button ${className}`} {...props}>{children}</button>;
}

export function Badge({ children, tone = "neutral" }: PropsWithChildren<{ tone?: "neutral" | "positive" | "accent" }>) {
  return <span className={`badge badge-${tone}`}>{children}</span>;
}

export function Panel({ children, className = "" }: PropsWithChildren<{ className?: string }>) {
  return <section className={`panel ${className}`}>{children}</section>;
}
