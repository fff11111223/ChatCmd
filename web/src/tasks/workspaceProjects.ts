import type { Task, WorkspaceProject } from '../types';

export interface WorkspaceTaskGroups {
  projects: Array<{ project: WorkspaceProject; tasks: Task[] }>;
  unclassified: Task[];
}

export function canonicalProjectPath(path: string) {
  let clean = path.trim().replace(/\\/g, '/');
  while (clean.length > 1 && clean.endsWith('/')) clean = clean.slice(0, -1);
  return /^[a-z]:/i.test(clean) || clean.startsWith('//') ? clean.toLowerCase() : clean;
}

export function groupTasksByWorkspaceProjects(projects: WorkspaceProject[], tasks: Task[]): WorkspaceTaskGroups {
  const projectTasks = projects.map((project) => ({ project, tasks: [] as Task[] }));
  const byPath = new Map<string, Task[]>();
  for (const entry of projectTasks) {
    const allPaths = entry.project.paths && entry.project.paths.length > 0
      ? entry.project.paths
      : [entry.project.path];
    for (const p of allPaths) {
      if (p && p.trim()) {
        byPath.set(canonicalProjectPath(p), entry.tasks);
      }
    }
  }

  const unclassified: Task[] = [];
  for (const task of tasks) {
    const path = task.projectFolder?.trim();
    const group = path ? byPath.get(canonicalProjectPath(path)) : undefined;
    if (group) group.push(task);
    else unclassified.push(task);
  }

  return {
    projects: projectTasks,
    unclassified,
  };
}
