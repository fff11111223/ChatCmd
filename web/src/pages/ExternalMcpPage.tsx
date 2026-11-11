import { CheckCircle2, Link2, LoaderCircle, Pencil, Plug, Plug2, Plus, RefreshCw, Search, Server, Trash2, WifiOff, XCircle } from 'lucide-react';
import { FormEvent, useState } from 'react';
import { api } from '../api';
import { Empty, ErrorState, Loading, Modal, PageHeading, ProblemBanner } from '../components';
import { tr } from '../i18n';
import { useLoad } from '../useLoad';
import type {
  ExternalMcpServer,
  ExternalMcpServerConfig,
  ExternalMcpToolSummary,
} from '../types';
import './ExternalMcpPage.css';

// ── Status badge ─────────────────────────────────────────────────────────────

function McpStatusBadge({ status }: { status: ExternalMcpServer['status'] }) {
  const s = status?.status;
  if (s === 'connected') {
    return (
      <span className="mcp-status-badge connected">
        <CheckCircle2 size={12} /> {tr('Connected')}
      </span>
    );
  }
  if (s === 'connecting') {
    return (
      <span className="mcp-status-badge connecting">
        <LoaderCircle size={12} className="spin" /> {tr('Connecting')}
      </span>
    );
  }
  if (s === 'error') {
    return (
      <span className="mcp-status-badge error" title={status.message}>
        <XCircle size={12} /> {tr('Error')}
      </span>
    );
  }
  return (
    <span className="mcp-status-badge disconnected">
      <WifiOff size={12} /> {tr('Disconnected')}
    </span>
  );
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
    servers.setData((cur) => (cur ? [next, ...cur.filter((s) => s.id !== next.id)] : [next]));

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
        cur?.map((s) => (s.id === id ? { ...s, status: res.status ?? s.status, toolsCount: res.toolCount ?? s.toolsCount } : s)) ?? []
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
        cur?.map((s) => (s.id === id ? { ...s, status: { status: 'disconnected' }, toolsCount: 0 } : s)) ?? []
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
          cur?.map((s) => (s.id === id ? { ...s, status: res.status ?? s.status, toolsCount: res.toolCount ?? s.toolsCount } : s)) ?? []
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
    <div className="mcp-page">
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
      {servers.loading ? (
        <Loading />
      ) : servers.error ? (
        <ErrorState message={servers.error} retry={servers.reload} />
      ) : (
        <>
          {(!servers.data || servers.data.length === 0) && (
            <Empty title={tr('No external MCP servers')} body={tr('Add a server to expose its tools to ChatGPT.')} />
          )}
          <div className="mcp-servers-grid">
            {(servers.data ?? []).map((server) => {
              const isConnected = server.status?.status === 'connected';
              return (
                <div key={server.id} className={`mcp-card ${isConnected ? 'connected' : ''}`}>
                  <div className="mcp-card-header">
                    <div className="mcp-card-title-group">
                      <div className={`mcp-card-icon ${isConnected ? 'connected' : ''}`}>
                        <Server size={20} />
                      </div>
                      <div className="mcp-card-title-content">
                        <div className="mcp-card-name-row">
                          <h3 className="mcp-card-name">{server.name}</h3>
                          <code className="mcp-id-tag">{server.id}</code>
                        </div>
                        {server.description && <p className="mcp-card-desc">{server.description}</p>}
                      </div>
                    </div>
                    <McpStatusBadge status={server.status} />
                  </div>

                  <div className="mcp-card-meta">
                    <span className="mcp-meta-item">
                      <Server size={13} />
                      <span>{tr('Transport')}:</span>
                      <code>{server.transportType}</code>
                    </span>
                    {isConnected && (
                      <span className="mcp-meta-item">
                        <Link2 size={13} />
                        <span><strong>{server.toolsCount}</strong> {tr('tools')}</span>
                      </span>
                    )}
                    <span className="mcp-meta-item">
                      <span>{tr('Status')}:</span>
                      <span>{server.enabled ? tr('Enabled') : tr('Disabled')}</span>
                    </span>
                  </div>

                  <div className="mcp-card-actions">
                    {isConnected ? (
                      <button className="button secondary compact" onClick={() => handleDisconnect(server.id)} disabled={connectingId === server.id}>
                        {connectingId === server.id ? <LoaderCircle size={14} className="spin" /> : <WifiOff size={14} />}
                        {tr('Disconnect')}
                      </button>
                    ) : (
                      <button className="button primary compact" onClick={() => handleConnect(server.id)} disabled={connectingId === server.id}>
                        {connectingId === server.id ? <LoaderCircle size={14} className="spin" /> : <Plug size={14} />}
                        {tr('Connect')}
                      </button>
                    )}
                    <button className="button secondary compact" onClick={() => handleTest(server.id)} disabled={testingId === server.id}>
                      {testingId === server.id ? <LoaderCircle size={14} className="spin" /> : <RefreshCw size={14} />}
                      {tr('Test')}
                    </button>
                    <button className="button secondary compact" onClick={() => setEditor(server)}>
                      <Pencil size={14} /> {tr('Edit')}
                    </button>
                    <button className="button danger compact" onClick={() => handleDelete(server.id)} disabled={deletingId === server.id}>
                      {deletingId === server.id ? <LoaderCircle size={14} className="spin" /> : <Trash2 size={14} />}
                      {tr('Delete')}
                    </button>
                  </div>
                </div>
              );
            })}
          </div>
        </>
      )}

      {/* Tool search */}
      <section className="mcp-search-panel">
        <div className="mcp-search-header">
          <h2>{tr('Tool Search')}</h2>
          <p>{tr('Search across all configured MCP tools to inspect arguments and schemas.')}</p>
        </div>
        <div className="mcp-search-bar">
          <div className="mcp-search-input-wrap">
            <Search size={16} className="mcp-search-icon" />
            <input
              className="mcp-search-input"
              placeholder={tr('Search tools across all servers…')}
              value={searchQ}
              onChange={(e) => setSearchQ(e.target.value)}
              onKeyDown={(e) => e.key === 'Enter' && handleSearch()}
            />
          </div>
          <button className="button secondary" onClick={handleSearch} disabled={loadingTools}>
            {loadingTools ? <LoaderCircle size={15} className="spin" /> : <Search size={15} />}
            {tr('Search')}
          </button>
        </div>
        {toolResults && (
          <div className="mcp-tool-results">
            <span className="mcp-results-count">{toolResults.total} {tr('tools found')}</span>
            <div className="mcp-tool-grid">
              {toolResults.tools.map((tool) => (
                <div key={tool.qualifiedName} className="mcp-tool-card" onClick={() => handleViewSchema(tool)}>
                  <div className="mcp-tool-header">
                    <code className="mcp-tool-name">{tool.qualifiedName}</code>
                    <span className="mcp-tool-view-hint">{tr('View schema')} →</span>
                  </div>
                  {tool.description && <p className="mcp-tool-desc">{tool.description}</p>}
                </div>
              ))}
            </div>
          </div>
        )}
      </section>

      {/* Schema modal */}
      {schemaView && (
        <Modal title={schemaView.qualifiedName} close={() => setSchemaView(undefined)}>
          <div className="mcp-schema-view">
            {schemaView.description && <p className="mcp-schema-desc">{schemaView.description}</p>}
            <pre className="mcp-schema-code">{JSON.stringify(schemaView.schema, null, 2)}</pre>
          </div>
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
      <form onSubmit={handleSubmit} className="mcp-form-stack">
        {problem && <ProblemBanner message={problem} />}

        <div className="mcp-form-grid-2">
          <label className="mcp-field">
            <span className="mcp-field-label">
              {tr('Server ID')} <span className="required">*</span>
            </span>
            <input
              className="mcp-input mono"
              required
              disabled={!!initial}
              value={form.id}
              onChange={(e) => update('id', e.target.value)}
              placeholder="e.g. rea"
            />
            <span className="mcp-field-hint">
              {tr('Unique identifier. Used in tool names: mcp__{id}__tool')}
            </span>
          </label>

          <label className="mcp-field">
            <span className="mcp-field-label">
              {tr('Display Name')} <span className="required">*</span>
            </span>
            <input
              className="mcp-input"
              required
              value={form.name}
              onChange={(e) => update('name', e.target.value)}
              placeholder="e.g. REA"
            />
            <span className="mcp-field-hint">
              {tr('Human-readable title displayed in cards and menus')}
            </span>
          </label>
        </div>

        <label className="mcp-field">
          <span className="mcp-field-label">{tr('Description')}</span>
          <input
            className="mcp-input"
            value={form.description ?? ''}
            onChange={(e) => update('description', e.target.value)}
            placeholder={tr('Optional summary of what this MCP server provides')}
          />
        </label>

        <div className="mcp-form-checkboxes">
          <label className="mcp-checkbox-card">
            <input
              type="checkbox"
              checked={form.enabled}
              onChange={(e) => update('enabled', e.target.checked)}
            />
            <span className="mcp-checkbox-label">
              <strong className="mcp-checkbox-title">{tr('Enabled')}</strong>
              <small className="mcp-checkbox-desc">{tr('Allow tools from this server to be exposed')}</small>
            </span>
          </label>

          <label className="mcp-checkbox-card">
            <input
              type="checkbox"
              checked={form.autoConnect}
              onChange={(e) => update('autoConnect', e.target.checked)}
            />
            <span className="mcp-checkbox-label">
              <strong className="mcp-checkbox-title">{tr('Auto-connect on startup')}</strong>
              <small className="mcp-checkbox-desc">{tr('Connect automatically when ChatCMD starts')}</small>
            </span>
          </label>
        </div>

        <div className="mcp-form-divider" />

        <div className="mcp-form-grid-2">
          <label className="mcp-field">
            <span className="mcp-field-label">{tr('Transport Type')}</span>
            <select
              className="mcp-select"
              value={form.transport.type}
              onChange={(e) =>
                setForm((f) => ({
                  ...f,
                  transport:
                    e.target.value === 'stdio'
                      ? { type: 'stdio', command: '', args: [], env: {}, cwd: null }
                      : { type: 'streamable_http', url: '', headers: {} },
                }))
              }
            >
              <option value="stdio">{tr('stdio (subprocess)')}</option>
              <option value="streamable_http">{tr('Streamable HTTP (not yet supported)')}</option>
            </select>
          </label>

          <label className="mcp-field">
            <span className="mcp-field-label">{tr('Timeout (seconds)')}</span>
            <input
              className="mcp-input"
              type="number"
              min={5}
              max={300}
              value={form.timeoutSecs}
              onChange={(e) => update('timeoutSecs', Number(e.target.value))}
            />
          </label>
        </div>

        {isStdio ? (
          <>
            <label className="mcp-field">
              <span className="mcp-field-label">
                {tr('Executable')} <span className="required">*</span>
              </span>
              <input
                className="mcp-input mono"
                required
                value={(form.transport as { command: string }).command}
                onChange={(e) => updateTransport('command', e.target.value)}
                placeholder={String.raw`C:\Program Files\nodejs\node.exe`}
              />
              <span className="mcp-field-hint">
                {tr('Command name or full path to the executable')}
              </span>
            </label>

            <label className="mcp-field">
              <span className="mcp-field-label">{tr('Arguments (one per line)')}</span>
              <textarea
                className="mcp-textarea mono"
                rows={4}
                value={argsText}
                onChange={(e) => setArgsText(e.target.value)}
                placeholder={`D:\\frank\\REA\\rea-package-unpacked\\package\\scripts\\rea.mjs\nmcp`}
              />
            </label>

            <label className="mcp-field">
              <span className="mcp-field-label">{tr('Working Directory (optional)')}</span>
              <input
                className="mcp-input mono"
                value={(form.transport as { cwd?: string | null }).cwd ?? ''}
                onChange={(e) => updateTransport('cwd', e.target.value || null)}
                placeholder={String.raw`D:\frank\REA`}
              />
            </label>
          </>
        ) : (
          <label className="mcp-field">
            <span className="mcp-field-label">
              {tr('Endpoint URL')} <span className="required">*</span>
            </span>
            <input
              className="mcp-input"
              value={(form.transport as { url: string }).url ?? ''}
              onChange={(e) => updateTransport('url', e.target.value)}
              placeholder="https://example.com/mcp"
            />
          </label>
        )}

        <div className="mcp-modal-actions">
          <button type="button" className="button secondary" onClick={onClose}>
            {tr('Cancel')}
          </button>
          <button type="submit" className="button primary" disabled={saving}>
            {saving ? <LoaderCircle size={16} className="spin" /> : <Plug2 size={16} />}
            {initial ? tr('Save Changes') : tr('Add Server')}
          </button>
        </div>
      </form>
    </Modal>
  );
}
