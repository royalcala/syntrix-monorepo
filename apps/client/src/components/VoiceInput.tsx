import { useState, useRef, useCallback, useEffect } from "react";
import { Button } from "@syntrix/ui/components/ui/button";
import { Mic, Loader2 } from "lucide-react";

interface VoiceInputProps {
  onTranscript: (text: string) => void;
  disabled?: boolean;
}

interface SpeechResult {
  transcript: string;
}

interface SpeechRecognitionEvent {
  results: SpeechResult[][];
}

interface SpeechRecognition {
  lang: string;
  continuous: boolean;
  interimResults: boolean;
  maxAlternatives: number;
  start: () => void;
  abort: () => void;
  onresult: ((event: SpeechRecognitionEvent) => void) | null;
  onerror: (() => void) | null;
  onend: (() => void) | null;
}

function createRecognition(): SpeechRecognition | null {
  const ctor = (window as any).SpeechRecognition || (window as any).webkitSpeechRecognition;
  return ctor ? new ctor() as SpeechRecognition : null;
}

export function VoiceInput({ onTranscript, disabled }: VoiceInputProps) {
  const [listening, setListening] = useState(false);
  const [supported, setSupported] = useState(true);
  const ref = useRef<SpeechRecognition | null>(null);

  useEffect(() => {
    const recognition = createRecognition();
    if (!recognition) { setSupported(false); return; }

    recognition.lang = "es-MX";
    recognition.continuous = false;
    recognition.interimResults = false;
    recognition.maxAlternatives = 1;

    recognition.onresult = (event) => {
      const transcript = event.results[0][0].transcript;
      onTranscript(transcript);
      setListening(false);
    };

    recognition.onerror = () => setListening(false);
    recognition.onend = () => setListening(false);

    ref.current = recognition;
    return () => { try { recognition.abort(); } catch {} };
  }, [onTranscript]);

  const toggle = useCallback(() => {
    const r = ref.current;
    if (!r) return;
    if (listening) { r.abort(); setListening(false); }
    else { r.start(); setListening(true); }
  }, [listening]);

  if (!supported) return null;

  return (
    <Button
      type="button"
      variant={listening ? "default" : "ghost"}
      size="icon"
      className={`h-8 w-8 shrink-0 ${listening ? "bg-red-500 hover:bg-red-600 animate-pulse text-white" : "text-muted-foreground"}`}
      onClick={toggle}
      disabled={disabled}
      title={listening ? "Grabando..." : "Activar voz"}
    >
      {listening ? <Loader2 size={14} className="animate-spin" /> : <Mic size={14} />}
    </Button>
  );
}
