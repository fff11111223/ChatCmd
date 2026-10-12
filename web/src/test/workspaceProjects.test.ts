import { describe, expect, it } from 'vitest';

import { canonicalProjectPath, groupTasksByWorkspaceProjects } from '../tasks/workspaceProjects';
import type { Task, WorkspaceProject } from '../types';

describe('workspace projects', () => {
  it('canonicalizes equivalent Windows drive and UNC paths', () => {
    expect(canonicalProjectPath('D:\\DEV\\Dotty\\')).toBe('d:/dev/dotty');
    expect(canonicalProjectPath('d:/dev/dotty')).toBe('d:/dev/dotty');
    expect(canonicalProjectPath('\\\\Server\\Share\\Project\\')).toBe('//server/share/project');
    expect(canonicalProjectPath('C:\\')).toBe('c:');
    expect(canonicalProjectPath('/')).toBe('/');
  });

  it('groups tasks from backend projectFolder and leaves unknown folders unclassified', () => {
    const projects: WorkspaceProject[] = [{ id: 'project-1', name: 'Dotty', path: 'D:\\DEV\\Dotty' }];
    const task = (id: string, projectFolder?: string | null): Task => ({ id, projectFolder, status: 'completed', updatedAtUtc: '2026-01-01T00:00:00Z' });
    const groups = groupTasksByWorkspaceProjects(projects, [
      task('matched', 'd:/dev/dotty/'),
      task('manual', 'D:\\DEV\\Other'),
      task('missing'),
    ]);

    expect(groups.projects[0].tasks.map(({ id }) => id)).toEqual(['matched']);
    expect(groups.unclassified.map(({ id }) => id)).toEqual(['manual', 'missing']);
  });

  it('groups tasks belonging to any folder in a multi-folder project', () => {
    const projects: WorkspaceProject[] = [
      {
        id: 'project-multi',
        name: 'Monorepo',
        path: 'D:\\DEV\\Monorepo\\backend',
        paths: ['D:\\DEV\\Monorepo\\backend', 'D:\\DEV\\Monorepo\\frontend', 'D:\\DEV\\Monorepo\\docs'],
      },
      {
        id: 'project-single',
        name: 'Solo',
        path: 'D:\\DEV\\Solo',
      },
    ];
    const task = (id: string, projectFolder?: string | null): Task => ({ id, projectFolder, status: 'completed', updatedAtUtc: '2026-01-01T00:00:00Z' });
    const groups = groupTasksByWorkspaceProjects(projects, [
      task('t-backend', 'd:/dev/monorepo/backend/'),
      task('t-frontend', 'D:\\DEV\\Monorepo\\frontend'),
      task('t-docs', 'd:/dev/monorepo/docs'),
      task('t-solo', 'D:\\DEV\\Solo'),
      task('t-other', 'D:\\DEV\\Unknown'),
    ]);

    expect(groups.projects[0].tasks.map(({ id }) => id)).toEqual(['t-backend', 't-frontend', 't-docs']);
    expect(groups.projects[1].tasks.map(({ id }) => id)).toEqual(['t-solo']);
    expect(groups.unclassified.map(({ id }) => id)).toEqual(['t-other']);
  });
});
