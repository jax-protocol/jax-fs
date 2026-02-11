import { Component, createSignal, onMount, For, Show, createMemo } from 'solid-js';
import { useParams, useSearchParams, useNavigate } from '@solidjs/router';
import { ls, lsAtVersion, mkdir, deletePath, renamePath, uploadNativeFiles, addFile, publishBucket, isPublished as checkPublished, FileEntry } from '../lib/api';
import { pathToBreadcrumbs } from '../lib/utils';
import Breadcrumb from '../components/Breadcrumb';
import ConfirmDialog from '../components/ConfirmDialog';
import SharePanel from '../components/SharePanel';

const Explorer: Component = () => {
  const params = useParams<{ bucketId: string }>();
  const [searchParams, setSearchParams] = useSearchParams();
  const navigate = useNavigate();

  const currentPath = () => (searchParams.path as string) || '/';
  const versionHash = () => (searchParams.at as string) || null;
  const isHistoryView = createMemo(() => !!versionHash());

  const [entries, setEntries] = createSignal<FileEntry[]>([]);
  const [loading, setLoading] = createSignal(true);
  const [error, setError] = createSignal<string | null>(null);

  // New folder state
  const [showNewFolder, setShowNewFolder] = createSignal(false);
  const [newFolderName, setNewFolderName] = createSignal('');

  // New file state
  const [showNewFile, setShowNewFile] = createSignal(false);
  const [newFileName, setNewFileName] = createSignal('');

  // Rename state
  const [renamingPath, setRenamingPath] = createSignal<string | null>(null);
  const [renameValue, setRenameValue] = createSignal('');

  // Delete confirmation state
  const [deleteTarget, setDeleteTarget] = createSignal<FileEntry | null>(null);

  // Share panel
  const [showSharePanel, setShowSharePanel] = createSignal(false);

  // Publish state
  const [publishing, setPublishing] = createSignal(false);
  const [isPublished, setIsPublished] = createSignal<boolean | null>(null);
  const [showPublishConfirm, setShowPublishConfirm] = createSignal(false);

  const fetchEntries = async () => {
    try {
      setLoading(true);
      setError(null);
      let result: FileEntry[];
      if (versionHash()) {
        result = await lsAtVersion(params.bucketId, versionHash()!, currentPath());
      } else {
        result = await ls(params.bucketId, currentPath());
      }
      // Sort: folders first, then alphabetically
      result.sort((a, b) => {
        if (a.is_dir !== b.is_dir) return a.is_dir ? -1 : 1;
        return a.name.localeCompare(b.name);
      });
      setEntries(result);
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  };

  const fetchPublishedStatus = async () => {
    try {
      setIsPublished(await checkPublished(params.bucketId));
    } catch {
      // Non-critical, ignore
    }
  };

  const refresh = () => {
    fetchEntries();
    fetchPublishedStatus();
  };

  onMount(refresh);

  // Re-fetch when path changes
  const navigateToPath = (path: string) => {
    if (versionHash()) {
      setSearchParams({ path, at: versionHash()! });
    } else {
      setSearchParams({ path });
    }
    setTimeout(refresh, 0);
  };

  const handleEntryClick = (entry: FileEntry) => {
    if (entry.is_dir) {
      navigateToPath(entry.path);
    } else {
      const atParam = versionHash() ? `&at=${encodeURIComponent(versionHash()!)}` : '';
      navigate(`/buckets/${params.bucketId}/view?path=${encodeURIComponent(entry.path)}${atParam}`);
    }
  };

  const handleUpload = async () => {
    try {
      const { open } = await import('@tauri-apps/plugin-dialog');
      const selected = await open({ multiple: true });
      if (!selected) return;

      const paths = Array.isArray(selected) ? selected as string[] : [selected as string];
      if (paths.length === 0) return;

      setError(null);
      await uploadNativeFiles(params.bucketId, currentPath(), paths);
      refresh();
    } catch (e) {
      setError(String(e));
    }
  };

  const handleNewFolder = async () => {
    const name = newFolderName().trim();
    if (!name) return;

    try {
      setError(null);
      const path = currentPath().endsWith('/')
        ? `${currentPath()}${name}`
        : `${currentPath()}/${name}`;
      await mkdir(params.bucketId, path);
      setShowNewFolder(false);
      setNewFolderName('');
      refresh();
    } catch (e) {
      setError(String(e));
    }
  };

  const handleNewFile = async () => {
    const name = newFileName().trim();
    if (!name) return;

    try {
      setError(null);
      const path = currentPath().endsWith('/')
        ? `${currentPath()}${name}`
        : `${currentPath()}/${name}`;
      await addFile(params.bucketId, path, []);
      setShowNewFile(false);
      setNewFileName('');
      refresh();
    } catch (e) {
      setError(String(e));
    }
  };

  const handleRename = async (entry: FileEntry) => {
    const newName = renameValue().trim();
    if (!newName || newName === entry.name) {
      setRenamingPath(null);
      return;
    }

    try {
      setError(null);
      const parent = entry.path.substring(0, entry.path.lastIndexOf('/')) || '/';
      const newPath = parent === '/' ? `/${newName}` : `${parent}/${newName}`;
      await renamePath(params.bucketId, entry.path, newPath);
      setRenamingPath(null);
      refresh();
    } catch (e) {
      setError(String(e));
    }
  };

  const handleDelete = async () => {
    const target = deleteTarget();
    if (!target) return;

    try {
      setError(null);
      await deletePath(params.bucketId, target.path);
      setDeleteTarget(null);
      refresh();
    } catch (e) {
      setError(String(e));
    }
  };

  const goToHead = () => {
    navigate(`/buckets/${params.bucketId}?path=${encodeURIComponent(currentPath())}`);
    setTimeout(refresh, 0);
  };

  const handlePublish = async () => {
    try {
      setPublishing(true);
      setShowPublishConfirm(false);
      setError(null);
      await publishBucket(params.bucketId);
      fetchPublishedStatus();
    } catch (e) {
      setError(String(e));
    } finally {
      setPublishing(false);
    }
  };

  const breadcrumbs = () => pathToBreadcrumbs(currentPath());

  return (
    <div>
      {/* History version banner */}
      <Show when={isHistoryView()}>
        <div style={{
          background: 'hsl(217 91% 60% / 0.08)',
          border: '1px solid hsl(217 91% 60% / 0.3)',
          padding: '0.625rem 1rem',
          'border-radius': '8px',
          'margin-bottom': '1rem',
          display: 'flex',
          'justify-content': 'space-between',
          'align-items': 'center',
        }}>
          <span style={{ color: 'var(--accent-blue)', 'font-size': '0.875rem' }}>
            Viewing historical version: <code style={{ 'font-size': '0.75rem' }}>{versionHash()!.substring(0, 16)}...</code>
          </span>
          <button
            onClick={goToHead}
            style={{
              padding: '0.375rem 0.625rem',
              'border-radius': '6px',
              border: '1px solid var(--accent-blue)',
              background: 'transparent',
              color: 'var(--accent-blue)',
              cursor: 'pointer',
              'font-size': '0.75rem',
              'font-family': 'inherit',
              'font-weight': '500',
            }}
          >
            Back to HEAD
          </button>
        </div>
      </Show>

      {/* Header */}
      <div style={{
        display: 'flex',
        'justify-content': 'space-between',
        'align-items': 'center',
        'margin-bottom': '1rem',
      }}>
        <div>
          <h2 style={{ 'font-size': '1.5rem', 'font-weight': '700', 'margin-bottom': '0.5rem' }}>
            Files
          </h2>
          <Breadcrumb items={breadcrumbs()} onNavigate={navigateToPath} />
        </div>
        <div style={{ display: 'flex', 'align-items': 'center', gap: '0.5rem' }}>
          <Show when={isPublished()}>
            <span style={{
              'font-size': '0.6875rem',
              'font-weight': '600',
              padding: '0.125rem 0.5rem',
              'border-radius': '9999px',
              background: 'hsl(142 76% 36% / 0.12)',
              color: 'var(--accent-green)',
            }}>
              Published
            </span>
          </Show>
          <span style={{ 'font-size': '0.75rem', color: 'var(--muted-fg)', 'font-family': 'monospace' }}>
            {params.bucketId.substring(0, 8)}...
          </span>
        </div>
      </div>

      {/* Action bar */}
      <div style={{
        display: 'flex',
        gap: '0.5rem',
        'margin-bottom': '1rem',
      }}>
        <Show when={!isHistoryView()}>
          <button
            onClick={handleUpload}
            style={{
              padding: '0.5rem 0.75rem',
              'border-radius': '8px',
              border: '1px solid var(--border)',
              background: 'var(--fg)',
              color: 'var(--bg)',
              cursor: 'pointer',
              'font-size': '0.8125rem',
              'font-weight': '500',
              'font-family': 'inherit',
            }}
          >
            Upload Files
          </button>
          <button
            onClick={() => { setShowNewFolder(true); setNewFolderName(''); }}
            style={{
              padding: '0.5rem 0.75rem',
              'border-radius': '8px',
              border: '1px solid var(--border)',
              background: 'var(--muted)',
              color: 'var(--fg)',
              cursor: 'pointer',
              'font-size': '0.8125rem',
              'font-weight': '500',
              'font-family': 'inherit',
            }}
          >
            New Folder
          </button>
          <button
            onClick={() => { setShowNewFile(true); setNewFileName(''); }}
            style={{
              padding: '0.5rem 0.75rem',
              'border-radius': '8px',
              border: '1px solid var(--border)',
              background: 'var(--muted)',
              color: 'var(--fg)',
              cursor: 'pointer',
              'font-size': '0.8125rem',
              'font-weight': '500',
              'font-family': 'inherit',
            }}
          >
            New File
          </button>
        </Show>
        <button
          onClick={() => navigate(`/buckets/${params.bucketId}/history`)}
          style={{
            padding: '0.5rem 0.75rem',
            'border-radius': '8px',
            border: '1px solid var(--border)',
            background: 'var(--muted)',
            color: 'var(--fg)',
            cursor: 'pointer',
            'font-size': '0.8125rem',
            'font-weight': '500',
            'font-family': 'inherit',
            'margin-left': isHistoryView() ? '0' : 'auto',
          }}
        >
          History
        </button>
        <Show when={!isHistoryView()}>
          <button
            onClick={() => setShowSharePanel(!showSharePanel())}
            style={{
              padding: '0.5rem 0.75rem',
              'border-radius': '8px',
              border: '1px solid var(--border)',
              background: showSharePanel() ? 'var(--fg)' : 'var(--muted)',
              color: showSharePanel() ? 'var(--bg)' : 'var(--fg)',
              cursor: 'pointer',
              'font-size': '0.8125rem',
              'font-weight': '500',
              'font-family': 'inherit',
            }}
          >
            Share
          </button>
          <Show when={!isPublished()}>
            <button
              onClick={() => setShowPublishConfirm(true)}
              disabled={publishing()}
              style={{
                padding: '0.5rem 0.75rem',
                'border-radius': '8px',
                border: '1px solid var(--accent-green)',
                background: publishing() ? 'var(--muted)' : 'var(--accent-green)',
                color: publishing() ? 'var(--muted-fg)' : 'white',
                cursor: publishing() ? 'not-allowed' : 'pointer',
                'font-size': '0.8125rem',
                'font-weight': '500',
                'font-family': 'inherit',
              }}
            >
              {publishing() ? 'Publishing...' : 'Publish'}
            </button>
          </Show>
        </Show>
      </div>

      {/* New folder inline input */}
      <Show when={showNewFolder() && !isHistoryView()}>
        <div style={{
          display: 'flex',
          gap: '0.5rem',
          'margin-bottom': '1rem',
          'align-items': 'center',
        }}>
          <input
            type="text"
            placeholder="Folder name..."
            value={newFolderName()}
            onInput={(e) => setNewFolderName(e.currentTarget.value)}
            onKeyPress={(e) => {
              if (e.key === 'Enter') handleNewFolder();
              if (e.key === 'Escape') setShowNewFolder(false);
            }}
            autofocus
            style={{
              padding: '0.375rem 0.625rem',
              'border-radius': '6px',
              border: '1px solid var(--border)',
              background: 'var(--bg)',
              color: 'var(--fg)',
              'font-size': '0.8125rem',
              'font-family': 'inherit',
              outline: 'none',
              width: '200px',
            }}
          />
          <button
            onClick={handleNewFolder}
            disabled={!newFolderName().trim()}
            style={{
              padding: '0.375rem 0.625rem',
              'border-radius': '6px',
              border: '1px solid var(--border)',
              background: 'var(--fg)',
              color: 'var(--bg)',
              cursor: 'pointer',
              'font-size': '0.8125rem',
              'font-family': 'inherit',
              opacity: !newFolderName().trim() ? '0.4' : '1',
            }}
          >
            Create
          </button>
          <button
            onClick={() => setShowNewFolder(false)}
            style={{
              padding: '0.375rem 0.625rem',
              'border-radius': '6px',
              border: '1px solid var(--border)',
              background: 'var(--muted)',
              color: 'var(--fg)',
              cursor: 'pointer',
              'font-size': '0.8125rem',
              'font-family': 'inherit',
            }}
          >
            Cancel
          </button>
        </div>
      </Show>

      {/* New file inline input */}
      <Show when={showNewFile() && !isHistoryView()}>
        <div style={{
          display: 'flex',
          gap: '0.5rem',
          'margin-bottom': '1rem',
          'align-items': 'center',
        }}>
          <input
            type="text"
            placeholder="File name..."
            value={newFileName()}
            onInput={(e) => setNewFileName(e.currentTarget.value)}
            onKeyPress={(e) => {
              if (e.key === 'Enter') handleNewFile();
              if (e.key === 'Escape') setShowNewFile(false);
            }}
            autofocus
            style={{
              padding: '0.375rem 0.625rem',
              'border-radius': '6px',
              border: '1px solid var(--border)',
              background: 'var(--bg)',
              color: 'var(--fg)',
              'font-size': '0.8125rem',
              'font-family': 'inherit',
              outline: 'none',
              width: '200px',
            }}
          />
          <button
            onClick={handleNewFile}
            disabled={!newFileName().trim()}
            style={{
              padding: '0.375rem 0.625rem',
              'border-radius': '6px',
              border: '1px solid var(--border)',
              background: 'var(--fg)',
              color: 'var(--bg)',
              cursor: 'pointer',
              'font-size': '0.8125rem',
              'font-family': 'inherit',
              opacity: !newFileName().trim() ? '0.4' : '1',
            }}
          >
            Create
          </button>
          <button
            onClick={() => setShowNewFile(false)}
            style={{
              padding: '0.375rem 0.625rem',
              'border-radius': '6px',
              border: '1px solid var(--border)',
              background: 'var(--muted)',
              color: 'var(--fg)',
              cursor: 'pointer',
              'font-size': '0.8125rem',
              'font-family': 'inherit',
            }}
          >
            Cancel
          </button>
        </div>
      </Show>

      {/* Error display */}
      <Show when={error()}>
        <div style={{
          background: 'hsl(0 84% 60% / 0.08)',
          border: '1px solid hsl(0 84% 60% / 0.3)',
          padding: '0.75rem 1rem',
          'border-radius': '8px',
          'margin-bottom': '1rem',
          color: 'var(--accent-red)',
          'font-size': '0.875rem',
        }}>
          {error()}
        </div>
      </Show>

      {/* Loading */}
      <Show when={loading()}>
        <p style={{ color: 'var(--muted-fg)', 'font-size': '0.875rem' }}>Loading...</p>
      </Show>

      {/* Empty state */}
      <Show when={!loading() && entries().length === 0 && !error()}>
        <div style={{
          background: 'var(--muted)',
          border: '1px solid var(--border)',
          'border-radius': 'var(--radius)',
          padding: '3rem',
          'text-align': 'center',
        }}>
          <p style={{ 'font-size': '0.9375rem', 'font-weight': '500', 'margin-bottom': '0.375rem' }}>Empty directory</p>
          <p style={{ color: 'var(--muted-fg)', 'font-size': '0.875rem' }}>
            {isHistoryView() ? 'No files at this version' : 'Upload files or create a folder to get started'}
          </p>
        </div>
      </Show>

      {/* File table */}
      <Show when={!loading() && entries().length > 0}>
        <div style={{
          border: '1px solid var(--border)',
          'border-radius': 'var(--radius)',
          overflow: 'hidden',
        }}>
          {/* Table header */}
          <div style={{
            display: 'grid',
            'grid-template-columns': '1fr 120px 160px',
            padding: '0.625rem 1rem',
            background: 'var(--muted)',
            'border-bottom': '1px solid var(--border)',
            'font-size': '0.75rem',
            'font-weight': '600',
            'text-transform': 'uppercase',
            'letter-spacing': '0.05em',
            color: 'var(--muted-fg)',
          }}>
            <span>Name</span>
            <span>Type</span>
            <span style={{ 'text-align': 'right' }}>Actions</span>
          </div>

          {/* Table rows */}
          <For each={entries()}>
            {(entry) => (
              <div style={{
                display: 'grid',
                'grid-template-columns': '1fr 120px 160px',
                padding: '0.625rem 1rem',
                'border-bottom': '1px solid var(--border)',
                'align-items': 'center',
                transition: 'background 0.1s ease',
              }}>
                {/* Name column */}
                <div style={{ display: 'flex', 'align-items': 'center', gap: '0.5rem', 'min-width': '0' }}>
                  <span style={{ 'flex-shrink': '0', display: 'inline-flex', 'align-items': 'center' }}>
                    {entry.is_dir ? (
                      <svg width="16" height="16" viewBox="0 0 16 16" fill="none" style={{ display: 'block' }}>
                        <path d="M1.5 3C1.5 2.44772 1.94772 2 2.5 2H6.29289C6.4255 2 6.55268 2.05268 6.64645 2.14645L7.85355 3.35355C7.94732 3.44732 8.0745 3.5 8.20711 3.5H13.5C14.0523 3.5 14.5 3.94772 14.5 4.5V13C14.5 13.5523 14.0523 14 13.5 14H2.5C1.94772 14 1.5 13.5523 1.5 13V3Z" fill="var(--muted-fg)" opacity="0.5" stroke="var(--muted-fg)" stroke-width="1"/>
                      </svg>
                    ) : (
                      <svg width="16" height="16" viewBox="0 0 16 16" fill="none" style={{ display: 'block' }}>
                        <path d="M4 1.5H9.58579C9.71839 1.5 9.84557 1.55268 9.93934 1.64645L12.8536 4.56066C12.9473 4.65443 13 4.78161 13 4.91421V14C13 14.2761 12.7761 14.5 12.5 14.5H4C3.72386 14.5 3.5 14.2761 3.5 14V2C3.5 1.72386 3.72386 1.5 4 1.5Z" stroke="var(--muted-fg)" stroke-width="1"/>
                        <path d="M9.5 1.5V4.5H12.5" stroke="var(--muted-fg)" stroke-width="1" fill="none"/>
                      </svg>
                    )}
                  </span>
                  <Show when={renamingPath() === entry.path && !isHistoryView()} fallback={
                    <button
                      onClick={() => handleEntryClick(entry)}
                      style={{
                        background: 'none',
                        border: 'none',
                        color: 'var(--fg)',
                        cursor: 'pointer',
                        'font-size': '0.875rem',
                        'font-family': 'inherit',
                        'text-align': 'left',
                        padding: '0',
                        overflow: 'hidden',
                        'text-overflow': 'ellipsis',
                        'white-space': 'nowrap',
                      }}
                    >
                      {entry.name}
                    </button>
                  }>
                    <input
                      type="text"
                      value={renameValue()}
                      onInput={(e) => setRenameValue(e.currentTarget.value)}
                      onKeyPress={(e) => {
                        if (e.key === 'Enter') handleRename(entry);
                        if (e.key === 'Escape') setRenamingPath(null);
                      }}
                      onBlur={() => handleRename(entry)}
                      autofocus
                      style={{
                        padding: '0.125rem 0.375rem',
                        'border-radius': '4px',
                        border: '1px solid var(--accent-blue)',
                        background: 'var(--bg)',
                        color: 'var(--fg)',
                        'font-size': '0.875rem',
                        'font-family': 'inherit',
                        outline: 'none',
                        width: '100%',
                      }}
                    />
                  </Show>
                </div>

                {/* Type badge */}
                <div>
                  <span style={{
                    'font-size': '0.6875rem',
                    'font-weight': '500',
                    padding: '0.125rem 0.5rem',
                    'border-radius': '9999px',
                    background: entry.is_dir ? 'hsl(217 91% 60% / 0.12)' : 'var(--muted)',
                    color: entry.is_dir ? 'var(--accent-blue)' : 'var(--muted-fg)',
                  }}>
                    {entry.is_dir ? 'Folder' : entry.mime_type.split('/').pop()}
                  </span>
                </div>

                {/* Actions */}
                <div style={{ display: 'flex', gap: '0.25rem', 'justify-content': 'flex-end' }}>
                  <Show when={!entry.is_dir}>
                    <button
                      onClick={() => handleEntryClick(entry)}
                      style={actionBtnStyle()}
                    >
                      View
                    </button>
                  </Show>
                  <Show when={!isHistoryView()}>
                    <Show when={!entry.is_dir}>
                      <button
                        onClick={() => navigate(`/buckets/${params.bucketId}/edit?path=${encodeURIComponent(entry.path)}`)}
                        style={actionBtnStyle()}
                      >
                        Edit
                      </button>
                    </Show>
                    <button
                      onClick={() => { setRenamingPath(entry.path); setRenameValue(entry.name); }}
                      style={actionBtnStyle()}
                    >
                      Rename
                    </button>
                    <button
                      onClick={() => setDeleteTarget(entry)}
                      style={{
                        ...actionBtnStyle(),
                        color: 'var(--accent-red)',
                      }}
                    >
                      Delete
                    </button>
                  </Show>
                </div>
              </div>
            )}
          </For>
        </div>
      </Show>

      {/* Delete confirmation dialog */}
      <ConfirmDialog
        open={!!deleteTarget()}
        title={`Delete ${deleteTarget()?.is_dir ? 'folder' : 'file'}`}
        message={`Are you sure you want to delete "${deleteTarget()?.name}"? This action cannot be undone.`}
        onConfirm={handleDelete}
        onCancel={() => setDeleteTarget(null)}
      />

      {/* Publish confirmation dialog */}
      <ConfirmDialog
        open={showPublishConfirm()}
        title="Publish bucket"
        message="Publishing makes this version available to all shared peers. Mirrors will be able to decrypt and read the contents. Continue?"
        confirmLabel="Publish"
        confirmColor="var(--accent-green)"
        onConfirm={handlePublish}
        onCancel={() => setShowPublishConfirm(false)}
      />

      {/* Share panel */}
      <SharePanel
        bucketId={params.bucketId}
        open={showSharePanel()}
        onClose={() => setShowSharePanel(false)}
      />
    </div>
  );
};

function actionBtnStyle(): Record<string, string> {
  return {
    background: 'none',
    border: 'none',
    color: 'var(--muted-fg)',
    cursor: 'pointer',
    'font-size': '0.75rem',
    'font-family': 'inherit',
    padding: '0.25rem 0.375rem',
    'border-radius': '4px',
  };
}

export default Explorer;
