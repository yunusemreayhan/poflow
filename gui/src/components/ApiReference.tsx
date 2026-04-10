import { useState, useEffect } from "react";
import { Copy, Check, ExternalLink } from "lucide-react";
import { apiCall } from "../store/api";

interface PathItem {
  summary?: string;
  description?: string;
  operationId?: string;
  parameters?: Array<{ name: string; in: string; required?: boolean; schema?: { type?: string } }>;
  requestBody?: { content?: { "application/json"?: { schema?: { "$ref"?: string } } } };
  security?: unknown[];
}

interface OpenApiSpec {
  info: { title: string; version: string; description?: string };
  paths: Record<string, Record<string, PathItem>>;
  components?: { schemas?: Record<string, unknown> };
}

function CopyButton({ text }: { text: string }) {
  const [copied, setCopied] = useState(false);
  const copy = () => { navigator.clipboard.writeText(text); setCopied(true); setTimeout(() => setCopied(false), 1500); };
  return (
    <button onClick={copy} className="shrink-0 w-7 h-7 flex items-center justify-center rounded-lg text-white/30 hover:text-white hover:bg-white/10 transition-all" title="Copy">
      {copied ? <Check size={14} className="text-[var(--color-success)]" /> : <Copy size={14} />}
    </button>
  );
}

const METHOD_COLORS: Record<string, string> = {
  get: "text-green-400 bg-green-400/10",
  post: "text-blue-400 bg-blue-400/10",
  put: "text-yellow-400 bg-yellow-400/10",
  delete: "text-red-400 bg-red-400/10",
};

function buildCurlExample(method: string, path: string, op: PathItem, hasAuth: boolean): string {
  let curl = `curl -s`;
  if (method !== "get") curl += ` -X ${method.toUpperCase()}`;
  if (hasAuth) curl += ` -H "Authorization: Bearer <token>"`;
  if (op.requestBody) curl += ` -H "Content-Type: application/json" -d '{}'`;
  curl += ` http://localhost:9090${path}`;
  return curl;
}

function specToMarkdown(spec: OpenApiSpec): string {
  let md = `# ${spec.info.title} v${spec.info.version}\n\n`;
  if (spec.info.description) md += `${spec.info.description}\n\n`;
  md += `Base URL: http://localhost:9090\nAuth: Bearer JWT token in Authorization header\n\n`;
  md += `## Endpoints\n\n`;
  for (const [path, methods] of Object.entries(spec.paths)) {
    for (const [method, op] of Object.entries(methods)) {
      const hasAuth = !!(op.security && op.security.length > 0);
      md += `### ${method.toUpperCase()} ${path}\n`;
      if (op.summary) md += `${op.summary}\n`;
      if (op.description) md += `${op.description}\n`;
      md += `Auth: ${hasAuth ? "Required" : "None"}\n`;
      if (op.parameters?.length) {
        md += `Parameters: ${op.parameters.map(p => `${p.name} (${p.in}${p.required ? ", required" : ""})`).join(", ")}\n`;
      }
      md += `\`\`\`\n${buildCurlExample(method, path, op, hasAuth)}\n\`\`\`\n\n`;
    }
  }
  if (spec.components?.schemas) {
    md += `## Schemas\n\n\`\`\`json\n${JSON.stringify(spec.components.schemas, null, 2)}\n\`\`\`\n`;
  }
  return md;
}

export default function ApiReference() {
  const [spec, setSpec] = useState<OpenApiSpec | null>(null);
  const [error, setError] = useState("");
  const [copyAll, setCopyAll] = useState(false);

  useEffect(() => {
    apiCall<OpenApiSpec>("GET", "/api-docs/openapi.json")
      .then(setSpec)
      .catch((e) => setError(String(e)));
  }, []);

  if (error) return <div className="p-8 text-[var(--color-danger)] text-sm">Failed to load API spec: {error}</div>;
  if (!spec) return <div className="p-8 text-white/40 text-sm">Loading API spec from server...</div>;

  const fullDoc = specToMarkdown(spec);

  const handleCopyAll = () => {
    navigator.clipboard.writeText(fullDoc);
    setCopyAll(true);
    setTimeout(() => setCopyAll(false), 2000);
  };

  return (
    <div className="flex flex-col gap-4 p-8 h-full overflow-y-auto">
      <div className="flex items-center justify-between">
        <h2 className="text-lg font-semibold text-white">{spec.info.title} <span className="text-xs text-white/30">v{spec.info.version}</span></h2>
        <div className="flex gap-2">
          <a href="http://localhost:9090/swagger-ui/" target="_blank" rel="noreferrer"
            className="flex items-center gap-2 px-4 py-2 rounded-lg text-sm glass text-white/60 hover:text-white">
            <ExternalLink size={14} /> Swagger UI
          </a>
          <button onClick={handleCopyAll}
            className={`flex items-center gap-2 px-4 py-2 rounded-lg text-sm transition-all ${copyAll ? "bg-[var(--color-success)] text-white" : "glass text-white/60 hover:text-white"}`}>
            {copyAll ? <Check size={14} /> : <Copy size={14} />}
            {copyAll ? "Copied!" : "Copy Full API Doc"}
          </button>
        </div>
      </div>

      {spec.info.description && (
        <div className="glass p-4 text-xs text-white/50">{spec.info.description}</div>
      )}

      {Object.entries(spec.paths).map(([path, methods]) =>
        Object.entries(methods).map(([method, op]) => {
          const hasAuth = !!(op.security && op.security.length > 0);
          const curl = buildCurlExample(method, path, op, hasAuth);
          const refName = op.requestBody?.content?.["application/json"]?.schema?.["$ref"]?.split("/").pop();
          return (
            <div key={`${method}-${path}`} className="glass p-4">
              <div className="flex items-center gap-2 mb-1">
                <span className={`text-xs font-mono font-bold px-2 py-0.5 rounded uppercase ${METHOD_COLORS[method] ?? "text-white/60 bg-white/5"}`}>{method}</span>
                <span className="text-sm font-mono text-white/80">{path}</span>
                {hasAuth && <span className="text-[10px] text-white/30 bg-white/5 px-1.5 py-0.5 rounded">🔒 Auth</span>}
              </div>
              {(op.summary || op.description) && (
                <p className="text-xs text-white/50 mb-2">{op.summary || op.description}</p>
              )}
              {op.parameters && op.parameters.length > 0 && (
                <p className="text-xs text-white/40 mb-2">
                  Params: {op.parameters.map(p => <span key={p.name} className="text-white/60">{p.name}{p.required ? "" : "?"} </span>)}
                </p>
              )}
              {refName && (
                <p className="text-xs text-white/40 mb-2">Body: <span className="text-[var(--color-accent)]">{refName}</span></p>
              )}
              <div className="flex items-center gap-2 bg-black/30 rounded-lg px-3 py-2">
                <code className="flex-1 text-xs text-white/70 font-mono break-all select-all">{curl}</code>
                <CopyButton text={curl} />
              </div>
            </div>
          );
        })
      )}

      {spec.components?.schemas && (
        <div className="glass p-4">
          <h3 className="text-sm font-semibold text-white/60 mb-3">Schemas</h3>
          <pre className="text-xs text-white/50 font-mono overflow-x-auto max-h-96 overflow-y-auto">
            {JSON.stringify(spec.components.schemas, null, 2)}
          </pre>
        </div>
      )}
    </div>
  );
}
