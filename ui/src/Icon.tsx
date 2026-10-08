import type { ReactNode } from 'react';

export type IconName = 'leaf' | 'car' | 'grid' | 'code' | 'lock' | 'bolt' | 'arrow' | 'check' | 'search' | 'out' | 'shield' | 'alert' | 'heart' | 'pin' | 'clock';
export function Icon({ name, className = '' }: { name: IconName; className?: string }) {
  const paths: Record<IconName, ReactNode> = {
    heart: <><path d="M20.8 4.6a5.5 5.5 0 0 0-7.8 0L12 5.7l-1.1-1.1a5.5 5.5 0 0 0-7.8 7.8L12 21l8.8-8.6a5.5 5.5 0 0 0 0-7.8Z" /><path d="M3 12h5l2-4 4 8 2-4h5" /></>,
    pin: <><path d="M20 10c0 6-8 12-8 12S4 16 4 10a8 8 0 1 1 16 0Z" /><circle cx="12" cy="10" r="2.5" /></>,
    clock: <><circle cx="12" cy="12" r="9" /><path d="M12 7v5l3 2" /></>,
    leaf: <><path d="M19 4C11 2 4 8 5 15c7 4 15-1 14-11Z" /><path d="m4 21 10-11" /></>,
    car: <><path d="m5 9 2-5h10l2 5M3 10h18v8H3zM6 18v2m12-2v2M6 13h2m8 0h2" /></>,
    grid: <><rect x="3" y="3" width="7" height="7" rx="1" /><rect x="14" y="3" width="7" height="7" rx="1" /><rect x="3" y="14" width="7" height="7" rx="1" /><rect x="14" y="14" width="7" height="7" rx="1" /></>,
    code: <><path d="m8 6-6 6 6 6m8-12 6 6-6 6m-3-15-2 18" /></>,
    lock: <><rect x="5" y="10" width="14" height="11" rx="2" /><path d="M8 10V7a4 4 0 0 1 8 0v3m-4 5v2" /></>,
    bolt: <path d="m13 2-9 12h7l-1 8 10-13h-7z" />,
    arrow: <path d="M4 12h16m-6-6 6 6-6 6" />,
    check: <path d="m5 12 4 4L19 6" />,
    search: <><circle cx="10" cy="10" r="7" /><path d="m15 15 6 6" /></>,
    out: <><path d="M10 4H4v16h6m4-13 5 5-5 5m-5-5h10" /></>,
    shield: <><path d="m12 2 9 4v7c0 5-9 9-9 9S3 18 3 13V6z" /><path d="m8 12 3 3 5-6" /></>,
    alert: <><path d="m12 3 10 18H2zM12 9v5m0 3v.1" /></>,
  };
  return <svg className={`icon ${className}`} viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.65" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">{paths[name]}</svg>;
}
