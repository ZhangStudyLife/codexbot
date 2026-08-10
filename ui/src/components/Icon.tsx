import type { SVGProps } from "react";

export type IconName =
  | "activity"
  | "alert"
  | "bell"
  | "check"
  | "chevron"
  | "close"
  | "code"
  | "copy"
  | "eye"
  | "link"
  | "lock"
  | "logo"
  | "overview"
  | "pause"
  | "play"
  | "qq"
  | "queue"
  | "refresh"
  | "shield"
  | "stop"
  | "user";

interface IconProps extends Omit<SVGProps<SVGSVGElement>, "name"> {
  name: IconName;
}

export function Icon({ name, ...props }: IconProps) {
  return (
    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeLinecap="round" strokeLinejoin="round" strokeWidth="1.8" aria-hidden="true" {...props}>
      {name === "logo" ? <><path d="M4.8 7.8 12 3.7l7.2 4.1v8.3L12 20.3l-7.2-4.2V7.8Z" /><path d="m8.1 9.9 3.9 2.3 3.9-2.3M12 12.2v4.5" /></> : null}
      {name === "overview" ? <path d="M4 13h6V4H4v9Zm0 7h6v-4H4v4Zm10 0h6v-9h-6v9Zm0-16v4h6V4h-6Z" /> : null}
      {name === "link" ? <><path d="m8.5 14.5 7-7" /><path d="m6.3 17.7-1.4 1.4a2.8 2.8 0 0 1-4-4l4.2-4.2a2.8 2.8 0 0 1 4 0M17.7 6.3l1.4-1.4a2.8 2.8 0 1 1 4 4l-4.2 4.2a2.8 2.8 0 0 1-4 0" /></> : null}
      {name === "bell" ? <><path d="M18 8a6 6 0 0 0-12 0c0 7-3 7-3 9h18c0-2-3-2-3-9Z" /><path d="M10 21h4" /></> : null}
      {name === "activity" ? <path d="M4 19V5m0 14h16M8 15l3-4 3 2 5-7" /> : null}
      {name === "shield" ? <><path d="M12 3 5 6v5c0 4.7 2.8 8.4 7 10 4.2-1.6 7-5.3 7-10V6l-7-3Z" /><path d="m9 12 2 2 4-4" /></> : null}
      {name === "refresh" ? <path d="M20 11a8 8 0 1 0-2.3 5.7M20 4v7h-7" /> : null}
      {name === "chevron" ? <path d="m9 18 6-6-6-6" /> : null}
      {name === "code" ? <><path d="m8 9-3 3 3 3m8-6 3 3-3 3" /><path d="m14 6-4 12" /></> : null}
      {name === "qq" ? <><path d="M12 3.5c-4.1 0-6.7 3.4-6.7 7.7 0 1.7.4 3.3 1.2 4.6L5 19.2l3.7-.7c1 .5 2.1.8 3.3.8 1.2 0 2.4-.3 3.4-.8l3.6.7-1.5-3.4c.8-1.3 1.2-2.9 1.2-4.6 0-4.3-2.6-7.7-6.7-7.7Z" /><path d="M8.8 11c.4-.7 1.1-1 1.8-1h2.8c.7 0 1.4.3 1.8 1" /></> : null}
      {name === "user" ? <><circle cx="12" cy="8" r="4" /><path d="M4.5 21a7.5 7.5 0 0 1 15 0" /></> : null}
      {name === "queue" ? <path d="M5 12h14m-7-7 7 7-7 7" /> : null}
      {name === "check" ? <path d="m5 12 4 4L19 6" /> : null}
      {name === "alert" ? <><path d="M12 9v4m0 4h.01" /><path d="M10.3 4.8 2.8 18a2 2 0 0 0 1.7 3h15a2 2 0 0 0 1.7-3L13.7 4.8a2 2 0 0 0-3.4 0Z" /></> : null}
      {name === "pause" ? <path d="M8 5v14m8-14v14" /> : null}
      {name === "lock" ? <><path d="M7 10V7a5 5 0 0 1 10 0v3M5 10h14v11H5Z" /><path d="M12 15v2" /></> : null}
      {name === "copy" ? <><rect x="8" y="8" width="11" height="11" rx="2" /><path d="M16 8V6a2 2 0 0 0-2-2H6a2 2 0 0 0-2 2v8a2 2 0 0 0 2 2h2" /></> : null}
      {name === "eye" ? <><path d="M2 12s3.5-6 10-6 10 6 10 6-3.5 6-10 6S2 12 2 12Z" /><circle cx="12" cy="12" r="2.5" /></> : null}
      {name === "close" ? <path d="m6 6 12 12M18 6 6 18" /> : null}
      {name === "play" ? <path d="m8 5 11 7-11 7V5Z" /> : null}
      {name === "stop" ? <rect x="6" y="6" width="12" height="12" rx="2" /> : null}
    </svg>
  );
}
