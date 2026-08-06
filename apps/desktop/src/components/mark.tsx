/**
 * The Nirmoka mark, as the window's own drawing of it.
 *
 * निर्मोक is the skin a snake sheds: the N's diagonal is broken once and the
 * upper piece has slid clear. `assets/nirmoka-mark.svg` is the source of truth
 * and what `scripts/generate-icons.sh` rasterizes for the bundle — this is the
 * same geometry, inline, so the sidebar shows the logo rather than a letter.
 * Edit both together.
 */
export function NirmokaMark({ className }: { className?: string }) {
  return (
    <svg viewBox="0 0 512 512" className={className} role="img" aria-label="Nirmoka">
      <defs>
        <linearGradient id="nirmoka-mark-bg" x1="0" y1="0" x2="0" y2="1">
          <stop offset="0" stopColor="#8B5CF6" />
          <stop offset="1" stopColor="#6D28D9" />
        </linearGradient>
      </defs>
      <rect width="512" height="512" rx="114" fill="url(#nirmoka-mark-bg)" />
      <rect x="150" y="142" width="46" height="228" rx="23" fill="#FFFFFF" />
      <rect x="316" y="142" width="46" height="228" rx="23" fill="#FFFFFF" />
      <path
        d="M240 240 L338 338"
        fill="none"
        stroke="#FFFFFF"
        strokeWidth="46"
        strokeLinecap="round"
      />
      <path
        d="M230 162 L260 192"
        fill="none"
        stroke="#EDE9FE"
        strokeOpacity="0.6"
        strokeWidth="46"
        strokeLinecap="round"
      />
    </svg>
  );
}
