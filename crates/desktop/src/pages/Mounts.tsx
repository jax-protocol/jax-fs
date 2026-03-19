import { Component, createSignal, onMount, For, Show } from 'solid-js';
import { open } from '@tauri-apps/plugin-dialog';
import {
  listMounts,
  createMount,
  deleteMount,
  startMount,
  stopMount,
  isFuseAvailable,
  listBuckets,
  MountInfo,
  BucketInfo,
} from '../lib/api';
import ConfirmDialog from '../components/ConfirmDialog';

const Mounts: Component = () => {
  const [mounts, setMounts] = createSignal<MountInfo[]>([]);
  const [buckets, setBuckets] = createSignal<BucketInfo[]>([]);
  const [loading, setLoading] = createSignal(true);
  const [error, setError] = createSignal<string | null>(null);
  const [fuseAvailable, setFuseAvailable] = createSignal(false);

  // Delete confirmation
  const [deleteTarget, setDeleteTarget] = createSignal<string | null>(null);

  // Add mount dialog
  const [showAddDialog, setShowAddDialog] = createSignal(false);
  const [selectedBucket, setSelectedBucket] = createSignal('');
  const [mountPath, setMountPath] = createSignal('');
  const [autoMount, setAutoMount] = createSignal(false);
  const [readOnly, setReadOnly] = createSignal(false);
  const [addLoading, setAddLoading] = createSignal(false);

  const fetchMounts = async () => {
    try {
      setLoading(true);
      setError(null);
      const result = await listMounts();
      setMounts(result);
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  };

  const fetchBuckets = async () => {
    try {
      const result = await listBuckets();
      setBuckets(result);
    } catch (_e) {
      // Ignore errors
    }
  };

  onMount(async () => {
    const available = await isFuseAvailable();
    setFuseAvailable(available);
    if (available) {
      await Promise.all([fetchMounts(), fetchBuckets()]);
    } else {
      setLoading(false);
    }
  });

  const handleAddMount = async () => {
    if (!selectedBucket() || !mountPath()) {
      setError('Please select a bucket and mount path');
      return;
    }

    try {
      setAddLoading(true);
      setError(null);
      await createMount({
        bucket_id: selectedBucket(),
        mount_point: mountPath(),
        auto_mount: autoMount(),
        read_only: readOnly(),
      });
      setShowAddDialog(false);
      setSelectedBucket('');
      setMountPath('');
      setAutoMount(false);
      setReadOnly(false);
      await fetchMounts();
    } catch (e) {
      setError(String(e));
    } finally {
      setAddLoading(false);
    }
  };

  const handleDelete = async () => {
    const mountId = deleteTarget();
    if (!mountId) return;
    setDeleteTarget(null);
    try {
      setError(null);
      await deleteMount(mountId);
      await fetchMounts();
    } catch (e) {
      setError(String(e));
    }
  };

  const handleToggle = async (mount: MountInfo) => {
    try {
      setError(null);
      if (mount.status === 'running') {
        await stopMount(mount.mount_id);
      } else {
        await startMount(mount.mount_id);
      }
      await fetchMounts();
    } catch (e) {
      setError(String(e));
    }
  };

  const selectDirectory = async () => {
    try {
      const selected = await open({
        directory: true,
        multiple: false,
        title: 'Select Mount Point',
      });
      if (selected) {
        setMountPath(selected as string);
      }
    } catch (_e) {
      // Cancelled
    }
  };

  const statusColor = (status: string) => {
    switch (status) {
      case 'running':
        return 'var(--accent-green)';
      case 'starting':
      case 'stopping':
        return 'var(--accent-yellow)';
      case 'error':
        return 'var(--accent-red)';
      default:
        return 'var(--muted-fg)';
    }
  };

  return (
    <div style={{ 'max-width': '800px' }}>
      <div style={{
        display: 'flex',
        'justify-content': 'space-between',
        'align-items': 'center',
        'margin-bottom': '1.5rem',
      }}>
        <h2 style={{ 'font-size': '1.5rem', 'font-weight': '700' }}>
          FUSE Mounts
        </h2>
        <Show when={fuseAvailable()}>
          <button
            onClick={() => setShowAddDialog(true)}
            style={primaryButtonStyle()}
          >
            Add Mount
          </button>
        </Show>
      </div>

      <Show when={error()}>
        <div style={errorStyle()}>
          {error()}
        </div>
      </Show>

      <Show when={!fuseAvailable()}>
        <div style={cardStyle()}>
          <p style={{ color: 'var(--muted-fg)', 'font-size': '0.875rem' }}>
            FUSE support is not available. The daemon was built without the <code>fuse</code> feature.
          </p>
        </div>
      </Show>

      <Show when={fuseAvailable()}>
        <Show when={loading()}>
          <p style={{ color: 'var(--muted-fg)', 'font-size': '0.875rem' }}>Loading...</p>
        </Show>

        <Show when={!loading() && mounts().length === 0}>
          <div style={cardStyle()}>
            <p style={{ color: 'var(--muted-fg)', 'font-size': '0.875rem', 'text-align': 'center' }}>
              No mounts configured. Click "Add Mount" to create one.
            </p>
          </div>
        </Show>

        <Show when={!loading() && mounts().length > 0}>
          <div style={{ display: 'flex', 'flex-direction': 'column', gap: '0.75rem' }}>
            <For each={mounts()}>
              {(mount) => (
                <div style={cardStyle()}>
                  <div style={{
                    display: 'flex',
                    'justify-content': 'space-between',
                    'align-items': 'flex-start',
                  }}>
                    <div style={{ flex: '1' }}>
                      <div style={{
                        display: 'flex',
                        'align-items': 'center',
                        gap: '0.5rem',
                        'margin-bottom': '0.25rem',
                      }}>
                        <span style={{
                          width: '8px',
                          height: '8px',
                          'border-radius': '50%',
                          background: statusColor(mount.status),
                        }} />
                        <span style={{ 'font-weight': '500', 'font-size': '0.875rem' }}>
                          {mount.mount_point}
                        </span>
                        <Show when={mount.read_only}>
                          <span style={tagStyle()}>Read-only</span>
                        </Show>
                        <Show when={mount.auto_mount}>
                          <span style={tagStyle()}>Auto-mount</span>
                        </Show>
                      </div>
                      <div style={{
                        'font-size': '0.75rem',
                        color: 'var(--muted-fg)',
                        'font-family': 'monospace',
                      }}>
                        Bucket: {mount.bucket_id}
                      </div>
                      <Show when={mount.error_message}>
                        <div style={{
                          'font-size': '0.75rem',
                          color: 'var(--accent-red)',
                          'margin-top': '0.25rem',
                        }}>
                          Error: {mount.error_message}
                        </div>
                      </Show>
                    </div>
                    <div style={{ display: 'flex', gap: '0.5rem' }}>
                      <button
                        onClick={() => handleToggle(mount)}
                        style={smallButtonStyle()}
                      >
                        {mount.status === 'running' ? 'Stop' : 'Start'}
                      </button>
                      <button
                        onClick={() => setDeleteTarget(mount.mount_id)}
                        style={smallDangerButtonStyle()}
                      >
                        Delete
                      </button>
                    </div>
                  </div>
                </div>
              )}
            </For>
          </div>
        </Show>
      </Show>

      <ConfirmDialog
        open={!!deleteTarget()}
        title="Delete mount"
        message="Are you sure you want to delete this mount?"
        onConfirm={handleDelete}
        onCancel={() => setDeleteTarget(null)}
      />

      {/* Add Mount Dialog */}
      <Show when={showAddDialog()}>
        <div style={overlayStyle()}>
          <div style={dialogStyle()}>
            <h3 style={{ 'font-size': '1rem', 'font-weight': '600', 'margin-bottom': '1rem' }}>
              Add Mount
            </h3>

            <div style={{ 'margin-bottom': '1rem' }}>
              <label style={labelStyle()}>Bucket</label>
              <select
                value={selectedBucket()}
                onInput={(e) => setSelectedBucket(e.target.value)}
                style={inputStyle()}
              >
                <option value="">Select a bucket...</option>
                <For each={buckets()}>
                  {(bucket) => (
                    <option value={bucket.bucket_id}>{bucket.name}</option>
                  )}
                </For>
              </select>
            </div>

            <div style={{ 'margin-bottom': '1rem' }}>
              <label style={labelStyle()}>Mount Point</label>
              <div style={{ display: 'flex', gap: '0.5rem' }}>
                <input
                  type="text"
                  value={mountPath()}
                  onInput={(e) => setMountPath(e.target.value)}
                  placeholder="/path/to/mount"
                  style={{ ...inputStyle(), flex: '1' }}
                />
                <button onClick={selectDirectory} style={smallButtonStyle()}>
                  Browse
                </button>
              </div>
            </div>

            <div style={{ 'margin-bottom': '1rem' }}>
              <label style={{ display: 'flex', 'align-items': 'center', gap: '0.5rem', cursor: 'pointer' }}>
                <input
                  type="checkbox"
                  checked={autoMount()}
                  onChange={(e) => setAutoMount(e.target.checked)}
                />
                <span style={{ 'font-size': '0.875rem' }}>Auto-mount on startup</span>
              </label>
            </div>

            <div style={{ 'margin-bottom': '1.5rem' }}>
              <label style={{ display: 'flex', 'align-items': 'center', gap: '0.5rem', cursor: 'pointer' }}>
                <input
                  type="checkbox"
                  checked={readOnly()}
                  onChange={(e) => setReadOnly(e.target.checked)}
                />
                <span style={{ 'font-size': '0.875rem' }}>Read-only</span>
              </label>
            </div>

            <div style={{ display: 'flex', 'justify-content': 'flex-end', gap: '0.5rem' }}>
              <button
                onClick={() => setShowAddDialog(false)}
                style={secondaryButtonStyle()}
                disabled={addLoading()}
              >
                Cancel
              </button>
              <button
                onClick={handleAddMount}
                style={primaryButtonStyle()}
                disabled={addLoading()}
              >
                {addLoading() ? 'Creating...' : 'Create Mount'}
              </button>
            </div>
          </div>
        </div>
      </Show>
    </div>
  );
};

function cardStyle(): Record<string, string> {
  return {
    background: 'var(--muted)',
    border: '1px solid var(--border)',
    'border-radius': 'var(--radius)',
    padding: '1rem',
  };
}

function errorStyle(): Record<string, string> {
  return {
    background: 'hsl(0 84% 60% / 0.08)',
    border: '1px solid hsl(0 84% 60% / 0.3)',
    padding: '0.75rem 1rem',
    'border-radius': '8px',
    'margin-bottom': '1rem',
    color: 'var(--accent-red)',
    'font-size': '0.875rem',
  };
}

function primaryButtonStyle(): Record<string, string> {
  return {
    background: 'var(--fg)',
    color: 'var(--bg)',
    border: 'none',
    padding: '0.5rem 1rem',
    'border-radius': '8px',
    cursor: 'pointer',
    'font-size': '0.875rem',
    'font-weight': '500',
    'font-family': 'inherit',
  };
}

function secondaryButtonStyle(): Record<string, string> {
  return {
    background: 'var(--bg)',
    color: 'var(--fg)',
    border: '1px solid var(--border)',
    padding: '0.5rem 1rem',
    'border-radius': '8px',
    cursor: 'pointer',
    'font-size': '0.875rem',
    'font-family': 'inherit',
  };
}

function smallButtonStyle(): Record<string, string> {
  return {
    background: 'var(--bg)',
    color: 'var(--fg)',
    border: '1px solid var(--border)',
    padding: '0.375rem 0.75rem',
    'border-radius': '6px',
    cursor: 'pointer',
    'font-size': '0.75rem',
    'font-family': 'inherit',
  };
}

function smallDangerButtonStyle(): Record<string, string> {
  return {
    ...smallButtonStyle(),
    color: 'var(--accent-red)',
    'border-color': 'hsl(0 84% 60% / 0.3)',
  };
}

function tagStyle(): Record<string, string> {
  return {
    background: 'var(--bg)',
    border: '1px solid var(--border)',
    padding: '0.125rem 0.375rem',
    'border-radius': '4px',
    'font-size': '0.625rem',
    'font-weight': '500',
    'text-transform': 'uppercase',
    'letter-spacing': '0.025em',
  };
}

function overlayStyle(): Record<string, string> {
  return {
    position: 'fixed',
    top: '0',
    left: '0',
    right: '0',
    bottom: '0',
    background: 'rgba(0, 0, 0, 0.5)',
    display: 'flex',
    'align-items': 'center',
    'justify-content': 'center',
    'z-index': '100',
  };
}

function dialogStyle(): Record<string, string> {
  return {
    background: 'var(--bg)',
    border: '1px solid var(--border)',
    'border-radius': 'var(--radius)',
    padding: '1.5rem',
    width: '400px',
    'max-width': '90vw',
  };
}

function labelStyle(): Record<string, string> {
  return {
    display: 'block',
    'font-size': '0.75rem',
    'font-weight': '500',
    'margin-bottom': '0.375rem',
    color: 'var(--muted-fg)',
  };
}

function inputStyle(): Record<string, string> {
  return {
    width: '100%',
    padding: '0.5rem 0.75rem',
    border: '1px solid var(--border)',
    'border-radius': '6px',
    'font-size': '0.875rem',
    'font-family': 'inherit',
    background: 'var(--bg)',
    color: 'var(--fg)',
  };
}

export default Mounts;
