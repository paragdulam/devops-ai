import { useEffect, useState } from "react";
import { formatElapsed } from "../lib/time";

export function RentalTimer({ since }: { since: string }) {
  const [elapsedMs, setElapsedMs] = useState(() => Date.now() - new Date(since).getTime());

  useEffect(() => {
    const interval = setInterval(() => {
      setElapsedMs(Date.now() - new Date(since).getTime());
    }, 1000);
    return () => clearInterval(interval);
  }, [since]);

  return <span className="rental-timer">Rental time: {formatElapsed(elapsedMs)}</span>;
}
