import { useState, useRef, useEffect } from "react";
import { MessageCircle, X, Send, Settings, Bot, Sparkles } from "lucide-react";
import { Button } from "./ui/button";
import { ScrollArea } from "./ui/scroll-area";
import { useOllama } from "../hooks/use-ollama";
import { useAiChat, type ProviderConfig, type ChatMessage } from "../hooks/use-ai-chat";
import { cn } from "../lib/utils";

interface ChatPanelProps {
  orgId: string;
}

function MessageBubble({ message }: { message: ChatMessage }) {
  const isUser = message.role === "user";
  const isTool = message.role === "tool";

  if (isTool) return null;

  return (
    <div className={cn("flex gap-2 mb-3", isUser ? "justify-end" : "justify-start")}>
      {!isUser && (
        <div className="w-7 h-7 rounded-full bg-primary/10 flex items-center justify-center shrink-0 mt-1">
          <Bot size={14} className="text-primary" />
        </div>
      )}
      <div
        className={cn(
          "max-w-[80%] rounded-lg px-3 py-2 text-sm leading-relaxed",
          isUser
            ? "bg-primary text-primary-foreground"
            : "bg-muted text-foreground",
        )}
      >
        <p className="whitespace-pre-wrap break-words">
          {message.content}
          {message.isStreaming && <span className="inline-block w-1 h-4 bg-foreground/50 ml-0.5 animate-pulse" />}
        </p>
      </div>
      {isUser && (
        <div className="w-7 h-7 rounded-full bg-primary flex items-center justify-center shrink-0 mt-1">
          <span className="text-primary-foreground text-xs font-bold">
            {isUser ? "U" : ""}
          </span>
        </div>
      )}
    </div>
  );
}

function ConfigDrawer({
  open,
  onClose,
  config,
  onSave,
}: {
  open: boolean;
  onClose: () => void;
  config: ProviderConfig;
  onSave: (config: ProviderConfig) => void;
}) {
  const [baseUrl, setBaseUrl] = useState(config.base_url);
  const [model, setModel] = useState(config.model);
  const [apiKey, setApiKey] = useState(config.api_key || "");
  const ollama = useOllama();

  if (!open) return null;

  return (
    <div className="fixed inset-0 z-[60]">
      <div className="fixed inset-0 bg-black/50" onClick={onClose} />
      <div className="fixed inset-y-0 right-0 w-80 bg-card border-l border-border shadow-xl z-[61] p-4">
        <div className="flex items-center justify-between mb-4">
          <h3 className="font-semibold text-sm">Provider Settings</h3>
          <Button variant="ghost" size="icon" onClick={onClose}>
            <X size={16} />
          </Button>
        </div>

        {ollama.checking && (
          <p className="text-xs text-muted-foreground mb-3">Checking Ollama...</p>
        )}
        {ollama.available && (
          <div className="flex items-center gap-2 text-xs text-green-600 mb-3 p-2 bg-green-50 rounded">
            <Sparkles size={12} />
            Ollama disponible localmente
          </div>
        )}
        {!ollama.available && !ollama.checking && (
          <div className="flex items-center gap-2 text-xs text-amber-600 mb-3 p-2 bg-amber-50 rounded">
            <Sparkles size={12} />
            Ollama no detectado — usando cloud
          </div>
        )}

        <div className="space-y-3">
          <div>
            <label className="block text-xs font-medium mb-1">Base URL</label>
            <input
              className="w-full h-8 px-2 text-xs border rounded"
              value={baseUrl}
              onChange={e => setBaseUrl(e.target.value)}
            />
          </div>
          <div>
            <label className="block text-xs font-medium mb-1">Model</label>
            <input
              className="w-full h-8 px-2 text-xs border rounded"
              value={model}
              onChange={e => setModel(e.target.value)}
            />
          </div>
          <div>
            <label className="block text-xs font-medium mb-1">API Key</label>
            <input
              className="w-full h-8 px-2 text-xs border rounded"
              type="password"
              value={apiKey}
              onChange={e => setApiKey(e.target.value)}
              placeholder="sk-..."
            />
          </div>
          <Button
            size="sm"
            className="w-full"
            onClick={() => {
              onSave({
                base_url: baseUrl,
                model,
                api_key: apiKey || undefined,
              });
              onClose();
            }}
          >
            Save
          </Button>
        </div>
      </div>
    </div>
  );
}

export function ChatPanel({ orgId }: ChatPanelProps) {
  const [open, setOpen] = useState(false);
  const [configOpen, setConfigOpen] = useState(false);
  const [input, setInput] = useState("");
  const [providerConfig, setProviderConfig] = useState<ProviderConfig>({
    base_url: "https://api.groq.com/openai/v1",
    model: "gpt-oss-20b",
  });
  const { messages, isLoading, sendMessage, clearMessages } = useAiChat(orgId);
  const scrollRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (scrollRef.current) {
      scrollRef.current.scrollTop = scrollRef.current.scrollHeight;
    }
  }, [messages]);

  const handleSend = () => {
    const text = input.trim();
    if (!text || isLoading) return;
    setInput("");
    sendMessage(text, providerConfig);
  };

  return (
    <>
      {!open && (
        <Button
          className="fixed bottom-5 right-5 z-40 rounded-full shadow-lg w-12 h-12"
          size="icon"
          onClick={() => setOpen(true)}
        >
          <MessageCircle size={20} />
        </Button>
      )}

      {open && (
        <div className="fixed inset-0 z-50">
          <div className="fixed inset-0 bg-black/40" onClick={() => setOpen(false)} />
          <div className="fixed bottom-0 right-0 w-full max-w-sm h-[80vh] sm:h-[600px] bg-card border border-border shadow-2xl rounded-t-xl sm:rounded-xl sm:bottom-5 sm:right-5 z-50 flex flex-col">
            <div className="flex items-center justify-between px-4 py-3 border-b border-border">
              <div className="flex items-center gap-2">
                <Bot size={18} className="text-primary" />
                <span className="font-semibold text-sm">AI Assistant</span>
              </div>
              <div className="flex items-center gap-1">
                <Button variant="ghost" size="icon" onClick={() => setConfigOpen(true)}>
                  <Settings size={15} />
                </Button>
                <Button variant="ghost" size="icon" onClick={clearMessages}>
                  <Sparkles size={15} />
                </Button>
                <Button variant="ghost" size="icon" onClick={() => setOpen(false)}>
                  <X size={16} />
                </Button>
              </div>
            </div>

            <ScrollArea className="flex-1 p-4" ref={scrollRef}>
              {messages.length === 0 && (
                <div className="text-center text-muted-foreground text-xs mt-8">
                  <Bot size={32} className="mx-auto mb-3 opacity-40" />
                  <p>Ask me about your data</p>
                  <p className="text-[10px] mt-1">
                    Try: "Show me recent invoices" or "Search for Juan"
                  </p>
                </div>
              )}
              {messages.map(msg => (
                <MessageBubble key={msg.id} message={msg} />
              ))}
            </ScrollArea>

            <div className="border-t border-border p-3">
              <div className="flex gap-2">
                <input
                  className="flex-1 h-9 px-3 text-sm border rounded-lg bg-background focus:outline-none focus:ring-2 focus:ring-ring"
                  placeholder={isLoading ? "AI is thinking..." : "Ask about your data..."}
                  value={input}
                  onChange={e => setInput(e.target.value)}
                  onKeyDown={e => e.key === "Enter" && handleSend()}
                  disabled={isLoading}
                />
                <Button size="icon" className="h-9 w-9 shrink-0" onClick={handleSend} disabled={isLoading}>
                  <Send size={15} />
                </Button>
              </div>
            </div>
          </div>
        </div>
      )}

      <ConfigDrawer
        open={configOpen}
        onClose={() => setConfigOpen(false)}
        config={providerConfig}
        onSave={setProviderConfig}
      />
    </>
  );
}
