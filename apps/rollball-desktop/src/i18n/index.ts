import i18n from "i18next";
import { initReactI18next } from "react-i18next";

import en from "./locales/en.json";
import zhCN from "./locales/zh-CN.json";
import zhTW from "./locales/zh-TW.json";
import ja from "./locales/ja.json";
import ko from "./locales/ko.json";

const STORAGE_KEY = "i18nextLng";

const SUPPORTED_LANGS = ["en", "zh-CN", "zh-TW", "ja", "ko"];

const resources = {
    en: { translation: en },
    "zh-CN": { translation: zhCN },
    "zh-TW": { translation: zhTW },
    ja: { translation: ja },
    ko: { translation: ko },
};

// Detect initial language: localStorage > navigator > fallback
function detectLanguage(): string {
    try {
        const stored = localStorage.getItem(STORAGE_KEY);
        if (stored && SUPPORTED_LANGS.includes(stored)) return stored;
    } catch { /* ignore */ }
    const navLang = navigator.language;
    // Match exact or prefix (e.g. "zh-CN", "zh-TW", "ja", "ko")
    if (SUPPORTED_LANGS.includes(navLang)) return navLang;
    if (navLang.startsWith("zh-T")) return "zh-TW";
    if (navLang.startsWith("zh")) return "zh-CN";
    if (navLang.startsWith("ja")) return "ja";
    if (navLang.startsWith("ko")) return "ko";
    return "en";
}

const initialLang = detectLanguage();
console.log("[i18n] Detected initial language:", initialLang);

i18n
    .use(initReactI18next)
    .init({
        resources,
        lng: initialLang,
        fallbackLng: "en",
        supportedLngs: SUPPORTED_LANGS,
        interpolation: {
            escapeValue: false,
        },
    })
    .then(() => {
        console.log("[i18n] Initialized, current language:", i18n.language);
    });

i18n.on("languageChanged", (lng) => {
    console.log("[i18n] Language changed to:", lng);
    try {
        localStorage.setItem(STORAGE_KEY, lng);
    } catch { /* ignore */ }
});

export default i18n;
