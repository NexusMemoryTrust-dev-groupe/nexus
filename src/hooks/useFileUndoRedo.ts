import { useState, useCallback, useRef } from 'react';
import { invoke } from '@tauri-apps/api/core';

export type FileAction =
  | { type: 'create'; path: string; isDir: boolean; content?: string }
  | { type: 'delete'; path: string; isDir: boolean; content?: string; children?: unknown }
  | { type: 'rename'; oldPath: string; newPath: string; isDir: boolean }
  | { type: 'move'; sourcePath: string; destPath: string; originalParent: string };

interface UndoRedoState {
  undoStack: FileAction[];
  redoStack: FileAction[];
}

function getParentDir(filePath: string): string {
  const lastSlash = Math.max(filePath.lastIndexOf('\\'), filePath.lastIndexOf('/'));
  return lastSlash > 0 ? filePath.substring(0, lastSlash) : filePath;
}

function getFileName(filePath: string): string {
  const lastSlash = Math.max(filePath.lastIndexOf('\\'), filePath.lastIndexOf('/'));
  return lastSlash >= 0 ? filePath.substring(lastSlash + 1) : filePath;
}

export function useFileUndoRedo(onRefresh: () => void) {
  const [stacks, setStacks] = useState<UndoRedoState>({
    undoStack: [],
    redoStack: [],
  });
  const stacksRef = useRef(stacks);
  stacksRef.current = stacks;

  const pushAction = useCallback((action: FileAction) => {
    setStacks((prev) => ({
      undoStack: [...prev.undoStack, action],
      redoStack: [], // clear redo on new action
    }));
  }, []);

  const undo = useCallback(async () => {
    const { undoStack, redoStack } = stacksRef.current;
    if (undoStack.length === 0) return;

    const action = undoStack[undoStack.length - 1];
    const newUndo = undoStack.slice(0, -1);

    try {
      switch (action.type) {
        case 'create': {
          // Undo create = delete the file/folder
          if (action.isDir) {
            await invoke('delete_folder', { folderPath: action.path });
          } else {
            await invoke('delete_file', { filePath: action.path });
          }
          break;
        }
        case 'delete': {
          // Undo delete = recreate the file/folder
          if (action.isDir) {
            await invoke('create_folder', { folderPath: action.path });
          } else {
            await invoke('create_file', { filePath: action.path, content: action.content || '' });
          }
          break;
        }
        case 'rename': {
          const oldName = getFileName(action.oldPath);
          await invoke('rename_file', { oldPath: action.newPath, newName: oldName });
          break;
        }
        case 'move': {
          await invoke('move_entry', { sourcePath: action.destPath, destDir: action.originalParent });
          break;
        }
      }

      setStacks({
        undoStack: newUndo,
        redoStack: [...redoStack, action],
      });
      onRefresh();
    } catch (e) {
      console.error('Undo failed:', e);
    }
  }, [onRefresh]);

  const redo = useCallback(async () => {
    const { undoStack, redoStack } = stacksRef.current;
    if (redoStack.length === 0) return;

    const action = redoStack[redoStack.length - 1];
    const newRedo = redoStack.slice(0, -1);

    try {
      switch (action.type) {
        case 'create': {
          // Redo create = recreate
          if (action.isDir) {
            await invoke('create_folder', { folderPath: action.path });
          } else {
            await invoke('create_file', { filePath: action.path, content: action.content || '' });
          }
          break;
        }
        case 'delete': {
          // Redo delete = delete again
          if (action.isDir) {
            await invoke('delete_folder', { folderPath: action.path });
          } else {
            await invoke('delete_file', { filePath: action.path });
          }
          break;
        }
        case 'rename': {
          const newName = getFileName(action.newPath);
          await invoke('rename_file', { oldPath: action.oldPath, newName });
          break;
        }
        case 'move': {
          const destDir = getParentDir(action.destPath);
          await invoke('move_entry', { sourcePath: action.sourcePath, destDir });
          break;
        }
      }

      setStacks({
        undoStack: [...undoStack, action],
        redoStack: newRedo,
      });
      onRefresh();
    } catch (e) {
      console.error('Redo failed:', e);
    }
  }, [onRefresh]);

  const clearStacks = useCallback(() => {
    setStacks({ undoStack: [], redoStack: [] });
  }, []);

  return {
    undoStack: stacks.undoStack,
    redoStack: stacks.redoStack,
    pushAction,
    undo,
    redo,
    clearStacks,
    canUndo: stacks.undoStack.length > 0,
    canRedo: stacks.redoStack.length > 0,
  };
}
