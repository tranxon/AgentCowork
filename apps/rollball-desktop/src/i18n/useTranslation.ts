import { useState, useEffect, useCallback } from "react";
import i18n from "./index";

/**
 * Custom useTranslation hook that directly subscribes to i18next's
 * languageChanged event, bypassing react-i18next's internal subscription
 * which is broken in react-i18next v17 + React 19 combination.
 */
export function useTranslation() {
    const [, setTick] = useState(0);

    useEffect(() => {
        const handler = () => setTick((n) => n + 1);
        i18n.on("languageChanged", handler);
        return () => {
            i18n.off("languageChanged", handler);
        };
    }, []);

    const t = useCallback(
        (key: string, options?: Record<string, unknown>) => i18n.t(key, options),
        // Recreate t when language changes (tick drives the dependency)
        // eslint-disable-next-line react-hooks/exhaustive-deps
        [i18n.language],
    );

    return { t, i18n };
}
