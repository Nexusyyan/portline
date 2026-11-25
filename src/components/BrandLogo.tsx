interface BrandLogoProps {
  className?: string;
}

export function BrandLogo({ className = "h-[18px] w-[18px]" }: BrandLogoProps) {
  return (
    <svg
      className={className}
      viewBox="0 0 32 32"
      fill="none"
      stroke="currentColor"
      strokeWidth="2.2"
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden="true"
    >
      <circle cx="8" cy="10" r="2.6" />
      <circle cx="8" cy="22" r="2.6" />
      <circle cx="24" cy="16" r="2.6" />
      <path d="M10.6 10H15C17.8 10 18.2 16 21.4 16" />
      <path d="M10.6 22H15C17.8 22 18.2 16 21.4 16" />
    </svg>
  );
}
