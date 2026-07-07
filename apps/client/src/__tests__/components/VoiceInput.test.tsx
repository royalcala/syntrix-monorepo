import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen } from "@testing-library/react";
import { VoiceInput } from "../../components/VoiceInput";

class MockSpeechRecognition {
  lang = "";
  continuous = false;
  interimResults = false;
  maxAlternatives = 1;
  start = vi.fn();
  abort = vi.fn();
  onresult: ((event: any) => void) | null = null;
  onerror: (() => void) | null = null;
  onend: (() => void) | null = null;
}

describe("VoiceInput", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("renders mic button when SpeechRecognition is available", () => {
    (window as any).SpeechRecognition = MockSpeechRecognition;
    render(<VoiceInput onTranscript={vi.fn()} />);
    expect(screen.getByRole("button")).toBeDefined();
  });

  it("renders nothing when SpeechRecognition is unavailable", () => {
    delete (window as any).SpeechRecognition;
    delete (window as any).webkitSpeechRecognition;
    const { container } = render(<VoiceInput onTranscript={vi.fn()} />);
    expect(container.textContent).toBe("");
  });
});
