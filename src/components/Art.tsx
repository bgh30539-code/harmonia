import { useEffect, useState } from "react";
import { Music } from "lucide-react";
import { artwork } from "../api";

interface ArtProps {
  hash: string | null;
  alt?: string;
  className?: string;
}

/** Renders cached artwork, falling back to a gradient placeholder. */
export function Art({ hash, alt = "", className = "" }: ArtProps) {
  const [url, setUrl] = useState<string | null>(null);
  const [loaded, setLoaded] = useState(false);

  useEffect(() => {
    let cancelled = false;
    setLoaded(false);
    setUrl(null);
    if (!hash) return;
    artwork(hash).then((u) => {
      if (!cancelled) setUrl(u);
    });
    return () => {
      cancelled = true;
    };
  }, [hash]);

  if (url) {
    return (
      <img
        src={url}
        alt={alt}
        className={className}
        loading="lazy"
        onLoad={() => setLoaded(true)}
        style={{ opacity: loaded ? 1 : 0 }}
      />
    );
  }
  return (
    <div className={`art-placeholder ${className}`} aria-label={alt}>
      <Music size="38%" strokeWidth={1.2} />
    </div>
  );
}
