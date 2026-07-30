import { create } from 'zustand';
import { invoke } from '@tauri-apps/api/core';
import type { GraphNode, GraphEdge, Memory } from '../types';

interface ProjectState {
  projects: GraphNode[];
  selectedProject: GraphNode | null;
  projectEntities: GraphNode[];
  projectEdges: GraphEdge[];
  projectMemories: Memory[];
  isLoading: boolean;
  error: string | null;

  fetchProjects: () => Promise<void>;
  selectProject: (project: GraphNode | null) => void;
  loadProjectData: (projectId: string) => Promise<void>;
  createProject: (title: string, description: string) => Promise<GraphNode>;
  updateProject: (id: string, title?: string, description?: string) => Promise<GraphNode>;
  updateEntityMetadata: (id: string, metadata: Record<string, unknown>) => Promise<GraphNode>;
  getEntityMetadata: (id: string) => Promise<Record<string, unknown>>;
  deleteProject: (id: string) => Promise<void>;
  createProjectMemory: (projectId: string, title: string, content: string) => Promise<Memory>;
  linkEntityToProject: (projectId: string, entityId: string) => Promise<GraphEdge>;
  deleteRelationship: (relationshipId: string) => Promise<void>;
  updateMemory: (id: string, title?: string, content?: string, summary?: string) => Promise<Memory>;
  deleteMemory: (id: string) => Promise<void>;
}

export const useProjectStore = create<ProjectState>((set, get) => ({
  projects: [],
  selectedProject: null,
  projectEntities: [],
  projectEdges: [],
  projectMemories: [],
  isLoading: false,
  error: null,

  fetchProjects: async () => {
    set({ isLoading: true, error: null });
    try {
      const projects = await invoke<GraphNode[]>('get_projects');
      set({ projects, isLoading: false });
    } catch (error) {
      set({ error: String(error), isLoading: false });
    }
  },

  selectProject: (project) => {
    set({ selectedProject: project });
    if (project) {
      get().loadProjectData(project.id);
    }
  },

  loadProjectData: async (projectId: string) => {
    set({ isLoading: true, error: null });
    try {
      const [graphData, memories] = await Promise.all([
        invoke<{ nodes: GraphNode[]; edges: GraphEdge[] }>('get_project_entities', { projectId }),
        invoke<Memory[]>('get_project_memories', { projectId }),
      ]);
      set({
        projectEntities: graphData.nodes,
        projectEdges: graphData.edges,
        projectMemories: memories,
        isLoading: false,
      });
    } catch (error) {
      set({ error: String(error), isLoading: false });
    }
  },

  createProject: async (title, description) => {
    const project = await invoke<GraphNode>('create_entity', {
      entityType: 'Project',
      title,
      description,
    });
    set((state) => ({ projects: [...state.projects, project] }));
    return project;
  },

  updateProject: async (id, title, description) => {
    const project = await invoke<GraphNode>('update_entity', { id, title, description });
    set((state) => ({
      projects: state.projects.map((p) => (p.id === id ? project : p)),
      selectedProject: state.selectedProject?.id === id ? project : state.selectedProject,
    }));
    return project;
  },

  updateEntityMetadata: async (id, metadata) => {
    const project = await invoke<GraphNode>('update_entity', { id, metadata });
    set((state) => ({
      projects: state.projects.map((p) => (p.id === id ? project : p)),
      selectedProject: state.selectedProject?.id === id ? project : state.selectedProject,
    }));
    return project;
  },

  getEntityMetadata: async (id) => {
    return await invoke<Record<string, unknown>>('get_entity_metadata', { id });
  },

  deleteProject: async (id) => {
    // Clean up workspace entries first (delete_entity doesn't cascade)
    await invoke('delete_workspace_for_project', { projectId: id }).catch(() => {});
    await invoke('delete_entity', { id });
    set((state) => ({
      projects: state.projects.filter((p) => p.id !== id),
      selectedProject: state.selectedProject?.id === id ? null : state.selectedProject,
    }));
  },

  createProjectMemory: async (projectId, title, content) => {
    const memory = await invoke<Memory>('create_project_memory', {
      projectId,
      title,
      content,
    });
    set((state) => ({ projectMemories: [...state.projectMemories, memory] }));
    return memory;
  },

  linkEntityToProject: async (projectId, entityId) => {
    const edge = await invoke<GraphEdge>('link_entity_to_project', {
      projectId,
      entityId,
    });
    // Reload project entities after linking
    get().loadProjectData(projectId);
    return edge;
  },

  deleteRelationship: async (relationshipId) => {
    await invoke('delete_relationship', { relationshipId });
    const { selectedProject } = get();
    if (selectedProject) {
      get().loadProjectData(selectedProject.id);
    }
  },

  updateMemory: async (id, title, content, summary) => {
    const memory = await invoke<Memory>('update_memory', { id, title, content, summary });
    set((state) => ({
      projectMemories: state.projectMemories.map((m) => (m.id === id ? memory : m)),
    }));
    return memory;
  },

  deleteMemory: async (id) => {
    await invoke('delete_memory', { id });
    set((state) => ({
      projectMemories: state.projectMemories.filter((m) => m.id !== id),
    }));
  },
}));
