import { useEffect, useRef } from 'react';

export function useEventStream(
  url: string,
  onMessage: (data: any) => void
) {
  const reconnectTimeout = useRef<number | null>(null);
  const eventSourceRef = useRef<EventSource | null>(null);
  const onMessageRef = useRef(onMessage);

  useEffect(() => {
    onMessageRef.current = onMessage;
  }, [onMessage]);

  useEffect(() => {
    const connect = () => {
      console.log(`📡 Connecting to Event Stream: ${url}`);
      const es = new EventSource(url);
      eventSourceRef.current = es;

      es.onmessage = (event) => {
        try {
          const data = JSON.parse(event.data);
          if (onMessageRef.current) {
            onMessageRef.current(data);
          }
        } catch (err) {
          console.error("Failed to parse SSE event:", err);
        }
      };

      es.onerror = (err) => {
        console.error("SSE Connection Error. Retrying in 5s...", err);
        es.close();
        if (reconnectTimeout.current) clearTimeout(reconnectTimeout.current);
        reconnectTimeout.current = window.setTimeout(connect, 5000);
      };
    };

    connect();

    return () => {
      if (eventSourceRef.current) eventSourceRef.current.close();
      if (reconnectTimeout.current) clearTimeout(reconnectTimeout.current);
    };
  }, [url]);
}
