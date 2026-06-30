import { useState, useCallback, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen, UnlistenFn } from "@tauri-apps/api/event";

export interface ChatMessage {
  id: string;
  role: "user" | "assistant" | "tool";
  content: string;
  timestamp: number;
  isStreaming?: boolean;
}

export interface ProviderConfig {
  base_url: string;
  model: string;
  api_key?: string;
}

interface StreamEvent {
  type: "token" | "tool_call" | "tool_result" | "done" | "error";
  content?: string;
  name?: string;
  args?: unknown;
  result?: string;
  message?: string;
}

export function useAiChat(orgId: string) {
  const [messages, setMessages] = useState<ChatMessage[]>([]);
  const [isLoading, setIsLoading] = useState(false);
  const unlistenRef = useRef<UnlistenFn | null>(null);
  const currentAssistantId = useRef<string | null>(null);

  const sendMessage = useCallback(async (
    text: string,
    providerConfig?: ProviderConfig,
  ) => {
    const userMsg: ChatMessage = {
      id: crypto.randomUUID(),
      role: "user",
      content: text,
      timestamp: Date.now(),
    };

    const assistantId = crypto.randomUUID();
    currentAssistantId.current = assistantId;

    const assistantMsg: ChatMessage = {
      id: assistantId,
      role: "assistant",
      content: "",
      timestamp: Date.now(),
      isStreaming: true,
    };

    setMessages(prev => [...prev, userMsg, assistantMsg]);
    setIsLoading(true);

    const apiMessages = messages
      .filter(m => m.role !== "tool")
      .concat(userMsg)
      .map(m => ({
        role: m.role,
        content: m.role === "assistant" && m.isStreaming ? null : m.content,
      }));

    if (unlistenRef.current) {
      unlistenRef.current();
    }

    const unlisten = await listen<StreamEvent>("ai_chat_event", (event) => {
      const payload = event.payload;

      switch (payload.type) {
        case "token":
          setMessages(prev =>
            prev.map(m =>
              m.id === assistantId
                ? { ...m, content: m.content + (payload.content || "") }
                : m
            )
          );
          break;

        case "tool_call":
          setMessages(prev => {
            const last = prev[prev.length - 1];
            if (last && last.role === "assistant" && last.id === assistantId) {
              return prev.map(m =>
                m.id === assistantId
                  ? {
                      ...m,
                      content: m.content + `\n[🔧 Using ${payload.name}...]`,
                    }
                  : m
              );
            }
            return prev;
          });
          break;

        case "done":
          setMessages(prev =>
            prev.map(m =>
              m.id === assistantId
                ? { ...m, isStreaming: false }
                : m
            )
          );
          setIsLoading(false);
          if (unlistenRef.current) {
            unlistenRef.current();
            unlistenRef.current = null;
          }
          break;

        case "error":
          setMessages(prev =>
            prev.map(m =>
              m.id === assistantId
                ? { ...m, content: payload.message || "Error", isStreaming: false }
                : m
            )
          );
          setIsLoading(false);
          if (unlistenRef.current) {
            unlistenRef.current();
            unlistenRef.current = null;
          }
          break;
      }
    });

    unlistenRef.current = unlisten;

    try {
      await invoke("ai_chat", {
        orgId,
        messages: apiMessages,
        providerConfig: providerConfig ?? null,
      });
    } catch (err) {
      setMessages(prev =>
        prev.map(m =>
          m.id === assistantId
            ? { ...m, content: `Error: ${err}`, isStreaming: false }
            : m
        )
      );
      setIsLoading(false);
      if (unlistenRef.current) {
        unlistenRef.current();
        unlistenRef.current = null;
      }
    }
  }, [orgId, messages]);

  const clearMessages = useCallback(() => {
    setMessages([]);
  }, []);

  return { messages, isLoading, sendMessage, clearMessages };
}
