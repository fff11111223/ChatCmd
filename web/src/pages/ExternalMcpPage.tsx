import { CheckCircle2, Link2, LoaderCircle, Pencil, Plug, Plug2, Plus, RefreshCw, Search, Server, Trash2, WifiOff, XCircle } from 'lucide-react';
import { FormEvent, useEffect, useState } from 'react';
import { api } from '../api';
import { Empty, ErrorState, Loading, Modal, PageHeading, ProblemBanner } from '../components';
import { tr } from '../i18n';
import { useLoad } from '../useLoad';
import type {
  ExternalMcpServer,
  ExternalMcpServerConfig,
  ExternalMcpToolSummary,
} from '../types';

// ── Status badge ─────────────────────────────────────────────────────────────

function McpStatusBadge({ status }: { status: ExternalMcpServer['status'] }) {
  const s = status?.status;
  if (s === 'connected') return <span className="badge success"><CheckCircle2 size={12} /> {tr('Connected')}</span>;
  if (s === 'connecting') return <span className="badge warning"><LoaderCircle size={12} className="spin" /> {tr('Connecting')}</span>;
  if (s === 'error') return <span className="badge danger" title={status.message}><XCircle size={12} /> {tr('Error')}</span>;
  return <span className="badge muted"><WifiOff size={12} /> {tr('Disconnected')}</span>;
}

// ── Blank config ──────────────────────────────────────────────────────────────

const blankConfig = (): ExternalMcpServerConfig => ({
  id: '',
  name: '',
  description: '',
  enabled: true,
  transport: {
    type: 'stdio',
    command: '',
    args: [],
    env: {},
    cwd: null,
  },
  timeoutSecs: 30,
  autoConnect: true,
});

// ── Main page ─────────────────────────────────────────────────────────────────

export function ExternalMcpPage() {
  const servers = useLoad(api.externalMcpServers, []);
  const [editor, setEditor] = useState<ExternalMcpServer | 'new'>();
  const [problem, setProblem] = useState('');
  const [connectingId, setConnectingId] = useState<string>();
  const [testingId, setTestingId] = useState<string>();
  const [deletingId, setDeletingId] = useState<string>();
  const [searchQ, setSearchQ] = useState('');
  const [toolResults, setToolResults] = useState<{ tools: ExternalMcpToolSummary[]; total: number } | null>(null);
  const [loadingTools, setLoadingTools] = useState(false);
  const [schemaView, setSchemaView] = useState<ExternalMcpToolSummary & { schema?: unknown }>();

  const updateList = (next: ExternalMcpServer) =>
    servers.setData((cur) => cur ? [next, ...cur.filter((s) => s.id !== next.id)] : [next]);

  const handleSave = async (config: ExternalMcpServerConfig) => {
    setProblem('');
    try {
      if (editor === 'new') {
        const res = await api.createExternalMcpServer(config);
        updateList({
          ...config,
          ...res.server,
          transportType: config.transport.type,
          status: { status: 'disconnected' },
          toolsCount: 0,
          updatedAtMs: Date.now(),
        });
      } else if (editor) {
        const res = await api.updateExternalMcpServer(editor.id, config);
        updateList({
          ...editor,
          ...res.server,
          transportType: config.transport.type,
        });
      }
      setEditor(undefined);
    } catch (e: unknown) {
      setProblem(e instanceof Error ? e.message : String(e));
    }
  };

  const handleDelete = async (id: string) => {
    setDeletingId(id);
    try {
      await api.deleteExternalMcpServer(id);
      servers.setData((cur) => cur?.filter((s) => s.id !== id) ?? []);
    } catch (e: unknown) {
      setProblem(e instanceof Error ? e.message : String(e));
    } finally {
      setDeletingId(undefined);
    }
  };

  const handleConnect = async (id: string) => {
    setConnectingId(id);
    setProblem('');
    try {
      const res = await api.connectExternalMcpServer(id);
      servers.setData((cur) =>
        cur?.map((s) => s.id === id ? { ...s, status: res.status ?? s.status, toolsCount: res.toolCount ?? s.toolsCount } : s) ?? []
      );
    } catch (e: unknown) {
      setProblem(e instanceof Error ? e.message : String(e));
    } finally {
      setConnectingId(undefined);
    }
  };

  const handleDisconnect = async (id: string) => {
    setConnectingId(id);
    setProblem('');
    try {
      await api.disconnectExternalMcpServer(id);
      servers.setData((cur) =>
        cur?.map((s) => s.id === id ? { ...s, status: { status: 'disconnected' }, toolsCount: 0 } : s) ?? []
      );
    } catch (e: unknown) {
      setProblem(e instanceof Error ? e.message : String(e));
    } finally {
      setConnectingId(undefined);
    }
  };

  const handleTest = async (id: string) => {
    setTestingId(id);
    setProblem('');
    try {
      const res = await api.testExternalMcpServer(id);
      if (res.ok) {
        servers.setData((cur) =>
          cur?.map((s) => s.id === id ? { ...s, status: res.status ?? s.status, toolsCount: res.toolCount ?? s.toolsCount } : s) ?? []
        );
      } else {
        setProblem(`Test failed: ${res.error ?? 'unknown error'}`);
      }
    } catch (e: unknown) {
      setProblem(e instanceof Error ? e.message : String(e));
    } finally {
      setTestingId(undefined);
    }
  };

  const handleSearch = async () => {
    setLoadingTools(true);
    try {
      const res = await api.searchExternalMcpTools(searchQ);
      setToolResults(res);
    } catch (e: unknown) {
      setProblem(e instanceof Error ? e.message : String(e));
    } finally {
      setLoadingTools(false);
    }
  };

  const handleViewSchema = async (tool: ExternalMcpToolSummary) => {
    try {
      const res = await api.externalMcpToolSchema(tool.qualifiedName);
      setSchemaView({ ...tool, schema: res.tool?.inputSchema });
    } catch (e: unknown) {
      setProblem(e instanceof Error ? e.message : String(e));
    }
  };

  return (
    <div className="page">
      <PageHeading
        eyebrow={tr('Tools & Integrations')}
        title={tr('External MCP Servers')}
        body={tr('Manage external MCP server connections. Tools are exposed to ChatGPT as mcp__{id}__{tool}.')}
        actions={
          <button className="button primary" onClick={() => setEditor('new')}>
            <Plus size={16} /> {tr('Add Server')}
          </button>
        }
      />

      {problem && <ProblemBanner message={problem} clear={() => setProblem('')} />}

      {/* Server list */}
      {servers.loading ? <Loading /> : servers.error ? <ErrorState message={servers.error} retry={servers.reload} /> : (
        <>
          {(!servers.data || servers.data.length === 0) && (
            <Empty title={tr('No external MCP servers')} body={tr('Add a server to expose its tools to ChatGPT.')} />
          )}
          <div className="card-list">
            {(servers.data ?? []).map((server) => (
              <div key={server.id} className="card">
                <div className="card-header">
                  <div className="card-title">
                    <strong>{server.name}</strong>
                    <code className="muted small">{server.id}</code>
                  </div>
                  <McpStatusBadge status={server.status} />
                </div>
                {server.description && <p className="card-description">{server.description}</p>}
                <div className="card-meta muted small">
                  <span><Server size={12} /> {server.transportType}</span>
                  {server.status?.status === 'connected' && (
                    <span><Link2 size={12} /> {server.toolsCount} {tr('tools')}</span>
                  )}
                </div>
                <div className="card-actions">
                  {server.status?.status === 'connected' ? (
                    <button className="button small" onClick={() => handleDisconnect(server.id)} disabled={connectingId === server.id}>
                      {connectingId === server.id ? <LoaderCircle size={14} className="spin" /> : <WifiOff size={14} />}
                      {tr('Disconnect')}
                    </button>
                  ) : (
                    <button className="button small primary" onClick={() => handleConnect(server.id)} disabled={connectingId === server.id}>
                      {connectingId === server.id ? <LoaderCircle size={14} className="spin" /> : <Plug size={14} />}
                      {tr('Connect')}
                    </button>
                  )}
                  <button className="button small" onClick={() => handleTest(server.id)} disabled={testingId === server.id}>
                    {testingId === server.id ? <LoaderCircle size={14} className="spin" /> : <RefreshCw size={14} />}
                    {tr('Test')}
                  </button>
                  <button className="button small" onClick={() => setEditor(server)}>
                    <Pencil size={14} /> {tr('Edit')}
                  </button>
                  <button className="button small danger" onClick={() => handleDelete(server.id)} disabled={deletingId === server.id}>
                    {deletingId === server.id ? <LoaderCircle size={14} className="spin" /> : <Trash2 size={14} />}
                    {tr('Delete')}
                  </button>
                </div>
              </div>
            ))}
          </div>
        </>
      )}

      {/* Tool search */}
      <section className="section">
        <h2 className="section-title">{tr('Tool Search')}</h2>
        <div className="search-row">
          <input
            className="input"
            placeholder={tr('Search tools across all servers…')}
            value={searchQ}
            onChange={(e) => setSearchQ(e.target.value)}
            onKeyDown={(e) => e.key === 'Enter' && handleSearch()}
          />
          <button className="button" onClick={handleSearch} disabled={loadingTools}>
            {loadingTools ? <LoaderCircle size={16} className="spin" /> : <Search size={16} />}
            {tr('Search')}
          </button>
        </div>
        {toolResults && (
          <div className="tool-results">
            <p className="muted small">{toolResults.total} {tr('tools found')}</p>
            {toolResults.tools.map((tool) => (
              <div key={tool.qualifiedName} className="tool-row" onClick={() => handleViewSchema(tool)}>
                <code className="tool-name">{tool.qualifiedName}</code>
                <span className="muted small">{tool.description}</span>
              </div>
            ))}
          </div>
        )}
      </section>

      {/* Schema modal */}
      {schemaView && (
        <Modal title={schemaView.qualifiedName} close={() => setSchemaView(undefined)}>
          <p className="muted">{schemaView.description}</p>
          <pre className="code-block">{JSON.stringify(schemaView.schema, null, 2)}</pre>
        </Modal>
      )}

      {/* Editor modal */}
      {editor && (
        <ServerEditorModal
          initial={editor === 'new' ? null : editor}
          onSave={handleSave}
          onClose={() => { setEditor(undefined); setProblem(''); }}
          problem={problem}
        />
      )}
    </div>
  );
}

// ── Server editor modal ────────────────────────────────────────────────────────

interface ServerEditorModalProps {
  initial: ExternalMcpServer | null;
  onSave: (config: ExternalMcpServerConfig) => Promise<void>;
  onClose: () => void;
  problem: string;
}

function ServerEditorModal({ initial, onSave, onClose, problem }: ServerEditorModalProps) {
  const [saving, setSaving] = useState(false);
  const [form, setForm] = useState<ExternalMcpServerConfig>(() => {
    if (!initial) return blankConfig();
    return {
      id: initial.id,
      name: initial.name,
      description: initial.description ?? '',
      enabled: initial.enabled,
      transport: (initial as unknown as { transport: ExternalMcpServerConfig['transport'] }).transport ?? blankConfig().transport,
      timeoutSecs: (initial as unknown as { timeoutSecs?: number }).timeoutSecs ?? 30,
      autoConnect: (initial as unknown as { autoConnect?: boolean }).autoConnect ?? true,
    };
  });
  const [argsText, setArgsText] = useState(() =>
    Array.isArray((form.transport as { args?: string[] }).args) ? (form.transport as { args?: string[] }).args!.join('\n') : ''
  );

  const update = <K extends keyof ExternalMcpServerConfig>(key: K, value: ExternalMcpServerConfig[K]) =>
    setForm((f) => ({ ...f, [key]: value }));

  const updateTransport = <K extends string>(key: K, value: unknown) =>
    setForm((f) => ({ ...f, transport: { ...f.transport, [key]: value } }));

  const handleSubmit = async (e: FormEvent) => {
    e.preventDefault();
    setSaving(true);
    const args = argsText.split('\n').map((s) => s.trim()).filter(Boolean);
    const finalConfig = { ...form, transport: { ...form.transport, args } };
    await onSave(finalConfig);
    setSaving(false);
  };

  const isStdio = form.transport.type === 'stdio';

  return (
    <Modal title={initial ? tr('Edit Server') : tr('Add External MCP Server')} close={onClose}>
      <form onSubmit={handleSubmit} className="form">
        {problem && <ProblemBanner message={problem} />}

        <label className="field">
          <span>{tr('Server ID')}</span>
          <input className="input" required disabled={!!initial} value={form.id}
            onChange={(e) => update('id', e.target.value)}
            placeholder="rea" />
          <span className="hint">{tr('Unique identifier (alphanumeric, underscore, hyphen). Used in tool names: mcp__{id}__tool.')}</span>
        </label>

        <label className="field">
          <span>{tr('Display Name')}</span>
          <input className="input" required value={form.name}
            onChange={(e) => update('name', e.target.value)} placeholder="REA" />
        </label>

        <label className="field">
          <span>{tr('Description')}</span>
          <input className="input" value={form.description ?? ''}
            onChange={(e) => update('description', e.target.value)} />
        </label>

        <label className="field checkbox">
          <input type="checkbox" checked={form.enabled}
            onChange={(e) => update('enabled', e.target.checked)} />
          <span>{tr('Enabled')}</span>
        </label>

        <label className="field checkbox">
          <input type="checkbox" checked={form.autoConnect}
            onChange={(e) => update('autoConnect', e.target.checked)} />
          <span>{tr('Auto-connect on startup')}</span>
        </label>

        <label className="field">
          <span>{tr('Transport Type')}</span>
          <select className="input" value={form.transport.type}
            onChange={(e) => setForm((f) => ({
              ...f,
              transport: e.target.value === 'stdio'
                ? { type: 'stdio', command: '', args: [], env: {}, cwd: null }
                : { type: 'streamable_http', url: '', headers: {} },
            }))}>
            <option value="stdio">{tr('stdio (subprocess)')}</option>
            <option value="streamable_http">{tr('Streamable HTTP (not yet supported)')}</option>
          </select>
        </label>

        {isStdio ? (
          <>
            <label className="field">
              <span>{tr('Executable')}</span>
              <input className="input" required
                value={(form.transport as { command: string }).command}
                onChange={(e) => updateTransport('command', e.target.value)}
                placeholder={String.raw`C:\Program Files\nodejs\node.exe`} />
            </label>
            <label className="field">
              <span>{tr('Arguments (one per line)')}</span>
              <textarea className="input" rows={4} value={argsText}
                onChange={(e) => setArgsText(e.target.value)}
                placeholder={`D:\\frank\\REA\\rea-package-unpacked\\package\\scripts\\rea.mjs\nmcp`} />
            </label>
            <label className="field">
              <span>{tr('Working Directory (optional)')}</span>
              <input className="input"
                value={(form.transport as { cwd?: string | null }).cwd ?? ''}
                onChange={(e) => updateTransport('cwd', e.target.value || null)} />
            </label>
          </>
        ) : (
          <label className="field">
            <span>{tr('Endpoint URL')}</span>
            <input className="input"
              value={(form.transport as { url: string }).url ?? ''}
              onChange={(e) => updateTransport('url', e.target.value)}
              placeholder="https://example.com/mcp" />
          </label>
        )}

        <label className="field">
          <span>{tr('Timeout (seconds)')}</span>
          <input className="input" type="number" min={5} max={300}
            value={form.timeoutSecs}
            onChange={(e) => update('timeoutSecs', Number(e.target.value))} />
        </label>

        <div className="form-actions">
          <button type="button" className="button" onClick={onClose}>{tr('Cancel')}</button>
          <button type="submit" className="button primary" disabled={saving}>
            {saving ? <LoaderCircle size={16} className="spin" /> : <Plug2 size={16} />}
            {initial ? tr('Save Changes') : tr('Add Server')}
          </button>
        </div>
      </form>
    </Modal>
  );
}
