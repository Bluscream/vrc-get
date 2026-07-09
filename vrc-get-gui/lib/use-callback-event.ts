import { createContext, use, useCallback, useEffect, useRef } from "react";

const dummy = createContext(null);

function isInRender() {
	try {
		use(dummy);
		return true;
	} catch {
		return false;
	}
}

/**
 * Something like useEffectEvent
 * @see https://ja.react.dev/learn/separating-events-from-effects#declaring-an-effect-event
 */
export function useCallbackEvent<Args extends unknown[]>(
	listener: (...args: Args) => void,
): (...args: Args) => void {
	const event = useRef<(...args: Args) => void>(listener);

	useEffect(() => {
		event.current = listener;
	}, [listener]);

	return useCallback((...args: Args) => {
		if (isInRender()) {
			throw new Error("Cannot call an event handler while rendering.");
		}
		event.current(...args);
	}, []);
}
