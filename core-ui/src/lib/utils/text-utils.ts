/**
 * Text helpers for Vietnamese titles.
 *
 * RECOVERY NOTE: only the diacritic table and the Map built from it survive in
 * the compiled bundles (the module's functions were tree-shaken because the
 * frontend never called them). The functions below are inferred.
 */

const VIETNAMESE_DIACRITICS: ReadonlyArray<readonly [string, string]> = [
  ["àáảãạăằắẳẵặâầấẩẫậ", "a"],
  ["èéẻẽẹêềếểễệ", "e"],
  ["ìíỉĩị", "i"],
  ["òóỏõọôồốổỗộơờớởỡợ", "o"],
  ["ùúủũụưừứửữự", "u"],
  ["ỳýỷỹỵ", "y"],
  ["đ", "d"],
];

/** Accented lowercase character -> plain ASCII letter. */
const DIACRITIC_MAP = new Map<string, string>();
for (const [chars, base] of VIETNAMESE_DIACRITICS) {
  for (const ch of chars) DIACRITIC_MAP.set(ch, base);
}

/** Lowercases and strips Vietnamese diacritics: "Đường Về" -> "duong ve". (inferred) */
export function stripVietnameseDiacritics(text: string): string {
  let out = "";
  for (const ch of text.toLowerCase()) out += DIACRITIC_MAP.get(ch) ?? ch;
  return out;
}

/** Normalizes a title for fuzzy comparison / search. (inferred) */
export function normalizeTitle(text: string): string {
  return stripVietnameseDiacritics(text)
    .replace(/[^a-z0-9]+/g, " ")
    .trim();
}
