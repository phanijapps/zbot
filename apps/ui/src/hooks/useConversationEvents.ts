/**
 * React hook for subscribing to conversation events.
 *
 * Uses server-side subscription routing - events are only received
 * for explicitly subscribed conversations.
 */

import { useEffect, useRef } from "react";
import { getTransport } from "@/services/transport";
import type {
  ConversationEvent,
  SubscriptionErrorMessage,
  UnsubscribeFn,
} from "@/services/transport/types";

interface UseConversationEventsOptions {
  /** Called when a subscription error occurs */
  onError?: (error: SubscriptionErrorMessage) => void;
  /** Called when subscription is confirmed with current sequence */
  onConfirmed?: (seq: number) => void;
}

/**
 * Subscribe to events for a specific conversation.
 *
 * @param conversationId - The conversation to subscribe to (null to skip)
 * @param onEvent - Callback for received events
 * @param options - Optional error and confirmation handlers
 */
export function useConversationEvents(
  conversationId: string | null,
  onEvent: (event: ConversationEvent) => void,
  options: UseConversationEventsOptions = {}
) {
  // Use refs to avoid re-subscribing when callbacks change
  const onEventRef = useRef(onEvent);
  const onErrorRef = useRef(options.onError);
  const onConfirmedRef = useRef(options.onConfirmed);

  useEffect(() => {
    onEventRef.current = onEvent;
  }, [onEvent]);

  useEffect(() => {
    onErrorRef.current = options.onError;
  }, [options.onError]);

  useEffect(() => {
    onConfirmedRef.current = options.onConfirmed;
  }, [options.onConfirmed]);

  useEffect(() => {
    if (!conversationId) return;

    let unsubscribe: UnsubscribeFn | null = null;
    let cancelled = false;

    const subscribe = async () => {
      try {
        const transport = await getTransport();

        if (cancelled) return;

        unsubscribe = transport.subscribeConversation(conversationId, {
          onEvent: (event) => onEventRef.current(event),
          onError: (error) => onErrorRef.current?.(error),
          onConfirmed: (seq) => onConfirmedRef.current?.(seq),
        });
      } catch (error) {
        if (!cancelled && onErrorRef.current) {
          onErrorRef.current({
            type: "subscription_error",
            conversation_id: conversationId,
            code: "SERVER_ERROR",
            message: String(error),
          });
        }
      }
    };

    subscribe();

    return () => {
      cancelled = true;
      if (unsubscribe) unsubscribe();
    };
  }, [conversationId]);
}
