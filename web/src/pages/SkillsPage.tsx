import { FormEvent, useCallback, useEffect, useMemo, useRef, useState } from 'react';
import {
  AlertCircle,
  AlertTriangle,
  ArrowLeft,
  Blocks,
  BookOpen,
  Check,
  CircleAlert,
  ExternalLink,
  Eye,
  FileText,
  Folder,
  FolderGit2,
  GitFork,
  Info,
  Layers,
  LoaderCircle,
  Plus,
  RefreshCw,
  Search,
  Settings2,
  ShieldAlert,
  Trash2,
  X,
} from 'lucide-react';
import { api } from '../api';
import { tr } from '../i18n';
import type {
  GlobalSkillDiagnostics,
  LocalSkillSource,
  ResidentPreview,
  ResidentUsage,
  SkillInstallPreview,
  SkillOptionValue,
  SkillSourceLintResult,
  UserSkill,
  UserSkillOption,
} from '../types';

type LoadState = 'loading' | 'ready' | 'error';

export function SkillsPage() {
  const [skills, setSkills] = useState<UserSkill[]>([]);
  const [sources, setSources] = useState<LocalSkillSource[]>([]);
  const [usage, setUsage] = useState<ResidentUsage | null>(null);
  const [diagnostics, setDiagnostics] = useState<GlobalSkillDiagnostics | null>(null);

  const [loadState, setLoadState] = useState<LoadState>('loading');
  const [error, setError] = useState<string>();
  const [busy, setBusy] = useState<string>();

  // Modals
  const [addModalOpen, setAddModalOpen] = useState(false);
  const [previewModalOpen, setPreviewModalOpen] = useState(false);
  const [detailSkill, setDetailSkill] = useState<UserSkill>();
  const [optionsSkill, setOptionsSkill] = useState<UserSkill>();
  const [deleteSkill, setDeleteSkill] = useState<UserSkill>();
  const [removeSource, setRemoveSource] = useState<LocalSkillSource>();
  const [lintResult, setLintResult] = useState<{ source: LocalSkillSource; results: SkillSourceLintResult[] }>();

  // Filters
  const [query, setQuery] = useState('');
  const [filter, setFilter] = useState<'all' | 'enabled' | 'disabled'>('all');
  const [sourceFilter, setSourceFilter] = useState<string>('all');
  const [showDiagnostics, setShowDiagnostics] = useState(false);

  const load = useCallback(async () => {
    setLoadState('loading');
    setError(undefined);
    try {
      const [skillsData, sourcesData, usageData, diagData] = await Promise.all([
        api.skills().catch((err) => { console.error('Failed to load skills', err); return [] as UserSkill[]; }),
        api.localSources().catch((err) => { console.error('Failed to load sources', err); return [] as LocalSkillSource[]; }),
        api.residentUsage().catch((err) => { console.error('Failed to load usage', err); return null; }),
        api.skillDiagnostics().catch((err) => { console.error('Failed to load diagnostics', err); return null; }),
      ]);
      setSkills(skillsData);
      setSources(sourcesData);
      setUsage(usageData);
      setDiagnostics(diagData);
      setLoadState('ready');
    } catch (reason) {
      setError(message(reason, tr('Could not load skills.')));
      setLoadState('error');
    }
  }, []);

  useEffect(() => {
    void load();
  }, [load]);

  const enabledCount = useMemo(() => skills.filter((skill) => skill.enabled).length, [skills]);
  const configurableCount = useMemo(() => skills.filter((skill) => skill.options?.length).length, [skills]);

  const visibleSkills = useMemo(() => {
    const normalizedQuery = query.trim().toLocaleLowerCase();
    return skills.filter((skill) => {
      if (filter === 'enabled' && !skill.enabled) return false;
      if (filter === 'disabled' && skill.enabled) return false;
      if (sourceFilter !== 'all') {
        const type = (skill.sourceType || skill.source || '').toLowerCase();
        if (type !== sourceFilter.toLowerCase()) return false;
      }
      if (!normalizedQuery) return true;
      return [
        skill.title,
        skill.description,
        skill.sourceUrl,
        skill.overrides,
        skill.sourceType,
      ].some((value) => value?.toLocaleLowerCase().includes(normalizedQuery));
    });
  }, [filter, skills, sourceFilter, query]);

  function replaceSkill(next: UserSkill) {
    setSkills((current) => current.map((skill) => (skill.id === next.id ? next : skill)));
    setOptionsSkill((current) => (current?.id === next.id ? next : current));
    if (detailSkill?.id === next.id) setDetailSkill(next);
  }

  async function toggle(skill: UserSkill) {
    setBusy(`toggle:${skill.id}`);
    setError(undefined);
    try {
      replaceSkill(await api.setSkillEnabled(skill.id, !skill.enabled));
      // Refresh usage after toggling
      api.residentUsage().then(setUsage).catch(() => {});
    } catch (reason) {
      setError(message(reason, tr('Could not change skill status.')));
    } finally {
      setBusy(undefined);
    }
  }

  async function previewGit(repositoryUrl: string) {
    setBusy('preview');
    setError(undefined);
    try {
      return await api.previewSkills(repositoryUrl);
    } catch (reason) {
      throw new Error(message(reason, tr('Could not scan skills from GitHub.')), { cause: reason });
    } finally {
      setBusy(undefined);
    }
  }

  async function installGit(repositoryUrl: string, skillPaths: string[]) {
    setBusy('install');
    setError(undefined);
    try {
      const { skills: installed } = await api.installSkills(repositoryUrl, skillPaths);
      const installedIds = new Set(installed.map((skill) => skill.id));
      setSkills((current) => [...installed, ...current.filter((skill) => !installedIds.has(skill.id))]);
      setAddModalOpen(false);
      void load();
    } catch (reason) {
      throw new Error(message(reason, tr('Could not install skills from GitHub.')), { cause: reason });
    } finally {
      setBusy(undefined);
    }
  }

  async function addLocalSource(path: string, scope: string) {
    setBusy('add-local-source');
    setError(undefined);
    try {
      await api.addLocalSource(path, scope);
      setAddModalOpen(false);
      await load();
    } catch (reason) {
      throw new Error(message(reason, tr('Could not add local folder source.')), { cause: reason });
    } finally {
      setBusy(undefined);
    }
  }

  async function reloadSource(source: LocalSkillSource) {
    setBusy(`reload:${source.id}`);
    setError(undefined);
    try {
      await api.reloadLocalSource(source.id);
      await load();
    } catch (reason) {
      setError(message(reason, tr('Could not reload source.')));
    } finally {
      setBusy(undefined);
    }
  }

  async function removeLocalSourceConfirmed(source: LocalSkillSource) {
    setBusy(`remove-source:${source.id}`);
    setError(undefined);
    try {
      await api.removeLocalSource(source.id);
      setRemoveSource(undefined);
      await load();
    } catch (reason) {
      setError(message(reason, tr('Could not remove source.')));
    } finally {
      setBusy(undefined);
    }
  }

  async function runLint(source: LocalSkillSource) {
    setBusy(`lint:${source.id}`);
    setError(undefined);
    try {
      const results = await api.lintLocalSource(source.id);
      setLintResult({ source, results });
    } catch (reason) {
      setError(message(reason, tr('Could not run lint on source.')));
    } finally {
      setBusy(undefined);
    }
  }

  async function saveOptions(skill: UserSkill, options: Record<string, SkillOptionValue>) {
    setBusy(`options:${skill.id}`);
    try {
      replaceSkill(await api.updateSkillOptions(skill.id, options));
      setOptionsSkill(undefined);
    } catch (reason) {
      throw new Error(message(reason, tr('Could not save skill options.')), { cause: reason });
    } finally {
      setBusy(undefined);
    }
  }

  async function removeSkill(skill: UserSkill) {
    setBusy(`delete:${skill.id}`);
    setError(undefined);
    try {
      await api.deleteSkill(skill.id);
      setSkills((current) => current.filter((item) => item.id !== skill.id));
      setDeleteSkill(undefined);
      if (detailSkill?.id === skill.id) setDetailSkill(undefined);
      void load();
    } catch (reason) {
      setError(message(reason, tr('Could not delete skill.')));
    } finally {
      setBusy(undefined);
    }
  }

  // Resident usage percentage
  const usedChars = usage?.usedChars ?? 0;
  const limitChars = usage?.limitChars || 800;
  const usagePercent = Math.min(100, Math.round((usedChars / limitChars) * 100));
  const isOverLimit = usedChars > limitChars;
  const isNearLimit = usedChars >= limitChars * 0.85;

  return (
    <div className="skills-page">
      <header className="skills-heading page-heading">
        <span className="eyebrow">
          <Blocks /> {tr('AGENT SKILLS')}
        </span>
        <div className="skills-title-row">
          <div>
            <h1>{tr('Your skills')}</h1>
            <p>{tr('Manage global and project skills that Agents can choose while handling tasks.')}</p>
          </div>
          <button className="button primary skills-add" onClick={() => setAddModalOpen(true)}>
            <Plus /> {tr('Add skill')}
          </button>
        </div>

        {loadState === 'ready' && (
          <div className="skills-stats" aria-label={tr('Skills overview')}>
            <span>
              <strong>{skills.length}</strong>
              {tr('Total skills')}
            </span>
            <span>
              <strong>{enabledCount}</strong>
              {tr('Enabled')}
            </span>
            <span>
              <strong>{configurableCount}</strong>
              {tr('Configurable')}
            </span>
            <span>
              <strong>{sources.length}</strong>
              {tr('Local sources')}
            </span>
          </div>
        )}

        {/* Resident usage budget bar */}
        {loadState === 'ready' && (
          <div
            style={{
              marginTop: '18px',
              padding: '14px 16px',
              background: 'var(--surface)',
              border: `1px solid ${isOverLimit ? 'var(--bad, #e53935)' : isNearLimit ? 'var(--warning, #fb8c00)' : 'var(--border)'}`,
              borderRadius: '12px',
              display: 'flex',
              flexDirection: 'column',
              gap: '8px',
            }}
          >
            <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', flexWrap: 'wrap', gap: '8px' }}>
              <div style={{ display: 'flex', alignItems: 'center', gap: '8px' }}>
                <Layers style={{ width: '16px', height: '16px', color: 'var(--accent)' }} />
                <span style={{ fontSize: '0.85rem', fontWeight: 600 }}>{tr('Resident Instruction Usage')}</span>
                <span
                  style={{
                    fontSize: '0.78rem',
                    padding: '2px 8px',
                    borderRadius: '6px',
                    fontWeight: 600,
                    background: isOverLimit ? 'color-mix(in srgb, var(--bad, #e53935) 15%, transparent)' : isNearLimit ? 'color-mix(in srgb, var(--warning, #fb8c00) 15%, transparent)' : 'var(--surface-3)',
                    color: isOverLimit ? 'var(--bad, #e53935)' : isNearLimit ? 'var(--warning, #fb8c00)' : 'var(--text)',
                  }}
                >
                  {usedChars} / {limitChars} {tr('chars')}
                </span>
                {isOverLimit && (
                  <span style={{ fontSize: '0.75rem', color: 'var(--bad, #e53935)', display: 'inline-flex', alignItems: 'center', gap: '4px' }}>
                    <AlertCircle style={{ width: '13px', height: '13px' }} /> {tr('Exceeds budget limit (will be truncated)')}
                  </span>
                )}
              </div>
              <div style={{ display: 'flex', gap: '8px', alignItems: 'center' }}>
                <button
                  type="button"
                  className="button secondary"
                  style={{ fontSize: '0.75rem', padding: '4px 10px', height: '32px' }}
                  onClick={() => setPreviewModalOpen(true)}
                >
                  <Eye style={{ width: '14px', height: '14px' }} /> {tr('Model preview')}
                </button>
                <button
                  type="button"
                  className="button secondary"
                  style={{ fontSize: '0.75rem', padding: '4px 10px', height: '32px' }}
                  onClick={() => setShowDiagnostics((prev) => !prev)}
                >
                  <Info style={{ width: '14px', height: '14px' }} /> {showDiagnostics ? tr('Hide diagnostics') : tr('Diagnostics')}
                </button>
              </div>
            </div>

            {/* Progress bar */}
            <div
              style={{
                width: '100%',
                height: '7px',
                background: 'var(--surface-3)',
                borderRadius: '999px',
                overflow: 'hidden',
              }}
            >
              <div
                style={{
                  width: `${usagePercent}%`,
                  height: '100%',
                  background: isOverLimit ? 'var(--bad, #e53935)' : isNearLimit ? 'var(--warning, #fb8c00)' : 'var(--accent)',
                  transition: 'width 200ms ease',
                }}
              />
            </div>
          </div>
        )}
      </header>

      {error && (
        <div className="skills-error" role="alert">
          <CircleAlert />
          <span>{error}</span>
          {loadState === 'error' && (
            <button onClick={() => void load()}>
              <RefreshCw /> {tr('Retry')}
            </button>
          )}
          <button className="plain-icon" aria-label={tr('Close notification')} onClick={() => setError(undefined)}>
            <X />
          </button>
        </div>
      )}

      {/* Read-only Global Diagnostics section */}
      {showDiagnostics && diagnostics && (
        <section
          style={{
            margin: '20px 0',
            padding: '16px',
            background: 'var(--surface)',
            border: '1px solid var(--border)',
            borderRadius: '12px',
          }}
        >
          <div style={{ display: 'flex', alignItems: 'center', gap: '8px', marginBottom: '10px' }}>
            <Folder style={{ width: '18px', height: '18px', color: 'var(--accent)' }} />
            <h3 style={{ margin: 0, fontSize: '0.95rem' }}>{tr('Global Skills Diagnostics')}</h3>
          </div>
          <div style={{ fontSize: '0.8rem', display: 'grid', gap: '6px', color: 'var(--muted)' }}>
            <div>
              <strong style={{ color: 'var(--text)' }}>{tr('Global Skills Directory:')}</strong> <code>{diagnostics.globalSkillsDir || tr('Not set')}</code>
            </div>
            <div>
              <strong style={{ color: 'var(--text)' }}>{tr('Loaded Skills in Folder:')}</strong> {diagnostics.skillCount}
            </div>
            <div>
              <strong style={{ color: 'var(--text)' }}>{tr('Skipped Subdirectories:')}</strong> {diagnostics.skippedSubdirectories.length}
            </div>
          </div>

          {diagnostics.skippedSubdirectories.length > 0 && (
            <div style={{ marginTop: '12px' }}>
              <div style={{ fontSize: '0.78rem', fontWeight: 600, marginBottom: '6px', color: 'var(--muted)' }}>{tr('Skipped subdirectories and reasons:')}</div>
              <ul style={{ margin: 0, paddingLeft: '18px', fontSize: '0.75rem', display: 'grid', gap: '4px' }}>
                {diagnostics.skippedSubdirectories.map((item) => (
                  <li key={item.path}>
                    <code style={{ fontWeight: 600 }}>{item.name}</code>: <span style={{ color: 'var(--bad, #e53935)' }}>{item.reason}</span>
                  </li>
                ))}
              </ul>
            </div>
          )}
        </section>
      )}

      {/* Local Sources List section */}
      {sources.length > 0 && (
        <section
          style={{
            margin: '20px 0',
            padding: '16px',
            background: 'var(--surface)',
            border: '1px solid var(--border)',
            borderRadius: '12px',
          }}
        >
          <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: '12px' }}>
            <div style={{ display: 'flex', alignItems: 'center', gap: '8px' }}>
              <FolderGit2 style={{ width: '18px', height: '18px', color: 'var(--accent)' }} />
              <h3 style={{ margin: 0, fontSize: '0.95rem' }}>{tr('Registered Local Sources')} ({sources.length})</h3>
            </div>
          </div>
          <div style={{ display: 'grid', gap: '8px' }}>
            {sources.map((src) => {
              const reloading = busy === `reload:${src.id}`;
              const linting = busy === `lint:${src.id}`;
              return (
                <div
                  key={src.id}
                  style={{
                    display: 'flex',
                    alignItems: 'center',
                    justifyContent: 'space-between',
                    padding: '10px 12px',
                    background: 'var(--surface-3)',
                    borderRadius: '8px',
                    fontSize: '0.8rem',
                    flexWrap: 'wrap',
                    gap: '8px',
                  }}
                >
                  <div style={{ display: 'grid', gap: '2px', minWidth: '0' }}>
                    <div style={{ display: 'flex', alignItems: 'center', gap: '8px' }}>
                      <strong style={{ overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>{src.path}</strong>
                      <span
                        style={{
                          fontSize: '0.65rem',
                          padding: '1px 6px',
                          background: 'var(--surface)',
                          borderRadius: '4px',
                          border: '1px solid var(--border)',
                          textTransform: 'uppercase',
                        }}
                      >
                        {src.scope}
                      </span>
                    </div>
                  </div>
                  <div style={{ display: 'flex', gap: '6px', alignItems: 'center' }}>
                    <button
                      type="button"
                      className="button secondary"
                      style={{ fontSize: '0.72rem', height: '28px', padding: '0 8px' }}
                      disabled={busy !== undefined}
                      onClick={() => void runLint(src)}
                    >
                      {linting ? <LoaderCircle className="spin" style={{ width: '12px', height: '12px' }} /> : <FileText style={{ width: '12px', height: '12px' }} />}
                      {tr('Lint')}
                    </button>
                    <button
                      type="button"
                      className="button secondary"
                      style={{ fontSize: '0.72rem', height: '28px', padding: '0 8px' }}
                      disabled={busy !== undefined}
                      onClick={() => void reloadSource(src)}
                    >
                      {reloading ? <LoaderCircle className="spin" style={{ width: '12px', height: '12px' }} /> : <RefreshCw style={{ width: '12px', height: '12px' }} />}
                      {tr('Reload')}
                    </button>
                    <button
                      type="button"
                      className="plain-icon danger-icon"
                      style={{ width: '28px', height: '28px' }}
                      aria-label={tr('Remove source')}
                      disabled={busy !== undefined}
                      onClick={() => setRemoveSource(src)}
                    >
                      <Trash2 style={{ width: '14px', height: '14px' }} />
                    </button>
                  </div>
                </div>
              );
            })}
          </div>
        </section>
      )}

      {loadState === 'loading' ? (
        <div className="skills-skeleton" role="status" aria-live="polite" aria-busy="true" aria-label={tr('Loading')}>
          <span />
          <span />
          <span />
          <span />
        </div>
      ) : loadState === 'ready' && !skills.length ? (
        <section className="skills-empty">
          <span>
            <Blocks />
          </span>
          <h2>{tr('No skills available')}</h2>
          <p>{tr('Install skills from GitHub or link a local folder to give Agents specialized guidance and workflows.')}</p>
          <button className="button primary" onClick={() => setAddModalOpen(true)}>
            <Plus /> {tr('Add skill')}
          </button>
        </section>
      ) : (
        <>
          <div className="skills-toolbar">
            <label className="skills-search">
              <Search />
              <span className="sr-only">{tr('Search skills')}</span>
              <input value={query} onChange={(event) => setQuery(event.target.value)} placeholder={tr('Search skills…')} />
            </label>

            <div style={{ display: 'flex', gap: '8px', alignItems: 'center', flexWrap: 'wrap' }}>
              <div className="skills-filters" role="group" aria-label={tr('Filter status')}>
                {(['all', 'enabled', 'disabled'] as const).map((value) => (
                  <button
                    key={value}
                    type="button"
                    className={filter === value ? 'active' : ''}
                    aria-pressed={filter === value}
                    onClick={() => setFilter(value)}
                  >
                    {value === 'all' ? tr('All') : value === 'enabled' ? tr('Enabled') : tr('Disabled')}
                  </button>
                ))}
              </div>

              <select
                aria-label={tr('Filter by source')}
                value={sourceFilter}
                onChange={(e) => setSourceFilter(e.target.value)}
                style={{
                  height: '36px',
                  borderRadius: '8px',
                  background: 'var(--surface)',
                  border: '1px solid var(--border)',
                  color: 'var(--text)',
                  fontSize: '0.78rem',
                  padding: '0 8px',
                }}
              >
                <option value="all">{tr('All Sources')}</option>
                <option value="global">{tr('Global')}</option>
                <option value="project">{tr('Project')}</option>
                <option value="workspace">{tr('Workspace')}</option>
                <option value="user_home">{tr('User Home')}</option>
              </select>
            </div>
          </div>

          {visibleSkills.length ? (
            <div className="skills-list" aria-label={tr('Skills list')}>
              {visibleSkills.map((skill) => {
                const toggling = busy === `toggle:${skill.id}`;
                const sourceType = (skill.sourceType || skill.source || 'global').toLowerCase();
                const isLegacyNoResident = sourceType === 'workspace' || sourceType === 'user_home';
                const hasErrors = (skill.errors?.length ?? 0) > 0;
                const hasWarnings = (skill.warnings?.length ?? 0) > 0;

                const sourceLabel =
                  sourceType === 'project'
                    ? tr('Project')
                    : sourceType === 'workspace'
                    ? tr('Workspace')
                    : sourceType === 'user_home'
                    ? tr('User Home')
                    : tr('Global');

                return (
                  <article
                    className={`skill-card ${skill.enabled ? 'enabled' : 'disabled'}`}
                    key={skill.id}
                    style={{ cursor: 'pointer' }}
                    onClick={(e) => {
                      // Prevent modal if user clicked toggle or controls
                      if ((e.target as HTMLElement).closest('.skill-controls, a')) return;
                      setDetailSkill(skill);
                    }}
                  >
                    <header className="skill-card-header">
                      <SkillIcon url={skill.iconUrl} />
                      <div className="skill-copy">
                        <div className="skill-title" style={{ display: 'flex', alignItems: 'center', gap: '6px', flexWrap: 'wrap' }}>
                          <h2>{skill.title}</h2>

                          {/* Source badge */}
                          <span style={{ background: sourceType === 'project' ? 'color-mix(in srgb, #4caf50 15%, var(--surface))' : undefined }}>
                            {sourceLabel}
                          </span>

                          {/* 3-tier badges */}
                          {skill.hasResident && (
                            <span style={{ background: 'color-mix(in srgb, #2196f3 15%, var(--surface))', color: '#1976d2' }}>
                              {tr('Resident')}{skill.residentChars ? ` (${skill.residentChars})` : ''}
                            </span>
                          )}
                          {skill.hasCore && (
                            <span style={{ background: 'color-mix(in srgb, #9c27b0 15%, var(--surface))', color: '#7b1fa2' }}>
                              {tr('Core')}
                            </span>
                          )}
                          {skill.examples && skill.examples.length > 0 && (
                            <span style={{ background: 'color-mix(in srgb, #ff9800 15%, var(--surface))', color: '#f57c00' }}>
                              {tr('{count} examples', { count: skill.examples.length })}
                            </span>
                          )}

                          {/* Overrides badge */}
                          {skill.overrides && (
                            <span style={{ background: 'color-mix(in srgb, #e91e63 15%, var(--surface))', color: '#c2185b' }}>
                              {tr('Overrides: {name}', { name: skill.overrides })}
                            </span>
                          )}

                          {/* Legacy badge */}
                          {isLegacyNoResident && (
                            <span style={{ background: 'color-mix(in srgb, #607d8b 15%, var(--surface))', color: '#455a64' }}>
                              {tr('No resident support')}
                            </span>
                          )}

                          {/* Warnings / Errors */}
                          {hasErrors && (
                            <span style={{ background: 'color-mix(in srgb, var(--bad, #e53935) 20%, var(--surface))', color: 'var(--bad, #e53935)' }} title={skill.errors?.join('\n')}>
                              <AlertCircle style={{ width: '11px', height: '11px', display: 'inline', marginRight: '3px' }} />
                              {tr('Errors')}
                            </span>
                          )}
                          {!hasErrors && hasWarnings && (
                            <span style={{ background: 'color-mix(in srgb, var(--warning, #fb8c00) 20%, var(--surface))', color: 'var(--warning, #fb8c00)' }} title={skill.warnings?.join('\n')}>
                              <AlertTriangle style={{ width: '11px', height: '11px', display: 'inline', marginRight: '3px' }} />
                              {tr('Warnings')}
                            </span>
                          )}
                        </div>
                        <p>{skill.description || tr('This skill has no description yet.')}</p>
                      </div>
                    </header>

                    <footer className="skill-card-footer">
                      <div className="skill-source">
                        {skill.sourceUrl ? (
                          <a href={skill.sourceUrl} target="_blank" rel="noreferrer" onClick={(e) => e.stopPropagation()}>
                            <GitFork />
                            <span>{shortRepository(skill.sourceUrl)}</span>
                            <ExternalLink />
                          </a>
                        ) : (
                          <span>
                            <Blocks />
                            {tr('Local skill')}
                          </span>
                        )}
                      </div>
                      <div className="skill-controls" onClick={(e) => e.stopPropagation()}>
                        {!!skill.options?.length && (
                          <button className="skill-configure" disabled={busy !== undefined} onClick={() => setOptionsSkill(skill)}>
                            <Settings2 />
                            {tr('Configure')}
                          </button>
                        )}
                        {skill.canDelete !== false && (
                          <button
                            className="plain-icon danger-icon"
                            aria-label={tr('Delete skill {name}', { name: skill.title })}
                            disabled={busy !== undefined}
                            onClick={() => setDeleteSkill(skill)}
                          >
                            <Trash2 />
                          </button>
                        )}
                        <label className="skill-toggle-control">
                          <span>{skill.enabled ? tr('On') : tr('Off')}</span>
                          <span className="agent-switch">
                            <span className="sr-only">{tr('Enable or disable skill {name}', { name: skill.title })}</span>
                            <input
                              type="checkbox"
                              role="switch"
                              aria-checked={skill.enabled}
                              checked={skill.enabled}
                              disabled={busy !== undefined}
                              onChange={() => void toggle(skill)}
                            />
                            <i>{toggling && <LoaderCircle className="spin" />}</i>
                          </span>
                        </label>
                      </div>
                    </footer>
                  </article>
                );
              })}
            </div>
          ) : (
            <section className="skills-no-results">
              <Search />
              <h2>{tr('No matching skills')}</h2>
              <p>{tr('Try another search term or change the current filter.')}</p>
              <button
                type="button"
                className="button secondary"
                onClick={() => {
                  setQuery('');
                  setFilter('all');
                  setSourceFilter('all');
                }}
              >
                {tr('Clear filters')}
              </button>
            </section>
          )}
        </>
      )}

      {/* Add Skill Modal (Tabs: Git / Local Folder) */}
      {addModalOpen && (
        <AddSkillModal
          busy={busy}
          onPreviewGit={previewGit}
          onInstallGit={installGit}
          onAddLocalSource={addLocalSource}
          onClose={() => setAddModalOpen(false)}
        />
      )}

      {/* Read-only Skill Detail Modal */}
      {detailSkill && (
        <SkillDetailModal
          skill={detailSkill}
          sources={sources}
          onClose={() => setDetailSkill(undefined)}
        />
      )}

      {/* Resident Model Preview Modal */}
      {previewModalOpen && (
        <ResidentPreviewModal onClose={() => setPreviewModalOpen(false)} />
      )}

      {/* Options Modal */}
      {optionsSkill && (
        <SkillOptionsModal
          skill={optionsSkill}
          busy={busy === `options:${optionsSkill.id}`}
          onSave={saveOptions}
          onClose={() => setOptionsSkill(undefined)}
        />
      )}

      {/* Confirm Delete Skill Modal */}
      {deleteSkill && (
        <ConfirmDeleteModal
          skill={deleteSkill}
          busy={busy === `delete:${deleteSkill.id}`}
          onConfirm={() => void removeSkill(deleteSkill)}
          onClose={() => setDeleteSkill(undefined)}
        />
      )}

      {/* Confirm Remove Source Modal */}
      {removeSource && (
        <ConfirmRemoveSourceModal
          source={removeSource}
          busy={busy === `remove-source:${removeSource.id}`}
          onConfirm={() => void removeLocalSourceConfirmed(removeSource)}
          onClose={() => setRemoveSource(undefined)}
        />
      )}

      {/* Lint Result Modal */}
      {lintResult && (
        <LintResultModal
          source={lintResult.source}
          results={lintResult.results}
          onClose={() => setLintResult(undefined)}
        />
      )}
    </div>
  );
}

// ── Modals & Subcomponents ──────────────────────────────────────────────────

function AddSkillModal({
  busy,
  onPreviewGit,
  onInstallGit,
  onAddLocalSource,
  onClose,
}: {
  busy?: string;
  onPreviewGit: (url: string) => Promise<SkillInstallPreview>;
  onInstallGit: (url: string, paths: string[]) => Promise<void>;
  onAddLocalSource: (path: string, scope: string) => Promise<void>;
  onClose: () => void;
}) {
  const [tab, setTab] = useState<'git' | 'local'>('git');

  // Git state
  const [gitUrl, setGitUrl] = useState('');
  const [gitPreview, setGitPreview] = useState<SkillInstallPreview>();
  const [selectedPaths, setSelectedPaths] = useState<Set<string>>(() => new Set());
  const [gitTouched, setGitTouched] = useState(false);
  const [gitError, setGitError] = useState<string>();

  // Local state
  const [localPath, setLocalPath] = useState('');
  const [localScope, setLocalScope] = useState('global');
  const [localError, setLocalError] = useState<string>();

  const isBusy = busy !== undefined;
  const dialogRef = useDialogFocus<HTMLFormElement>(onClose, isBusy);

  // Git logic
  const normalizedGitUrl = normalizeGitHubRepositoryUrl(gitUrl);
  const availableSkills = gitPreview?.skills.filter((skill) => !skill.installed) ?? [];
  const installedCount = (gitPreview?.skills.length ?? 0) - availableSkills.length;
  const allSelected = availableSkills.length > 0 && availableSkills.every((skill) => selectedPaths.has(skill.path));

  async function submitGit(event: FormEvent) {
    event.preventDefault();
    if (isBusy) return;
    setGitError(undefined);
    if (!gitPreview) {
      setGitTouched(true);
      if (!normalizedGitUrl) return;
      try {
        const result = await onPreviewGit(normalizedGitUrl);
        setGitPreview(result);
        setGitUrl(result.repositoryUrl);
        setSelectedPaths(new Set(result.skills.filter((skill) => !skill.installed).map((skill) => skill.path)));
      } catch (reason) {
        setGitError(message(reason, tr('Could not scan skills from GitHub.')));
      }
      return;
    }
    if (!selectedPaths.size) return;
    try {
      await onInstallGit(gitPreview.repositoryUrl, [...selectedPaths]);
    } catch (reason) {
      setGitError(message(reason, tr('Could not install skills from GitHub.')));
    }
  }

  async function submitLocal(event: FormEvent) {
    event.preventDefault();
    if (isBusy) return;
    setLocalError(undefined);
    if (!localPath.trim()) {
      setLocalError(tr('Folder path is required.'));
      return;
    }
    try {
      await onAddLocalSource(localPath.trim(), localScope.trim() || 'global');
    } catch (reason) {
      setLocalError(message(reason, tr('Could not add local folder source.')));
    }
  }

  return (
    <div
      className="skill-modal-backdrop"
      onMouseDown={(event) => {
        if (event.target === event.currentTarget && !isBusy) onClose();
      }}
    >
      <form
        ref={dialogRef}
        className={`skill-modal ${gitPreview ? 'skill-install-review' : ''}`}
        role="dialog"
        aria-modal="true"
        aria-labelledby="add-skill-title"
        onSubmit={tab === 'git' ? submitGit : submitLocal}
      >
        <header>
          <span>
            <Plus />
          </span>
          <div>
            <h2 id="add-skill-title">{tr('Add Skill')}</h2>
            <p>{tr('Choose how you want to add new skills to ChatCMD.')}</p>
          </div>
          <button type="button" className="icon-button" aria-label={tr('Close')} disabled={isBusy} onClick={onClose}>
            <X />
          </button>
        </header>

        {/* Tab switch */}
        {!gitPreview && (
          <div
            style={{
              display: 'flex',
              borderBottom: '1px solid var(--border)',
              padding: '0 20px',
              gap: '16px',
            }}
          >
            <button
              type="button"
              style={{
                padding: '10px 4px',
                border: 0,
                background: 'transparent',
                fontWeight: 600,
                fontSize: '0.85rem',
                color: tab === 'git' ? 'var(--accent)' : 'var(--muted)',
                borderBottom: tab === 'git' ? '2px solid var(--accent)' : '2px solid transparent',
                cursor: 'pointer',
              }}
              onClick={() => setTab('git')}
            >
              <GitFork style={{ width: '14px', height: '14px', display: 'inline', marginRight: '6px' }} />
              {tr('From GitHub repository')}
            </button>
            <button
              type="button"
              style={{
                padding: '10px 4px',
                border: 0,
                background: 'transparent',
                fontWeight: 600,
                fontSize: '0.85rem',
                color: tab === 'local' ? 'var(--accent)' : 'var(--muted)',
                borderBottom: tab === 'local' ? '2px solid var(--accent)' : '2px solid transparent',
                cursor: 'pointer',
              }}
              onClick={() => setTab('local')}
            >
              <Folder style={{ width: '14px', height: '14px', display: 'inline', marginRight: '6px' }} />
              {tr('Link local folder')}
            </button>
          </div>
        )}

        {/* Tab content */}
        {tab === 'git' ? (
          !gitPreview ? (
            <div className="skill-modal-body">
              <label htmlFor="skill-repository">{tr('GitHub repository URL')}</label>
              <input
                id="skill-repository"
                autoFocus
                type="text"
                inputMode="url"
                spellCheck={false}
                value={gitUrl}
                onBlur={() => setGitTouched(true)}
                onChange={(event) => {
                  setGitUrl(event.target.value);
                  setGitError(undefined);
                }}
                placeholder="https://github.com/owner/repository"
                aria-invalid={gitTouched && !!gitUrl && !normalizedGitUrl}
              />
              <small id="repository-hint">{tr('ChatCMD will scan the repository and let you choose which skills to install.')}</small>
              {gitTouched && gitUrl && !normalizedGitUrl && (
                <p className="skill-form-error" role="alert">
                  {tr('Enter an HTTPS github.com repository or /tree/{ref}/{path} URL.')}
                </p>
              )}
              <div className="skill-security">
                <ShieldAlert />
                <span>
                  <strong>{tr('Only install sources you trust.')}</strong> {tr('Skills may contain instructions and scripts that Agents will execute.')}
                </span>
              </div>
              {gitError && <p className="skill-form-error" role="alert">{gitError}</p>}
            </div>
          ) : (
            <div className="skill-modal-body skill-install-body">
              <div className="skill-repository-summary">
                <GitFork />
                <div>
                  <strong>{shortRepository(gitPreview.repositoryUrl)}</strong>
                  <code>{gitPreview.repositoryUrl}</code>
                </div>
                <button
                  type="button"
                  className="button secondary"
                  disabled={isBusy}
                  onClick={() => {
                    setGitPreview(undefined);
                    setSelectedPaths(new Set());
                    setGitError(undefined);
                  }}
                >
                  {tr('Change')}
                </button>
              </div>

              <div className="skill-selection-heading">
                <div>
                  <h3>{tr('{count} skills found', { count: gitPreview.skills.length })}</h3>
                  <p role="status">
                    {tr('{selected} of {available} available selected', {
                      selected: selectedPaths.size,
                      available: availableSkills.length,
                    })}
                  </p>
                </div>
                {availableSkills.length > 0 && (
                  <button
                    type="button"
                    className="skill-selection-toggle"
                    disabled={isBusy}
                    onClick={() =>
                      setSelectedPaths(allSelected ? new Set() : new Set(availableSkills.map((s) => s.path)))
                    }
                  >
                    {allSelected ? tr('Clear all') : tr('Select all')}
                  </button>
                )}
              </div>

              <fieldset className="skill-candidate-list" disabled={isBusy}>
                <legend className="sr-only">{tr('Skills available to install')}</legend>
                {gitPreview.skills.map((s) => {
                  const selected = selectedPaths.has(s.path);
                  return (
                    <label className={`skill-candidate ${s.installed ? 'installed' : ''}`} key={s.path}>
                      <input
                        type="checkbox"
                        checked={selected}
                        disabled={s.installed || isBusy}
                        onChange={(e) => {
                          const next = new Set(selectedPaths);
                          if (e.target.checked) next.add(s.path);
                          else next.delete(s.path);
                          setSelectedPaths(next);
                        }}
                      />
                      <span className="skill-candidate-check" aria-hidden="true">
                        {selected && <Check />}
                      </span>
                      <span className="skill-candidate-copy">
                        <span>
                          <strong>{s.title}</strong>
                          {s.installed && <em>{tr('Installed')}</em>}
                        </span>
                        <small>{s.description}</small>
                        <code>{s.path}</code>
                      </span>
                    </label>
                  );
                })}
              </fieldset>

              {installedCount > 0 && (
                <p className="skill-install-note">
                  {tr('{count} skills are already installed and were left unselected.', { count: installedCount })}
                </p>
              )}
              {gitPreview.skippedInvalid > 0 && (
                <p className="skill-install-note warning">
                  <CircleAlert />
                  {tr('{count} invalid skill folders were skipped.', { count: gitPreview.skippedInvalid })}
                </p>
              )}
              {gitError && <p className="skill-form-error" role="alert">{gitError}</p>}
            </div>
          )
        ) : (
          /* Tab: Local Folder */
          <div className="skill-modal-body">
            <label htmlFor="local-folder-path">{tr('Local folder absolute path')}</label>
            <input
              id="local-folder-path"
              autoFocus
              type="text"
              spellCheck={false}
              value={localPath}
              onChange={(e) => {
                setLocalPath(e.target.value);
                setLocalError(undefined);
              }}
              placeholder="D:\my-skills or /home/user/skills"
            />
            <small>{tr('ChatCMD will read skills in-place without copying files.')}</small>

            <div style={{ marginTop: '14px' }}>
              <label htmlFor="local-folder-scope">{tr('Scope')}</label>
              <input
                id="local-folder-scope"
                type="text"
                value={localScope}
                onChange={(e) => setLocalScope(e.target.value)}
                placeholder="global or project_name"
              />
              <small>{tr('Use "global" for all projects, or enter a specific project name.')}</small>
            </div>

            <div className="skill-security" style={{ marginTop: '16px' }}>
              <Info />
              <span>
                <strong>{tr('Folder validation')}</strong>:{' '}
                {tr('The folder must be absolute, readable, and contain subdirectories with SKILL.md or core.md.')}
              </span>
            </div>
            {localError && <p className="skill-form-error" role="alert">{localError}</p>}
          </div>
        )}

        <footer>
          {tab === 'git' ? (
            gitPreview ? (
              <>
                <button
                  type="button"
                  className="button secondary"
                  disabled={isBusy}
                  onClick={() => {
                    setGitPreview(undefined);
                    setSelectedPaths(new Set());
                  }}
                >
                  <ArrowLeft /> {tr('Back')}
                </button>
                <button className="button primary" disabled={!selectedPaths.size || isBusy}>
                  {busy === 'install' ? <LoaderCircle className="spin" /> : <Plus />}
                  {busy === 'install'
                    ? tr('Installing…')
                    : selectedPaths.size === 1
                    ? tr('Install 1 skill')
                    : tr('Install {count} skills', { count: selectedPaths.size })}
                </button>
              </>
            ) : (
              <>
                <button type="button" className="button secondary" disabled={isBusy} onClick={onClose}>
                  {tr('Cancel')}
                </button>
                <button className="button primary" disabled={!normalizedGitUrl || isBusy}>
                  {busy === 'preview' ? <LoaderCircle className="spin" /> : <Search />}
                  {busy === 'preview' ? tr('Scanning repository…') : tr('Find skills')}
                </button>
              </>
            )
          ) : (
            <>
              <button type="button" className="button secondary" disabled={isBusy} onClick={onClose}>
                {tr('Cancel')}
              </button>
              <button className="button primary" disabled={!localPath.trim() || isBusy}>
                {busy === 'add-local-source' ? <LoaderCircle className="spin" /> : <Plus />}
                {busy === 'add-local-source' ? tr('Validating & Adding…') : tr('Register local folder')}
              </button>
            </>
          )}
        </footer>
      </form>
    </div>
  );
}

function SkillDetailModal({
  skill,
  sources,
  onClose,
}: {
  skill: UserSkill;
  sources: LocalSkillSource[];
  onClose: () => void;
}) {
  const dialogRef = useDialogFocus<HTMLElement>(onClose, false);
  const sourceType = (skill.sourceType || skill.source || 'global').toLowerCase();
  const isLegacy = sourceType === 'workspace' || sourceType === 'user_home';

  // Find if skill directory matches any local source
  const matchedSource = sources.find((s) => s.scope === sourceType || skill.source.startsWith(s.path));

  return (
    <div
      className="skill-modal-backdrop"
      onMouseDown={(e) => {
        if (e.target === e.currentTarget) onClose();
      }}
    >
      <section ref={dialogRef} className="skill-modal" role="dialog" aria-modal="true">
        <header>
          <SkillIcon url={skill.iconUrl} />
          <div>
            <h2>{skill.title}</h2>
            <p>{skill.description || tr('No description provided.')}</p>
          </div>
          <button type="button" className="icon-button" aria-label={tr('Close')} onClick={onClose}>
            <X />
          </button>
        </header>

        <div className="skill-modal-body" style={{ display: 'grid', gap: '14px' }}>
          {/* Metadata */}
          <div
            style={{
              display: 'grid',
              gridTemplateColumns: 'repeat(auto-fit, minmax(140px, 1fr))',
              gap: '10px',
              padding: '12px',
              background: 'var(--surface-3)',
              borderRadius: '8px',
              fontSize: '0.8rem',
            }}
          >
            <div>
              <span style={{ color: 'var(--muted)', display: 'block', fontSize: '0.72rem' }}>{tr('Source Type')}</span>
              <strong style={{ textTransform: 'uppercase' }}>{sourceType}</strong>
            </div>
            <div>
              <span style={{ color: 'var(--muted)', display: 'block', fontSize: '0.72rem' }}>{tr('Status')}</span>
              <strong style={{ color: skill.enabled ? 'var(--accent)' : 'var(--muted)' }}>
                {skill.enabled ? tr('Enabled') : tr('Disabled')}
              </strong>
            </div>
            {skill.overrides && (
              <div>
                <span style={{ color: 'var(--muted)', display: 'block', fontSize: '0.72rem' }}>{tr('Overrides Global')}</span>
                <strong style={{ color: '#c2185b' }}>{skill.overrides}</strong>
              </div>
            )}
          </div>

          {/* 3-Tier Status Overview */}
          <div style={{ padding: '12px', border: '1px solid var(--border)', borderRadius: '8px' }}>
            <h4 style={{ margin: '0 0 10px', fontSize: '0.85rem' }}>{tr('3-Tier Architecture Status')}</h4>
            <div style={{ display: 'grid', gap: '8px', fontSize: '0.8rem' }}>
              <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between' }}>
                <span style={{ display: 'flex', alignItems: 'center', gap: '6px' }}>
                  <Layers style={{ width: '14px', height: '14px', color: '#1976d2' }} />
                  <strong>resident.md</strong>
                </span>
                {isLegacy ? (
                  <span style={{ color: 'var(--muted)', fontSize: '0.75rem' }}>{tr('Ignored (not supported for this source)')}</span>
                ) : skill.hasResident ? (
                  <span style={{ color: 'var(--accent)', fontWeight: 600 }}>
                    {tr('Active')} ({skill.residentChars ?? 0} {tr('chars')})
                  </span>
                ) : (
                  <span style={{ color: 'var(--muted)' }}>{tr('None')}</span>
                )}
              </div>

              <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between' }}>
                <span style={{ display: 'flex', alignItems: 'center', gap: '6px' }}>
                  <BookOpen style={{ width: '14px', height: '14px', color: '#7b1fa2' }} />
                  <strong>core.md</strong>
                </span>
                {skill.hasCore ? (
                  <span style={{ color: 'var(--accent)', fontWeight: 600 }}>{tr('Present')}</span>
                ) : (
                  <span style={{ color: 'var(--bad, #e53935)' }}>{tr('Missing (using fallback)')}</span>
                )}
              </div>

              <div>
                <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between' }}>
                  <span style={{ display: 'flex', alignItems: 'center', gap: '6px' }}>
                    <FileText style={{ width: '14px', height: '14px', color: '#f57c00' }} />
                    <strong>examples/</strong>
                  </span>
                  <span>{skill.examples?.length ? tr('{count} tools', { count: skill.examples.length }) : tr('None')}</span>
                </div>
                {skill.examples && skill.examples.length > 0 && (
                  <div style={{ marginTop: '6px', display: 'flex', flexWrap: 'wrap', gap: '4px' }}>
                    {skill.examples.map((tool) => (
                      <code key={tool} style={{ fontSize: '0.72rem', background: 'var(--surface-3)', padding: '2px 6px', borderRadius: '4px' }}>
                        {tool}.md
                      </code>
                    ))}
                  </div>
                )}
              </div>
            </div>
          </div>

          {/* Warnings and Errors */}
          {((skill.warnings?.length ?? 0) > 0 || (skill.errors?.length ?? 0) > 0) && (
            <div style={{ padding: '12px', background: 'var(--surface-3)', borderRadius: '8px', fontSize: '0.78rem' }}>
              {skill.errors && skill.errors.length > 0 && (
                <div style={{ color: 'var(--bad, #e53935)', marginBottom: '8px' }}>
                  <strong>{tr('Errors:')}</strong>
                  <ul style={{ margin: '4px 0 0', paddingLeft: '16px' }}>
                    {skill.errors.map((e, i) => (
                      <li key={i}>{e}</li>
                    ))}
                  </ul>
                </div>
              )}
              {skill.warnings && skill.warnings.length > 0 && (
                <div style={{ color: 'var(--warning, #fb8c00)' }}>
                  <strong>{tr('Warnings:')}</strong>
                  <ul style={{ margin: '4px 0 0', paddingLeft: '16px' }}>
                    {skill.warnings.map((w, i) => (
                      <li key={i}>{w}</li>
                    ))}
                  </ul>
                </div>
              )}
            </div>
          )}

          {matchedSource && (
            <p style={{ margin: 0, fontSize: '0.75rem', color: 'var(--muted)' }}>
              {tr('Linked to local source:')} <code>{matchedSource.path}</code>
            </p>
          )}
        </div>

        <footer>
          <button type="button" className="button secondary" onClick={onClose}>
            {tr('Close')}
          </button>
        </footer>
      </section>
    </div>
  );
}

function ResidentPreviewModal({ onClose }: { onClose: () => void }) {
  const [projectFolder, setProjectFolder] = useState('');
  const [preview, setPreview] = useState<ResidentPreview>();
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string>();
  const dialogRef = useDialogFocus<HTMLElement>(onClose, loading);

  const fetchPreview = useCallback(async (folder?: string) => {
    setLoading(true);
    setError(undefined);
    try {
      const result = await api.residentPreview(folder?.trim() || undefined);
      setPreview(result);
    } catch (err) {
      setError(message(err, tr('Could not load resident preview.')));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    void fetchPreview();
  }, [fetchPreview]);

  return (
    <div
      className="skill-modal-backdrop"
      onMouseDown={(e) => {
        if (e.target === e.currentTarget && !loading) onClose();
      }}
    >
      <section ref={dialogRef} className="skill-modal" role="dialog" aria-modal="true" style={{ maxWidth: '640px' }}>
        <header>
          <span>
            <Eye />
          </span>
          <div>
            <h2>{tr('Resident Instructions Preview')}</h2>
            <p>{tr('Inspect the exact resident instructions injected into model replies for tasks.')}</p>
          </div>
          <button type="button" className="icon-button" aria-label={tr('Close')} disabled={loading} onClick={onClose}>
            <X />
          </button>
        </header>

        <div className="skill-modal-body" style={{ display: 'grid', gap: '14px' }}>
          <div style={{ display: 'flex', gap: '8px', alignItems: 'flex-end' }}>
            <div style={{ flex: 1 }}>
              <label htmlFor="preview-project-folder" style={{ fontSize: '0.78rem', marginBottom: '4px', display: 'block' }}>
                {tr('Project folder (optional)')}
              </label>
              <input
                id="preview-project-folder"
                type="text"
                placeholder="D:\path\to\project"
                value={projectFolder}
                onChange={(e) => setProjectFolder(e.target.value)}
                style={{ width: '100%' }}
              />
            </div>
            <button
              type="button"
              className="button secondary"
              style={{ height: '36px' }}
              disabled={loading}
              onClick={() => void fetchPreview(projectFolder)}
            >
              {loading ? <LoaderCircle className="spin" /> : <RefreshCw />} {tr('Update')}
            </button>
          </div>

          {error && <p className="skill-form-error" role="alert">{error}</p>}

          {loading ? (
            <div style={{ padding: '24px', textAlign: 'center', color: 'var(--muted)' }}>
              <LoaderCircle className="spin" style={{ margin: '0 auto 8px', width: '24px', height: '24px' }} />
              <p>{tr('Loading resident snapshot…')}</p>
            </div>
          ) : preview ? (
            <>
              <div>
                <span style={{ fontSize: '0.78rem', color: 'var(--muted)' }}>
                  {tr('Contributing skills:')}
                </span>{' '}
                {preview.skillNames.length > 0 ? (
                  preview.skillNames.map((name) => (
                    <code
                      key={name}
                      style={{
                        marginRight: '6px',
                        fontSize: '0.72rem',
                        background: 'var(--surface-3)',
                        padding: '2px 6px',
                        borderRadius: '4px',
                      }}
                    >
                      {name}
                    </code>
                  ))
                ) : (
                  <span style={{ fontSize: '0.78rem', color: 'var(--muted)' }}>{tr('None')}</span>
                )}
              </div>

              <div>
                <label style={{ fontSize: '0.78rem', color: 'var(--muted)', display: 'block', marginBottom: '4px' }}>
                  {tr('Injected Content ({count} characters):', { count: preview.residentInstructions.length })}
                </label>
                <textarea
                  readOnly
                  rows={8}
                  style={{
                    width: '100%',
                    fontFamily: 'monospace',
                    fontSize: '0.8rem',
                    padding: '10px',
                    borderRadius: '8px',
                    background: 'var(--surface-3)',
                    border: '1px solid var(--border)',
                    color: 'var(--text)',
                    resize: 'vertical',
                  }}
                  value={preview.residentInstructions || tr('(No resident instructions are currently active)')}
                />
              </div>

              {preview.warnings.length > 0 && (
                <div style={{ padding: '10px', background: 'var(--surface-3)', borderRadius: '8px', fontSize: '0.75rem', color: 'var(--warning, #fb8c00)' }}>
                  <strong>{tr('Warnings:')}</strong>
                  <ul style={{ margin: '4px 0 0', paddingLeft: '16px' }}>
                    {preview.warnings.map((w, i) => (
                      <li key={i}>{w}</li>
                    ))}
                  </ul>
                </div>
              )}
            </>
          ) : null}
        </div>

        <footer>
          <button type="button" className="button secondary" onClick={onClose}>
            {tr('Close')}
          </button>
        </footer>
      </section>
    </div>
  );
}

function ConfirmRemoveSourceModal({
  source,
  busy,
  onConfirm,
  onClose,
}: {
  source: LocalSkillSource;
  busy: boolean;
  onConfirm: () => void;
  onClose: () => void;
}) {
  const dialogRef = useDialogFocus<HTMLElement>(onClose, busy);
  return (
    <div
      className="skill-modal-backdrop"
      onMouseDown={(event) => {
        if (event.target === event.currentTarget && !busy) onClose();
      }}
    >
      <section ref={dialogRef} className="skill-modal compact" role="alertdialog" aria-modal="true" aria-labelledby="remove-source-title">
        <header>
          <span className="danger">
            <Trash2 />
          </span>
          <div>
            <h2 id="remove-source-title">{tr('Deregister local source?')}</h2>
            <p style={{ margin: '4px 0 0', fontSize: '0.82rem' }}>
              <code>{source.path}</code>
            </p>
          </div>
          <button className="icon-button" aria-label={tr('Close')} disabled={busy} onClick={onClose}>
            <X />
          </button>
        </header>

        <div className="skill-modal-body" style={{ padding: '10px 0' }}>
          <div
            style={{
              padding: '12px',
              background: 'color-mix(in srgb, var(--accent) 10%, var(--surface))',
              border: '1px solid var(--border)',
              borderRadius: '8px',
              fontSize: '0.82rem',
              display: 'flex',
              gap: '10px',
              alignItems: 'center',
            }}
          >
            <Info style={{ width: '18px', height: '18px', flexShrink: 0, color: 'var(--accent)' }} />
            <span>
              <strong>{tr('Notice:')}</strong> {tr('This will only remove the registration in ChatCMD; files on disk will NOT be deleted.')}
            </span>
          </div>
        </div>

        <footer>
          <button autoFocus className="button secondary" disabled={busy} onClick={onClose}>
            {tr('Cancel')}
          </button>
          <button className="button danger" disabled={busy} onClick={onConfirm}>
            {busy ? <LoaderCircle className="spin" /> : <Trash2 />} {tr('Deregister')}
          </button>
        </footer>
      </section>
    </div>
  );
}

function LintResultModal({
  source,
  results,
  onClose,
}: {
  source: LocalSkillSource;
  results: SkillSourceLintResult[];
  onClose: () => void;
}) {
  const dialogRef = useDialogFocus<HTMLElement>(onClose, false);
  const totalDiags = results.reduce((acc, r) => acc + r.diagnostics.length, 0);

  return (
    <div
      className="skill-modal-backdrop"
      onMouseDown={(e) => {
        if (e.target === e.currentTarget) onClose();
      }}
    >
      <section ref={dialogRef} className="skill-modal" role="dialog" aria-modal="true" style={{ maxWidth: '640px' }}>
        <header>
          <span>
            <FileText />
          </span>
          <div>
            <h2>{tr('Lint Diagnostics')}</h2>
            <p>{tr('Diagnostics for {path}', { path: source.path })}</p>
          </div>
          <button type="button" className="icon-button" aria-label={tr('Close')} onClick={onClose}>
            <X />
          </button>
        </header>

        <div className="skill-modal-body" style={{ display: 'grid', gap: '12px', maxHeight: '60vh', overflowY: 'auto' }}>
          {results.length === 0 ? (
            <p style={{ color: 'var(--muted)', textAlign: 'center', padding: '20px' }}>
              {tr('No skills found in this source directory.')}
            </p>
          ) : totalDiags === 0 ? (
            <div style={{ textAlign: 'center', padding: '24px', color: 'var(--accent)' }}>
              <Check style={{ width: '32px', height: '32px', margin: '0 auto 8px' }} />
              <p>
                <strong>{tr('All {count} skills passed lint inspection without diagnostics!', { count: results.length })}</strong>
              </p>
            </div>
          ) : (
            results.map((r) => (
              <div
                key={r.path}
                style={{
                  padding: '10px 12px',
                  background: 'var(--surface-3)',
                  borderRadius: '8px',
                  fontSize: '0.8rem',
                }}
              >
                <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: '6px' }}>
                  <strong>{r.skillName}</strong>
                  <code style={{ fontSize: '0.72rem', color: 'var(--muted)' }}>{r.path}</code>
                </div>

                {r.diagnostics.length === 0 ? (
                  <span style={{ color: 'var(--accent)', fontSize: '0.75rem' }}>{tr('Passed')}</span>
                ) : (
                  <ul style={{ margin: 0, paddingLeft: '18px', display: 'grid', gap: '4px' }}>
                    {r.diagnostics.map((d, i) => (
                      <li key={i} style={{ color: d.severity === 'error' ? 'var(--bad, #e53935)' : 'var(--warning, #fb8c00)' }}>
                        <strong>[{d.severity.toUpperCase()}] {d.code}:</strong> {d.message}
                        {d.path && <small style={{ display: 'block', color: 'var(--muted)' }}>({d.path})</small>}
                      </li>
                    ))}
                  </ul>
                )}
              </div>
            ))
          )}
        </div>

        <footer>
          <button type="button" className="button secondary" onClick={onClose}>
            {tr('Close')}
          </button>
        </footer>
      </section>
    </div>
  );
}

function SkillOptionsModal({
  skill,
  busy,
  onSave,
  onClose,
}: {
  skill: UserSkill;
  busy: boolean;
  onSave: (skill: UserSkill, values: Record<string, SkillOptionValue>) => Promise<void>;
  onClose: () => void;
}) {
  const [values, setValues] = useState<Record<string, SkillOptionValue>>(() =>
    Object.fromEntries(skill.options.map((option) => [option.key, option.value]))
  );
  const [error, setError] = useState<string>();
  const dialogRef = useDialogFocus<HTMLFormElement>(onClose, busy);

  async function submit(event: FormEvent) {
    event.preventDefault();
    setError(undefined);
    try {
      await onSave(skill, values);
    } catch (reason) {
      setError(message(reason, tr('Could not save skill options.')));
    }
  }

  return (
    <div
      className="skill-modal-backdrop"
      onMouseDown={(event) => {
        if (event.target === event.currentTarget && !busy) onClose();
      }}
    >
      <form ref={dialogRef} className="skill-modal" role="dialog" aria-modal="true" aria-labelledby="skill-options-title" onSubmit={(event) => void submit(event)}>
        <header>
          <span>
            <Settings2 />
          </span>
          <div>
            <h2 id="skill-options-title">{tr('Configure: {name}', { name: skill.title })}</h2>
            <p>{tr('Agents use these values when applying the skill.')}</p>
          </div>
          <button type="button" className="icon-button" aria-label={tr('Close')} disabled={busy} onClick={onClose}>
            <X />
          </button>
        </header>
        <div className="skill-modal-body option-fields">
          {skill.options.map((option) => (
            <OptionField
              key={option.key}
              option={option}
              value={values[option.key]}
              onChange={(value) => setValues((current) => ({ ...current, [option.key]: value }))}
            />
          ))}
          {error && <p className="skill-form-error" role="alert">{error}</p>}
        </div>
        <footer>
          <button type="button" className="button secondary" disabled={busy} onClick={onClose}>
            {tr('Cancel')}
          </button>
          <button className="button primary" disabled={busy}>
            {busy ? <LoaderCircle className="spin" /> : <Check />} {tr('Save options')}
          </button>
        </footer>
      </form>
    </div>
  );
}

function OptionField({
  option,
  value,
  onChange,
}: {
  option: UserSkillOption;
  value: SkillOptionValue;
  onChange: (value: SkillOptionValue) => void;
}) {
  if (option.type === 'boolean') {
    return (
      <label className="skill-option-toggle">
        <span>
          <strong>{option.label}</strong>
          {option.description && <small>{option.description}</small>}
        </span>
        <span className="agent-switch">
          <input
            type="checkbox"
            role="switch"
            aria-checked={Boolean(value)}
            checked={Boolean(value)}
            onChange={(event) => onChange(event.target.checked)}
          />
          <i />
        </span>
      </label>
    );
  }
  const id = `skill-option-${option.key}`;
  return (
    <label className="skill-option-field" htmlFor={id}>
      <strong>{option.label}</strong>
      {option.type === 'select' ? (
        <select id={id} value={String(value)} onChange={(event) => onChange(event.target.value)}>
          {(option.choices ?? []).map((choice) => (
            <option key={choice.value} value={choice.value}>
              {choice.label}
            </option>
          ))}
        </select>
      ) : (
        <input
          id={id}
          type={option.type === 'number' ? 'number' : 'text'}
          value={String(value)}
          onChange={(event) => onChange(option.type === 'number' ? Number(event.target.value) : event.target.value)}
        />
      )}
      {option.description && <small>{option.description}</small>}
    </label>
  );
}

function ConfirmDeleteModal({
  skill,
  busy,
  onConfirm,
  onClose,
}: {
  skill: UserSkill;
  busy: boolean;
  onConfirm: () => void;
  onClose: () => void;
}) {
  const dialogRef = useDialogFocus<HTMLElement>(onClose, busy);
  return (
    <div
      className="skill-modal-backdrop"
      onMouseDown={(event) => {
        if (event.target === event.currentTarget && !busy) onClose();
      }}
    >
      <section ref={dialogRef} className="skill-modal compact" role="alertdialog" aria-modal="true" aria-labelledby="delete-skill-title">
        <header>
          <span className="danger">
            <Trash2 />
          </span>
          <div>
            <h2 id="delete-skill-title">{tr('Delete skill?')}</h2>
            <p>{tr('Skill {name} will be removed from this machine.', { name: skill.title })}</p>
          </div>
          <button className="icon-button" aria-label={tr('Close')} disabled={busy} onClick={onClose}>
            <X />
          </button>
        </header>
        <footer>
          <button autoFocus className="button secondary" disabled={busy} onClick={onClose}>
            {tr('Cancel')}
          </button>
          <button className="button danger" disabled={busy} onClick={onConfirm}>
            {busy ? <LoaderCircle className="spin" /> : <Trash2 />} {tr('Delete')}
          </button>
        </footer>
      </section>
    </div>
  );
}

function SkillIcon({ url }: { url?: string | null }) {
  const [failed, setFailed] = useState(false);
  const safeUrl = safeSkillIconUrl(url);
  return (
    <div className="skill-icon">
      {safeUrl && !failed ? <img src={safeUrl} alt="" loading="lazy" onError={() => setFailed(true)} /> : <Blocks />}
    </div>
  );
}

function useDialogFocus<T extends HTMLElement>(onClose: () => void, busy: boolean) {
  const dialogRef = useRef<T>(null);
  const closeRef = useRef(onClose);
  const busyRef = useRef(busy);
  useEffect(() => {
    closeRef.current = onClose;
    busyRef.current = busy;
  }, [onClose, busy]);

  useEffect(() => {
    const previousFocus = document.activeElement instanceof HTMLElement ? document.activeElement : null;
    const dialog = dialogRef.current;
    const frame = window.requestAnimationFrame(() => (dialog?.querySelector<HTMLElement>('[autofocus]') ?? focusableElements(dialog)[0])?.focus());
    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key === 'Escape' && !busyRef.current) {
        event.preventDefault();
        closeRef.current();
        return;
      }
      if (event.key !== 'Tab') return;
      const focusable = focusableElements(dialogRef.current);
      if (!focusable.length) {
        event.preventDefault();
        dialogRef.current?.focus();
        return;
      }
      const first = focusable[0];
      const last = focusable[focusable.length - 1];
      if (event.shiftKey && document.activeElement === first) {
        event.preventDefault();
        last.focus();
      } else if (!event.shiftKey && document.activeElement === last) {
        event.preventDefault();
        first.focus();
      }
    };
    window.addEventListener('keydown', handleKeyDown);
    return () => {
      window.cancelAnimationFrame(frame);
      window.removeEventListener('keydown', handleKeyDown);
      if (previousFocus?.isConnected) previousFocus.focus();
    };
  }, []);
  return dialogRef;
}

function focusableElements(root: HTMLElement | null) {
  return root
    ? Array.from(
        root.querySelectorAll<HTMLElement>(
          'button:not([disabled]),input:not([disabled]),select:not([disabled]),textarea:not([disabled]),a[href],[tabindex]:not([tabindex="-1"])'
        )
      ).filter((element) => !element.hidden && element.getClientRects().length > 0)
    : [];
}

function safeSkillIconUrl(value?: string | null) {
  if (!value) return null;
  if (/^(data:image\/|blob:)/i.test(value)) return value;
  try {
    const url = new URL(value, location.origin);
    return url.origin === location.origin ? url.href : null;
  } catch {
    return null;
  }
}

function normalizeGitHubRepositoryUrl(value: string) {
  const pastedUrl = value.trim().match(/https:\/\/(?:www\.)?github\.com\/[^\s\])]+/i)?.[0] ?? value.trim();
  const candidate = pastedUrl.replace(/[,;\])]+$/, '').replace(/\/$/, '');
  try {
    const url = new URL(candidate);
    if (
      url.protocol !== 'https:' ||
      !['github.com', 'www.github.com'].includes(url.hostname.toLowerCase()) ||
      url.username ||
      url.password ||
      url.port ||
      url.search ||
      url.hash
    )
      return null;
    const parts = url.pathname.split('/').filter(Boolean);
    if (parts.length < 2 || (parts.length > 2 && (parts.length < 4 || parts[2] !== 'tree'))) return null;
    return `https://github.com/${parts.join('/')}`;
  } catch {
    return null;
  }
}

function shortRepository(url: string) {
  try {
    const path = new URL(url).pathname.replace(/^\//, '').replace(/\.git$/, '');
    return path || url;
  } catch {
    return url;
  }
}

function message(reason: unknown, fallback: string) {
  return reason instanceof Error ? reason.message : fallback;
}
