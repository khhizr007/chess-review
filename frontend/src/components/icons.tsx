// Line icons used on the coach buttons and panel header.

const I = { width: 18, height: 18, viewBox: "0 0 24 24", "aria-hidden": true } as const;
export const Icons = {
  bulb: (
    <svg {...I}>
      <path fill="currentColor" d="M12 2.5a6.5 6.5 0 0 0-3.9 11.7c.6.5.9 1.1.9 1.8v.5h6V16c0-.7.3-1.3.9-1.8A6.5 6.5 0 0 0 12 2.5zM9.2 18.5h5.6v1a2 2 0 0 1-2 2h-1.6a2 2 0 0 1-2-2z" />
    </svg>
  ),
  star: (
    <svg {...I}>
      <circle cx="12" cy="12" r="10" fill="none" stroke="currentColor" strokeWidth="2" />
      <path fill="currentColor" d="M12 6.6l1.6 3.3 3.6.5-2.6 2.5.6 3.6-3.2-1.7-3.2 1.7.6-3.6-2.6-2.5 3.6-.5z" />
    </svg>
  ),
  retry: (
    <svg {...I}>
      <path fill="none" stroke="currentColor" strokeWidth="2.4" strokeLinecap="round" strokeLinejoin="round" d="M4.5 12a7.5 7.5 0 1 0 2.2-5.3M4.5 4v4.5H9" />
    </svg>
  ),
  next: (
    <svg {...I}>
      <path fill="none" stroke="currentColor" strokeWidth="2.6" strokeLinecap="round" strokeLinejoin="round" d="M4 12h15M13 6l6 6-6 6" />
    </svg>
  ),
  back: (
    <svg {...I}>
      <path fill="none" stroke="currentColor" strokeWidth="2.6" strokeLinecap="round" strokeLinejoin="round" d="M20 12H5M11 6l-6 6 6 6" />
    </svg>
  ),
  eye: (
    <svg {...I}>
      <path fill="none" stroke="currentColor" strokeWidth="2.2" d="M2.5 12S6 5.5 12 5.5 21.5 12 21.5 12 18 18.5 12 18.5 2.5 12 2.5 12z" />
      <circle cx="12" cy="12" r="3" fill="currentColor" />
    </svg>
  ),
  target: (
    <svg {...I}>
      <circle cx="12" cy="12" r="9" fill="none" stroke="currentColor" strokeWidth="2.2" />
      <circle cx="12" cy="12" r="4.5" fill="none" stroke="currentColor" strokeWidth="2.2" />
      <circle cx="12" cy="12" r="1.4" fill="currentColor" />
    </svg>
  ),
  prev: (
    <svg {...I}>
      <path fill="none" stroke="currentColor" strokeWidth="2.8" strokeLinecap="round" strokeLinejoin="round" d="M15 5l-7 7 7 7" />
    </svg>
  ),
  forward: (
    <svg {...I}>
      <path fill="none" stroke="currentColor" strokeWidth="2.8" strokeLinecap="round" strokeLinejoin="round" d="M9 5l7 7-7 7" />
    </svg>
  ),
  play: (
    <svg {...I}>
      <path fill="currentColor" d="M7.5 4.5v15L19.5 12z" />
    </svg>
  ),
  pause: (
    <svg {...I}>
      <path fill="currentColor" d="M7 5h3.5v14H7zM13.5 5H17v14h-3.5z" />
    </svg>
  ),
  sound: (
    <svg {...I} width={20} height={20}>
      <path fill="currentColor" d="M3.5 9h3.8L12 4.8v14.4L7.3 15H3.5z" />
      <path fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" d="M15.5 9a4 4 0 0 1 0 6M18 6.5a7.5 7.5 0 0 1 0 11" />
    </svg>
  ),
  muted: (
    <svg {...I} width={20} height={20}>
      <path fill="currentColor" d="M3.5 9h3.8L12 4.8v14.4L7.3 15H3.5z" />
      <path fill="none" stroke="currentColor" strokeWidth="2.2" strokeLinecap="round" d="M15.5 9.5l5 5M20.5 9.5l-5 5" />
    </svg>
  ),
  close: (
    <svg {...I}>
      <path fill="none" stroke="currentColor" strokeWidth="2.6" strokeLinecap="round" d="M6 6l12 12M18 6L6 18" />
    </svg>
  ),
  lines: (
    <svg {...I}>
      <path fill="none" stroke="currentColor" strokeWidth="2.4" strokeLinecap="round" d="M9 6.5h11M9 12h11M9 17.5h7" />
      <path fill="currentColor" d="M3.5 5h3v3h-3zm0 5.5h3v3h-3zm0 5.5h3v3h-3z" />
    </svg>
  ),
  report: (
    <svg {...I}>
      <path fill="currentColor" d="M5 20V10h3v10zm5.5 0V4h3v16zM16 20v-7h3v7z" />
    </svg>
  ),
};
