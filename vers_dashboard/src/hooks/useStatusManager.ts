import { useState, useEffect, useCallback, useRef } from 'react';
import { useEventStream } from './useEventStream';
import type { StrictSystemEvent } from '../types';

interface ThoughtLine {
  id: number;
  text: string;
  timestamp: number;
}

export const useStatusManager = (fetchMetrics: () => void) => {
  const [eventHistory, setEventHistory] = useState<StrictSystemEvent[]>([]);
  const [thoughtLines, setThoughtLines] = useState<ThoughtLine[]>([]);
  const [isHistoryLoaded, setIsHistoryLoaded] = useState(false);
  const nextId = useRef(0);
  const pendingEvents = useRef<StrictSystemEvent[]>([]);

  useEffect(() => {
    const interval = setInterval(() => {
      const now = Date.now();
      setThoughtLines(prev => prev.filter(line => now - line.timestamp < 30000));
    }, 100);
    return () => clearInterval(interval);
  }, []);

  const handleEvent = useCallback((data: StrictSystemEvent) => {
    if (!isHistoryLoaded) {
      pendingEvents.current.push(data);
      return;
    }

    const eventTimestamp = data.timestamp || Date.now();

    if (data.type === "Thought") {
      setThoughtLines(prev => {
        const next = [...prev, { id: nextId.current++, text: data.payload.content, timestamp: eventTimestamp }];
        return next.slice(-12);
      });
    } else if (data.type === "ResponseGenerated") {
      const text = data.payload.content.replace(/\n/g, ' ');
      const segments: string[] = text.match(/.{1,100}/g) || [];
      setThoughtLines(prev => {
        const next = [...prev, ...segments.slice(0, 3).map((s: string) => ({ id: nextId.current++, text: s, timestamp: eventTimestamp }))];
        return next.slice(-12);
      });
    } else {
      setEventHistory(prev => [...prev, { ...data, timestamp: eventTimestamp }]);
      if (data.type === "ToolEnd" || data.type === "MessageReceived") fetchMetrics();
    }
  }, [fetchMetrics, isHistoryLoaded]);

  useEventStream('/api/events', handleEvent);

  useEffect(() => {
    const fetchHistory = async () => {
      try {
        const res = await fetch('/api/history');
        if (res.ok) {
          const history: StrictSystemEvent[] = await res.json();
          const now = Date.now();
          
          setEventHistory(history.map(e => ({ ...e, timestamp: e.timestamp || now })));
          
          const historicalThoughts: ThoughtLine[] = [];
          history.forEach(e => {
            const eventTimestamp = e.timestamp || now;
            // Skip thoughts older than 30 seconds
            if (now - eventTimestamp > 30000) return;

            if (e.type === "Thought") {
              historicalThoughts.push({ id: nextId.current++, text: e.payload.content, timestamp: eventTimestamp });
            } else if (e.type === "ResponseGenerated") {
              const text = e.payload.content.replace(/\n/g, ' ');
              const segments: string[] = text.match(/.{1,100}/g) || [];
              segments.slice(0, 3).forEach(s => historicalThoughts.push({ id: nextId.current++, text: s, timestamp: eventTimestamp }));
            }
          });
          setThoughtLines(historicalThoughts.slice(-12));
          
          // Process pending events
          setIsHistoryLoaded(true);
          pendingEvents.current.forEach(handleEvent);
          pendingEvents.current = [];
        }
      } catch (e) {
        console.error("Failed to fetch history", e);
      }
    };
    fetchHistory();
  }, [fetchMetrics, handleEvent]);

  return { eventHistory, thoughtLines, isHistoryLoaded };
};
