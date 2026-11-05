# Project guidance for Codex

## Graphify first

Before scanning or reading source files to answer a question about this project:

1. If `graphify-out/graph.json` exists, consult Graphify first. Read `graphify-out/GRAPH_REPORT.md` for a broad project overview; for a focused question, run `graphify query "<question>"`. Use `graphify path "<A>" "<B>"` or `graphify explain "<symbol>"` when those fit the question.
2. Use the graph to locate relevant files and relationships, then inspect the current source at those paths and line numbers before stating a finding or changing code. Label inferred relationships and say when the graph cannot verify one.
3. Graphify does not cover every file type and may be stale. If an output is missing, stale for the files involved, or incomplete for the task, inspect the necessary source directly and say what was not covered. Do not treat an absent graph edge as proof that no dependency exists.
4. Include this Graphify-first rule in prompts to any subagent asked to explore this repository.

After changing code, run `graphify update .` when available so the code graph stays current. Update or rebuild the graph separately for changed documentation or other file types if they matter to the task.
