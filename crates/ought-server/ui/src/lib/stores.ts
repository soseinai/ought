import { writable, derived } from "svelte/store";
import type { ApiResponse, Spec, Section, Clause } from "./types";

export const data = writable<ApiResponse | null>(null);
export const activeSpecIndex = writable<number>(0);
export const searchQuery = writable<string>("");
export const activeFilter = writable<string | null>(null);
export const searchResults = writable<SearchResult[] | null>(null);
export const isSearching = writable<boolean>(false);

export interface SearchResult {
  clause_id: string;
  keyword: string;
  text: string;
  spec_name: string;
  section_path: string;
  condition: string | null;
  temporal: { kind: string; duration?: string } | null;
  score: number;
  highlight: string;
}

export const activeSpec = derived(
  [data, activeSpecIndex],
  ([$data, $idx]) => $data?.specs[$idx] ?? null
);

export async function loadData() {
  const res = await fetch("/api/specs");
  const json: ApiResponse = await res.json();
  data.set(json);
}

let searchTimer: ReturnType<typeof setTimeout> | null = null;

/** Debounced search against the server API */
export function triggerSearch(query: string) {
  if (searchTimer) clearTimeout(searchTimer);

  if (!query.trim()) {
    searchResults.set(null);
    isSearching.set(false);
    return;
  }

  isSearching.set(true);
  searchTimer = setTimeout(async () => {
    try {
      const res = await fetch(`/api/search?q=${encodeURIComponent(query)}&limit=30`);
      const json = await res.json();
      searchResults.set(json.results);
    } catch {
      searchResults.set([]);
    }
    isSearching.set(false);
  }, 150);
}

/** Count total clauses (including otherwise) in a section tree */
export function countClauses(sections: Section[]): number {
  let n = 0;
  for (const s of sections) {
    n += s.clauses.length;
    for (const c of s.clauses) {
      n += c.otherwise.length;
    }
    n += countClauses(s.subsections);
  }
  return n;
}

/** Filter clauses by keyword filter only (search is server-side now) */
export function filterClauses(
  clauses: Clause[],
  filter: string | null
): Clause[] {
  if (!filter) return clauses;
  return clauses.filter((c) => c.keyword === filter);
}
