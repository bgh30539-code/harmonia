import en from "./locales/en.json";
import es from "./locales/es.json";

const locales: Record<string, Record<string, string>> = { en, es };

export type Locale = keyof typeof locales;

function lookup(lang: string, key: string): string | undefined {
  const table = locales[lang] ?? locales.en;
  return table[key];
}

export function translate(lang: string, key: string, vars?: Record<string, string | number>): string {
  let text = lookup(lang, key) ?? lookup("en", key) ?? key;
  if (vars) {
    for (const [k, v] of Object.entries(vars)) {
      text = text.replaceAll(`{${k}}`, String(v));
    }
  }
  return text;
}
