export function matchSearch(text: string, query: string): boolean {
  if (!query) return true;
  try {
    return new RegExp(query, "i").test(text);
  } catch {
    return text.toLowerCase().includes(query.toLowerCase());
  }
}
