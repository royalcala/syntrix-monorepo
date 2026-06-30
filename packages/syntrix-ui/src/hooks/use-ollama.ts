import { useState, useEffect } from "react";

export interface OllamaStatus {
  available: boolean;
  checking: boolean;
  error?: string;
}

export function useOllama(): OllamaStatus {
  const [status, setStatus] = useState<OllamaStatus>({
    available: false,
    checking: true,
  });

  useEffect(() => {
    let cancelled = false;

    async function check() {
      try {
        const res = await fetch("http://localhost:11434/api/tags", {
          signal: AbortSignal.timeout(3000),
        });
        if (!cancelled) {
          setStatus({ available: res.ok, checking: false });
        }
      } catch (err) {
        if (!cancelled) {
          setStatus({
            available: false,
            checking: false,
            error: err instanceof Error ? err.message : "Ollama no detectado",
          });
        }
      }
    }

    check();
    return () => { cancelled = true; };
  }, []);

  return status;
}
