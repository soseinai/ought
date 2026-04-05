<script lang="ts">
  import hljs from "highlight.js/lib/core";
  import rust from "highlight.js/lib/languages/rust";
  import python from "highlight.js/lib/languages/python";
  import typescript from "highlight.js/lib/languages/typescript";
  import go from "highlight.js/lib/languages/go";
  import "highlight.js/styles/github-dark.css";

  // Register once per module load.
  if (!(hljs as any).__ought_registered) {
    hljs.registerLanguage("rust", rust);
    hljs.registerLanguage("python", python);
    hljs.registerLanguage("typescript", typescript);
    hljs.registerLanguage("javascript", typescript);
    hljs.registerLanguage("go", go);
    (hljs as any).__ought_registered = true;
  }

  interface Props {
    code: string;
    language?: string;
  }

  let { code, language = "rust" }: Props = $props();

  let highlighted = $derived.by(() => {
    try {
      const lang = hljs.getLanguage(language) ? language : "plaintext";
      if (lang === "plaintext") return escapeHtml(code);
      return hljs.highlight(code, { language: lang }).value;
    } catch {
      return escapeHtml(code);
    }
  });

  function escapeHtml(s: string): string {
    return s
      .replace(/&/g, "&amp;")
      .replace(/</g, "&lt;")
      .replace(/>/g, "&gt;");
  }
</script>

<pre
  class="text-xs rounded-md border border-[var(--border)] bg-[#0d1117] p-3 overflow-x-auto leading-relaxed"
><code class="hljs language-{language}">{@html highlighted}</code></pre>
