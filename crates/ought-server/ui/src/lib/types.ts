export interface Spec {
  name: string;
  source_path: string;
  metadata: {
    context: string | null;
    sources: string[];
    schemas: string[];
    requires: { label: string; path: string; anchor: string | null }[];
  };
  sections: Section[];
}

export interface Section {
  title: string;
  depth: number;
  prose: string;
  clauses: Clause[];
  subsections: Section[];
}

export interface Proof {
  name: string;
  summary: string;
  code: string;
  language: string;
}

export interface ClauseProofs {
  file: string | null;
  tests: Proof[];
}

export interface Clause {
  id: string;
  keyword: string;
  severity: string;
  text: string;
  condition: string | null;
  otherwise: Clause[];
  temporal: { kind: string; duration?: string } | null;
  hints: string[];
  proofs: ClauseProofs;
}

export interface ApiResponse {
  specs: Spec[];
  stats: {
    total_specs: number;
    total_sections: number;
    total_clauses: number;
    by_keyword: Record<string, number>;
  };
}

export const KW_LABELS: Record<string, string> = {
  Must: "MUST",
  MustNot: "MUST NOT",
  Should: "SHOULD",
  ShouldNot: "SHOULD NOT",
  May: "MAY",
  Wont: "WONT",
  Given: "GIVEN",
  Otherwise: "OTHERWISE",
  MustAlways: "MUST ALWAYS",
  MustBy: "MUST BY",
};

export const KW_ORDER = [
  "Must",
  "MustNot",
  "MustAlways",
  "MustBy",
  "Should",
  "ShouldNot",
  "May",
  "Wont",
  "Given",
  "Otherwise",
];
